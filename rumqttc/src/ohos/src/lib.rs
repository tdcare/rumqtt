use napi_derive_ohos::napi;
use rumqttc::{Client, Connection, Event, Incoming, MqttOptions, Outgoing, QoS, Transport};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use ohos_hilog_binding::{hilog_debug, hilog_info};

// ============================================================
// 辅助函数
// ============================================================

/// JSON 字符串转义：处理 \ " \n \r \t 及其他控制字符
fn json_escape(input: &str) -> String {
    let mut out = String::with_capacity(input.len() + 16);
    for ch in input.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                // 其他控制字符用 \uXXXX
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out
}

/// 将事件序列化为 JSON 字符串（手动拼接避免 serde 依赖）
fn event_json(event_type: &str, topic: &str, payload: &str, error: &str) -> String {
    let mut s = String::with_capacity(256);
    s.push_str("{\"type\":\"");
    s.push_str(event_type);
    s.push('"');
    if !topic.is_empty() {
        s.push_str(",\"topic\":\"");
        s.push_str(&json_escape(topic));
        s.push('"');
    }
    if !payload.is_empty() {
        s.push_str(",\"payload\":\"");
        s.push_str(&json_escape(payload));
        s.push('"');
    }
    if !error.is_empty() {
        s.push_str(",\"error\":\"");
        s.push_str(&json_escape(error));
        s.push('"');
    }
    s.push('}');
    s
}

/// 事件队列最大长度，防止无限堆积
const MAX_EVENT_QUEUE_SIZE: usize = 50;

/// 后台线程：运行 rumqttc 事件循环
fn run_event_loop(
    mut connection: Connection,
    connected: Arc<AtomicBool>,
    event_queue: Arc<Mutex<VecDeque<String>>>,
) {
    let mut consecutive_errors: u32 = 0;

    for notification in connection.iter() {
        match notification {
            Ok(Event::Incoming(Incoming::ConnAck(_ack))) => {
                connected.store(true, Ordering::SeqCst);
                consecutive_errors = 0; // 连接成功，重置错误计数
                hilog_info!("[rumqttc-ohos] Connected to broker");
                let json = event_json("connected", "", "", "");
                if let Ok(mut q) = event_queue.lock() {
                    q.push_back(json);
                }
            }
            Ok(Event::Incoming(Incoming::Publish(p))) => {
                let payload_str = String::from_utf8_lossy(&p.payload).to_string();
                let qos_num = match p.qos {
                    QoS::AtMostOnce => "0",
                    QoS::AtLeastOnce => "1",
                    QoS::ExactlyOnce => "2",
                };
                // 构造带 qos 和 retain 的 JSON
                let mut json = String::with_capacity(512);
                json.push_str("{\"type\":\"message\",\"topic\":\"");
                json.push_str(&json_escape(&p.topic));
                json.push_str("\",\"payload\":\"");
                json.push_str(&json_escape(&payload_str));
                json.push_str("\",\"qos\":");
                json.push_str(qos_num);
                json.push_str(",\"retain\":");
                json.push_str(if p.retain { "true" } else { "false" });
                json.push('}');
                if let Ok(mut q) = event_queue.lock() {
                    // 消息事件始终推入，但控制队列上限
                    if q.len() >= MAX_EVENT_QUEUE_SIZE {
                        q.pop_front();
                    }
                    q.push_back(json);
                }
            }
            Ok(Event::Incoming(Incoming::Disconnect)) => {
                connected.store(false, Ordering::SeqCst);
                hilog_info!("[rumqttc-ohos] Received server disconnect");
                let json = event_json("disconnected", "", "", "");
                if let Ok(mut q) = event_queue.lock() {
                    q.push_back(json);
                }
            }
            Ok(Event::Outgoing(Outgoing::Disconnect)) => {
                connected.store(false, Ordering::SeqCst);
                hilog_info!("[rumqttc-ohos] Client disconnected");
                let json = event_json("disconnected", "", "", "");
                if let Ok(mut q) = event_queue.lock() {
                    q.push_back(json);
                }
                break; // 主动断开，退出事件循环
            }
            Err(e) => {
                connected.store(false, Ordering::SeqCst);
                consecutive_errors += 1;

                // 只推送第1次和每第10次错误事件，防止洪泛
                if consecutive_errors == 1 || consecutive_errors % 10 == 0 {
                    hilog_info!(format!(
                        "[rumqttc-ohos] Connection error #{}: {}",
                        consecutive_errors, e
                    ));
                    let json = event_json("error", "", "", &e.to_string());
                    if let Ok(mut q) = event_queue.lock() {
                        if q.len() < MAX_EVENT_QUEUE_SIZE {
                            q.push_back(json);
                        }
                    }
                }

                // 指数退避 sleep：防止 rumqttc 高速重试淹没事件队列
                // 1s → 2s → 3s → ... 最大 5s
                let backoff =
                    Duration::from_secs(std::cmp::min(consecutive_errors as u64, 5));
                thread::sleep(backoff);
                // rumqttc 内部会自动重连，不 break
            }
            _ => {}
        }
    }

    hilog_info!("[rumqttc-ohos] Event loop exited");
}

// ============================================================
// NAPI 导出：MqttClient 类
// ============================================================

#[napi]
pub struct MqttClient {
    client: Arc<Mutex<Client>>,
    connected: Arc<AtomicBool>,
    event_queue: Arc<Mutex<VecDeque<String>>>,
}

#[napi]
impl MqttClient {
    /// 构造函数：创建 MQTT 客户端并立即启动后台事件循环（自动连接）
    ///
    /// - broker_url: 完整的 broker URL（支持 tcp://, ws://, wss://）
    /// - client_id: 客户端标识
    /// - username / password: 可选认证信息
    /// - keep_alive_secs: 心跳间隔（秒），默认 60
    /// - clean_session: 是否清除会话，默认 true
    #[napi(constructor)]
    pub fn new(
        broker_url: String,
        client_id: String,
        username: Option<String>,
        password: Option<String>,
        keep_alive_secs: Option<u32>,
        clean_session: Option<bool>,
    ) -> Self {
        hilog_info!(format!(
            "[rumqttc-ohos] Creating client, broker_url: {}",
            &broker_url
        ));

        // 检测是否为 WebSocket URL
        let is_websocket =
            broker_url.starts_with("ws://") || broker_url.starts_with("wss://");

        let mut opts = if is_websocket {
            hilog_info!("[rumqttc-ohos] WebSocket transport detected");

            // 解析端口
            let port = match url::Url::parse(&broker_url) {
                Ok(parsed_url) => parsed_url.port().unwrap_or(
                    if broker_url.starts_with("wss://") {
                        443
                    } else {
                        80
                    },
                ),
                Err(_) => 80,
            };

            hilog_debug!(format!(
                "[rumqttc-ohos] WebSocket config - URL: {}, port: {}",
                &broker_url, port
            ));

            // 将完整的 WebSocket URL 作为 host 参数
            let mut options = MqttOptions::new(&client_id, &broker_url, port);
            if broker_url.starts_with("wss://") {
                options.set_transport(Transport::wss_with_default_config());
            } else {
                options.set_transport(Transport::Ws);
            }
            options
        } else {
            // TCP 模式：手动解析 URL
            let (host, port) = if broker_url.starts_with("tcp://") {
                let without_scheme = &broker_url[6..];
                if let Some(pos) = without_scheme.find(':') {
                    let h = without_scheme[..pos].to_string();
                    let p: u16 = without_scheme[pos + 1..].parse().unwrap_or(1883);
                    (h, p)
                } else {
                    (without_scheme.to_string(), 1883)
                }
            } else if let Some(pos) = broker_url.rfind(':') {
                let h = broker_url[..pos].to_string();
                let p: u16 = broker_url[pos + 1..].parse().unwrap_or(1883);
                (h, p)
            } else {
                (broker_url.clone(), 1883)
            };

            hilog_info!(format!(
                "[rumqttc-ohos] TCP config - host: {}, port: {}",
                &host, port
            ));
            MqttOptions::new(&client_id, &host, port)
        };

        // 认证
        if let Some(ref user) = username {
            let pass = password.unwrap_or_default();
            opts.set_credentials(user.clone(), pass);
            hilog_debug!("[rumqttc-ohos] Credentials configured");
        }

        // 心跳
        let ka = keep_alive_secs.unwrap_or(60) as u64;
        opts.set_keep_alive(Duration::from_secs(ka));

        // 会话
        opts.set_clean_session(clean_session.unwrap_or(true));

        // 创建同步客户端
        let (client, connection) = Client::new(opts, 10);

        let connected = Arc::new(AtomicBool::new(false));
        let event_queue = Arc::new(Mutex::new(VecDeque::<String>::new()));

        // 启动后台事件循环线程
        let conn_flag = connected.clone();
        let eq = event_queue.clone();
        thread::spawn(move || {
            run_event_loop(connection, conn_flag, eq);
        });

        hilog_info!("[rumqttc-ohos] Client created, event loop started");

        Self {
            client: Arc::new(Mutex::new(client)),
            connected,
            event_queue,
        }
    }

    // ---- 状态查询 ----

    /// 检查客户端是否已连接到 broker
    #[napi]
    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    // ---- 订阅 / 取消订阅 ----

    /// 订阅指定主题
    ///
    /// - topic: MQTT 主题（支持通配符 + 和 #）
    /// - qos: 服务质量等级（0, 1, 2）
    #[napi]
    pub fn subscribe(&self, topic: String, qos: u32) -> bool {
        let qos_level = match qos {
            0 => QoS::AtMostOnce,
            1 => QoS::AtLeastOnce,
            2 => QoS::ExactlyOnce,
            _ => return false,
        };
        if let Ok(client) = self.client.lock() {
            let result = client.subscribe(&topic, qos_level).is_ok();
            hilog_debug!(format!(
                "[rumqttc-ohos] subscribe('{}', qos={}) => {}",
                topic, qos, result
            ));
            result
        } else {
            false
        }
    }

    /// 取消订阅指定主题
    #[napi]
    pub fn unsubscribe(&self, topic: String) -> bool {
        if let Ok(client) = self.client.lock() {
            let result = client.unsubscribe(&topic).is_ok();
            hilog_debug!(format!(
                "[rumqttc-ohos] unsubscribe('{}') => {}",
                topic, result
            ));
            result
        } else {
            false
        }
    }

    // ---- 发布 ----

    /// 发布消息到指定主题
    ///
    /// - topic: MQTT 主题
    /// - payload: 消息内容（字符串）
    /// - qos: 服务质量等级（0, 1, 2）
    /// - retain: 是否保留消息
    #[napi]
    pub fn publish(&self, topic: String, payload: String, qos: u32, retain: bool) -> bool {
        let qos_level = match qos {
            0 => QoS::AtMostOnce,
            1 => QoS::AtLeastOnce,
            2 => QoS::ExactlyOnce,
            _ => return false,
        };
        if let Ok(client) = self.client.lock() {
            let result = client
                .publish(&topic, qos_level, retain, payload.as_bytes())
                .is_ok();
            hilog_debug!(format!(
                "[rumqttc-ohos] publish('{}', qos={}, retain={}) => {}",
                topic, qos, retain, result
            ));
            result
        } else {
            false
        }
    }

    // ---- 断开连接 ----

    /// 主动断开与 broker 的连接
    #[napi]
    pub fn disconnect(&self) -> bool {
        hilog_info!("[rumqttc-ohos] Disconnecting...");
        if let Ok(client) = self.client.lock() {
            client.disconnect().is_ok()
        } else {
            false
        }
    }

    // ---- 事件轮询 ----
    // ArkTS 端通过定时器调用 pollEvent() / pollAllEvents() 获取事件

    /// 从事件队列中取出一条事件（JSON 字符串）
    ///
    /// 返回 null 表示队列为空
    #[napi]
    pub fn poll_event(&self) -> Option<String> {
        if let Ok(mut q) = self.event_queue.lock() {
            q.pop_front()
        } else {
            None
        }
    }

    /// 一次性取出所有待处理事件（减少跨语言调用次数）
    #[napi]
    pub fn poll_all_events(&self) -> Vec<String> {
        if let Ok(mut q) = self.event_queue.lock() {
            q.drain(..).collect()
        } else {
            Vec::new()
        }
    }
}

// ============================================================
// NAPI 导出：工具函数
// ============================================================

/// 获取 rumqttc 库版本信息
#[napi]
pub fn get_rumqttc_version() -> String {
    "0.25.1-ohos".to_string()
}
