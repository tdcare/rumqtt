/**
 * rumqttd FFI 返回的 JSON 数据模型
 *
 * 这些接口用于解析 RumqttdBridge.getConnections()、getMeters()、getAlerts()
 * 返回的 JSON 字符串。字段名与 Rust 侧 serde 序列化的 JSON key 对应。
 */

// ========== 连接信息 ==========

/**
 * 单个客户端连接的信息
 *
 * 对应 rumqttd_get_connections() 返回的 JSON 数组中的每个元素。
 */
export interface ConnectionInfo {
  /** 内部连接 ID */
  connection_id: number;
  /** MQTT 客户端标识符 */
  client_id: string;
  /** 该客户端订阅的 topic filter 列表 */
  subscriptions: string[];
  /** 入站发布消息计数 */
  incoming_publish_count: number;
  /** 入站发布消息总字节数 */
  incoming_publish_size: number;
  /** 出站发布消息计数 */
  outgoing_publish_count: number;
  /** 出站发布消息总字节数 */
  outgoing_publish_size: number;
  /**
   * 连接状态的调试表示
   * 例如："Ready"、"Paused(Caughtup)"
   * 注意：格式在不同版本间不保证稳定
   */
  status: string;
  /** 客户端 IP 地址 */
  peer_addr: string;
}

// ========== 路由器指标 ==========

/**
 * 路由器级别的运行指标
 *
 * 对应 rumqttd_get_meters() 返回的 JSON 中 Router 类型的指标数据。
 * Rust 侧 Meter 是枚举类型，JSON 序列化格式取决于 serde 配置。
 */
export interface RouterMeter {
  /** 时间戳（Unix 毫秒） */
  timestamp: number;
  /** 序列号 */
  sequence: number;
  /** 路由器 ID */
  router_id: number;
  /** 当前总连接数 */
  total_connections: number;
  /** 当前总订阅数 */
  total_subscriptions: number;
  /** 累计发布消息数 */
  total_publishes: number;
  /** 累计失败发布数 */
  failed_publishes: number;
}

// ========== Meter 条目 ==========

/**
 * meters JSON 数组中的单个条目
 *
 * Rust 枚举序列化后形如 { "Router": { ... } } 或 { "Subscription": { ... } }
 */
export interface MeterEntry {
  Router?: RouterMeter;
}

// ========== 订阅指标 ==========

/**
 * 订阅级别的指标
 *
 * 对应 Meter::Subscription 变体的数据。
 */
export interface SubscriptionMeter {
  /** Topic filter */
  filter: string;
  /** 匹配该 filter 的消息计数 */
  publish_count: number;
  /** 匹配该 filter 的消息总字节数 */
  publish_size: number;
}

// ========== 告警信息 ==========

/**
 * 告警信息
 *
 * 对应 rumqttd_get_alerts() 返回的 JSON 数组中的元素。
 * 具体字段取决于 Rust 侧 Alert 的序列化格式。
 */
export interface AlertInfo {
  /** 告警类型/标题 */
  title: string;
  /** 告警详情 */
  message: string;
  /** 时间戳 */
  timestamp: number;
}

// ========== 汇总数据（UI 层使用）==========

/**
 * Broker 运行状态汇总，供 UI 层使用
 */
export interface BrokerStatus {
  /** 是否正在运行 */
  isRunning: boolean;
  /** 当前连接数 */
  connectionCount: number;
  /** 当前订阅数 */
  subscriptionCount: number;
  /** 累计发布消息数 */
  totalPublishes: number;
  /** 累计失败消息数 */
  failedPublishes: number;
  /** 连接列表 */
  connections: ConnectionInfo[];
}
