use std::cell::RefCell;
use std::collections::VecDeque;
use std::ffi::{c_char, c_int, CStr, CString};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Once};
use std::thread;
use std::time::Duration;

use rumqttc::{Client, Connection, Event, Incoming, MqttOptions, Outgoing, QoS, Transport};

// ============================================================
// Logging initialization
// ============================================================

static INIT_LOGGING: Once = Once::new();

fn init_logging_once() {
    INIT_LOGGING.call_once(|| {
        #[cfg(target_os = "android")]
        {
            android_logger::init_once(
                android_logger::Config::default()
                    .with_max_level(log::LevelFilter::Info)
                    .with_tag("rumqttc"),
            );
        }

        #[cfg(not(target_os = "android"))]
        {
            // Desktop: use env_logger or do nothing
            let _ = std::io::stderr();
        }
    });
}

// ============================================================
// Thread-local error (same pattern as rumqttd ffi.rs)
// ============================================================

thread_local! {
    static LAST_ERROR: RefCell<Option<CString>> = RefCell::new(None);
}

fn set_last_error(msg: &str) {
    LAST_ERROR.with(|cell| {
        *cell.borrow_mut() = CString::new(msg).ok();
    });
}

// ============================================================
// Panic-safe wrapper
// ============================================================

fn catch_and_log<F, T>(default: T, f: F) -> T
where
    F: FnOnce() -> Result<T, Box<dyn std::error::Error>> + std::panic::UnwindSafe,
{
    match catch_unwind(f) {
        Ok(Ok(val)) => val,
        Ok(Err(e)) => {
            set_last_error(&e.to_string());
            default
        }
        Err(_) => {
            set_last_error("a panic occurred in FFI call");
            default
        }
    }
}

// ============================================================
// JSON helpers (same as OHOS lib.rs)
// ============================================================

/// JSON string escape: handles \ " \n \r \t and other control characters
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
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out
}

/// Serialize event as JSON string (hand-crafted to avoid serde dependency)
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

/// Maximum event queue size to prevent unbounded growth
const MAX_EVENT_QUEUE_SIZE: usize = 50;

// ============================================================
// Opaque handle
// ============================================================

struct RumqttcClientInner {
    client: Client,
    connected: Arc<AtomicBool>,
    event_queue: Arc<Mutex<VecDeque<String>>>,
}

pub struct RumqttcClient {
    inner: Mutex<RumqttcClientInner>,
}

// ============================================================
// Background event loop (same logic as OHOS lib.rs)
// ============================================================

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
                consecutive_errors = 0;
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
                    if q.len() >= MAX_EVENT_QUEUE_SIZE {
                        q.pop_front();
                    }
                    q.push_back(json);
                }
            }
            Ok(Event::Incoming(Incoming::Disconnect)) => {
                connected.store(false, Ordering::SeqCst);
                let json = event_json("disconnected", "", "", "");
                if let Ok(mut q) = event_queue.lock() {
                    q.push_back(json);
                }
            }
            Ok(Event::Outgoing(Outgoing::Disconnect)) => {
                connected.store(false, Ordering::SeqCst);
                let json = event_json("disconnected", "", "", "");
                if let Ok(mut q) = event_queue.lock() {
                    q.push_back(json);
                }
                break; // Client-initiated disconnect, exit event loop
            }
            Err(e) => {
                connected.store(false, Ordering::SeqCst);
                consecutive_errors += 1;

                // Only push 1st and every 10th error event to prevent flooding
                if consecutive_errors == 1 || consecutive_errors % 10 == 0 {
                    let json = event_json("error", "", "", &e.to_string());
                    if let Ok(mut q) = event_queue.lock() {
                        if q.len() < MAX_EVENT_QUEUE_SIZE {
                            q.push_back(json);
                        }
                    }
                }

                // Exponential backoff: 1s → 2s → ... max 5s
                let backoff =
                    Duration::from_secs(std::cmp::min(consecutive_errors as u64, 5));
                thread::sleep(backoff);
            }
            _ => {}
        }
    }
}

// ============================================================
// Helper: read a C string, returning None for NULL
// ============================================================

unsafe fn read_c_str<'a>(ptr: *const c_char) -> Option<&'a str> {
    if ptr.is_null() {
        return None;
    }
    CStr::from_ptr(ptr).to_str().ok()
}

unsafe fn read_c_str_required<'a>(ptr: *const c_char, name: &str) -> Result<&'a str, Box<dyn std::error::Error>> {
    if ptr.is_null() {
        return Err(format!("{} pointer is null", name).into());
    }
    CStr::from_ptr(ptr)
        .to_str()
        .map_err(|e| format!("Invalid UTF-8 in {}: {}", name, e).into())
}

// ============================================================
// FFI exports
// ============================================================

/// Create a new MQTT client and start the background event loop.
///
/// # Parameters
/// - `broker_url`: broker URL (tcp://host:port, ws://host:port/path, wss://...)
/// - `client_id`: MQTT client identifier
/// - `username`: optional, NULL if not needed
/// - `password`: optional, NULL if not needed
/// - `keep_alive_secs`: keep-alive interval in seconds (0 = default 60)
/// - `clean_session`: 1 = clean session, 0 = persistent session
///
/// # Returns
/// Opaque pointer on success, NULL on failure (call rumqttc_last_error).
///
/// # Safety
/// All `*const c_char` parameters must be valid null-terminated UTF-8 strings or NULL.
#[no_mangle]
pub unsafe extern "C" fn rumqttc_create(
    broker_url: *const c_char,
    client_id: *const c_char,
    username: *const c_char,
    password: *const c_char,
    keep_alive_secs: u32,
    clean_session: c_int,
) -> *mut RumqttcClient {
    let result = catch_unwind(AssertUnwindSafe(|| {
        init_logging_once();

        let url_str = match read_c_str_required(broker_url, "broker_url") {
            Ok(s) => s,
            Err(e) => {
                set_last_error(&e.to_string());
                return std::ptr::null_mut();
            }
        };

        let id_str = match read_c_str_required(client_id, "client_id") {
            Ok(s) => s,
            Err(e) => {
                set_last_error(&e.to_string());
                return std::ptr::null_mut();
            }
        };

        let user = read_c_str(username);
        let pass = read_c_str(password);

        // Detect transport from URL scheme
        let is_websocket = url_str.starts_with("ws://") || url_str.starts_with("wss://");

        let mut opts = if is_websocket {
            // Parse port from URL
            let port = match url::Url::parse(url_str) {
                Ok(parsed) => parsed.port().unwrap_or(
                    if url_str.starts_with("wss://") { 443 } else { 80 },
                ),
                Err(_) => 80,
            };

            let mut options = MqttOptions::new(id_str, url_str, port);
            if url_str.starts_with("wss://") {
                options.set_transport(Transport::wss_with_default_config());
            } else {
                options.set_transport(Transport::Ws);
            }
            options
        } else {
            // TCP mode: parse host:port from URL
            let (host, port) = if url_str.starts_with("tcp://") {
                let without_scheme = &url_str[6..];
                if let Some(pos) = without_scheme.find(':') {
                    let h = &without_scheme[..pos];
                    let p: u16 = without_scheme[pos + 1..].parse().unwrap_or(1883);
                    (h.to_string(), p)
                } else {
                    (without_scheme.to_string(), 1883)
                }
            } else if let Some(pos) = url_str.rfind(':') {
                let h = &url_str[..pos];
                let p: u16 = url_str[pos + 1..].parse().unwrap_or(1883);
                (h.to_string(), p)
            } else {
                (url_str.to_string(), 1883)
            };

            MqttOptions::new(id_str, &host, port)
        };

        // Credentials
        if let Some(u) = user {
            let p = pass.unwrap_or("");
            opts.set_credentials(u, p);
        }

        // Keep alive
        let ka = if keep_alive_secs == 0 { 60 } else { keep_alive_secs as u64 };
        opts.set_keep_alive(Duration::from_secs(ka));

        // Clean session
        opts.set_clean_session(clean_session != 0);

        // Create synchronous client
        let (client, connection) = Client::new(opts, 10);

        let connected = Arc::new(AtomicBool::new(false));
        let event_queue = Arc::new(Mutex::new(VecDeque::<String>::new()));

        // Spawn background event loop thread
        let conn_flag = connected.clone();
        let eq = event_queue.clone();
        thread::Builder::new()
            .name("rumqttc-ffi".to_owned())
            .spawn(move || {
                run_event_loop(connection, conn_flag, eq);
            })
            .ok();

        let handle = Box::new(RumqttcClient {
            inner: Mutex::new(RumqttcClientInner {
                client,
                connected,
                event_queue,
            }),
        });

        Box::into_raw(handle)
    }));

    match result {
        Ok(ptr) => ptr,
        Err(_) => {
            set_last_error("panic occurred in rumqttc_create");
            std::ptr::null_mut()
        }
    }
}

/// Free a client instance and release all associated resources.
///
/// After this call the pointer is invalid. Passing NULL is a safe no-op.
///
/// # Safety
/// `client` must be NULL or a valid pointer returned by `rumqttc_create`.
#[no_mangle]
pub unsafe extern "C" fn rumqttc_free(client: *mut RumqttcClient) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if client.is_null() {
            return;
        }
        let _ = Box::from_raw(client);
    }));
}

/// Subscribe to a topic.
///
/// # Returns
/// 0 on success, -1 on failure.
///
/// # Safety
/// `client` must be a valid pointer. `topic` must be a valid C string.
#[no_mangle]
pub unsafe extern "C" fn rumqttc_subscribe(
    client: *mut RumqttcClient,
    topic: *const c_char,
    qos: c_int,
) -> c_int {
    catch_and_log(-1, AssertUnwindSafe(|| {
        if client.is_null() {
            return Err("client pointer is null".into());
        }
        let topic_str = read_c_str_required(topic, "topic")?;
        let qos_level = match qos {
            0 => QoS::AtMostOnce,
            1 => QoS::AtLeastOnce,
            2 => QoS::ExactlyOnce,
            _ => return Err(format!("invalid QoS value: {}", qos).into()),
        };

        let inner = (*client).inner.lock().map_err(|_| "mutex poisoned")?;
        inner.client.subscribe(topic_str, qos_level).map_err(|e| format!("subscribe failed: {}", e))?;
        Ok(0)
    }))
}

/// Unsubscribe from a topic.
///
/// # Returns
/// 0 on success, -1 on failure.
///
/// # Safety
/// `client` must be a valid pointer. `topic` must be a valid C string.
#[no_mangle]
pub unsafe extern "C" fn rumqttc_unsubscribe(
    client: *mut RumqttcClient,
    topic: *const c_char,
) -> c_int {
    catch_and_log(-1, AssertUnwindSafe(|| {
        if client.is_null() {
            return Err("client pointer is null".into());
        }
        let topic_str = read_c_str_required(topic, "topic")?;

        let inner = (*client).inner.lock().map_err(|_| "mutex poisoned")?;
        inner.client.unsubscribe(topic_str).map_err(|e| format!("unsubscribe failed: {}", e))?;
        Ok(0)
    }))
}

/// Publish a message to a topic.
///
/// # Parameters
/// - `payload`: message payload bytes
/// - `payload_len`: length of payload in bytes
/// - `qos`: 0, 1, or 2
/// - `retain`: 1 = retain, 0 = don't retain
///
/// # Returns
/// 0 on success, -1 on failure.
///
/// # Safety
/// `client` must be a valid pointer. `topic` must be a valid C string.
/// `payload` must point to at least `payload_len` valid bytes (NULL allowed if payload_len is 0).
#[no_mangle]
pub unsafe extern "C" fn rumqttc_publish(
    client: *mut RumqttcClient,
    topic: *const c_char,
    payload: *const u8,
    payload_len: u32,
    qos: c_int,
    retain: c_int,
) -> c_int {
    catch_and_log(-1, AssertUnwindSafe(|| {
        if client.is_null() {
            return Err("client pointer is null".into());
        }
        let topic_str = read_c_str_required(topic, "topic")?;
        let qos_level = match qos {
            0 => QoS::AtMostOnce,
            1 => QoS::AtLeastOnce,
            2 => QoS::ExactlyOnce,
            _ => return Err(format!("invalid QoS value: {}", qos).into()),
        };

        let data: &[u8] = if payload.is_null() || payload_len == 0 {
            &[]
        } else {
            std::slice::from_raw_parts(payload, payload_len as usize)
        };

        let inner = (*client).inner.lock().map_err(|_| "mutex poisoned")?;
        inner.client
            .publish(topic_str, qos_level, retain != 0, data)
            .map_err(|e| format!("publish failed: {}", e))?;
        Ok(0)
    }))
}

/// Disconnect from the broker.
///
/// # Returns
/// 0 on success, -1 on failure.
///
/// # Safety
/// `client` must be a valid pointer returned by `rumqttc_create`.
#[no_mangle]
pub unsafe extern "C" fn rumqttc_disconnect(client: *mut RumqttcClient) -> c_int {
    catch_and_log(-1, AssertUnwindSafe(|| {
        if client.is_null() {
            return Err("client pointer is null".into());
        }

        let inner = (*client).inner.lock().map_err(|_| "mutex poisoned")?;
        inner.client.disconnect().map_err(|e| format!("disconnect failed: {}", e))?;
        Ok(0)
    }))
}

/// Check if the client is currently connected.
///
/// # Returns
/// 1 if connected, 0 if disconnected, -1 on error.
///
/// # Safety
/// `client` must be a valid pointer returned by `rumqttc_create`.
#[no_mangle]
pub unsafe extern "C" fn rumqttc_is_connected(client: *mut RumqttcClient) -> c_int {
    catch_and_log(-1, AssertUnwindSafe(|| {
        if client.is_null() {
            return Err("client pointer is null".into());
        }

        let inner = (*client).inner.lock().map_err(|_| "mutex poisoned")?;
        let connected = inner.connected.load(Ordering::SeqCst);
        Ok(if connected { 1 } else { 0 })
    }))
}

/// Poll a single event from the event queue.
///
/// Returns a JSON string that must be freed with `rumqttc_free_string`.
/// Returns NULL if the queue is empty.
///
/// # Safety
/// `client` must be a valid pointer returned by `rumqttc_create`.
#[no_mangle]
pub unsafe extern "C" fn rumqttc_poll_event(client: *mut RumqttcClient) -> *mut c_char {
    let result = catch_unwind(AssertUnwindSafe(|| {
        if client.is_null() {
            set_last_error("client pointer is null");
            return std::ptr::null_mut();
        }

        let inner = match (*client).inner.lock() {
            Ok(guard) => guard,
            Err(_) => {
                set_last_error("mutex poisoned");
                return std::ptr::null_mut();
            }
        };

        let event = {
            let mut q = match inner.event_queue.lock() {
                Ok(q) => q,
                Err(_) => {
                    set_last_error("event_queue mutex poisoned");
                    return std::ptr::null_mut();
                }
            };
            q.pop_front()
        };

        match event {
            Some(json) => match CString::new(json) {
                Ok(cs) => cs.into_raw(),
                Err(e) => {
                    set_last_error(&format!("JSON contains interior NUL byte: {}", e));
                    std::ptr::null_mut()
                }
            },
            None => std::ptr::null_mut(),
        }
    }));

    match result {
        Ok(ptr) => ptr,
        Err(_) => {
            set_last_error("panic occurred in rumqttc_poll_event");
            std::ptr::null_mut()
        }
    }
}

/// Poll all events from the event queue at once.
///
/// Returns a JSON array string that must be freed with `rumqttc_free_string`.
/// Returns "[]" if the queue is empty.
///
/// # Safety
/// `client` must be a valid pointer returned by `rumqttc_create`.
#[no_mangle]
pub unsafe extern "C" fn rumqttc_poll_all_events(client: *mut RumqttcClient) -> *mut c_char {
    let result = catch_unwind(AssertUnwindSafe(|| {
        if client.is_null() {
            set_last_error("client pointer is null");
            return std::ptr::null_mut();
        }

        let inner = match (*client).inner.lock() {
            Ok(guard) => guard,
            Err(_) => {
                set_last_error("mutex poisoned");
                return std::ptr::null_mut();
            }
        };

        let events: Vec<String> = {
            let mut q = match inner.event_queue.lock() {
                Ok(q) => q,
                Err(_) => {
                    set_last_error("event_queue mutex poisoned");
                    return std::ptr::null_mut();
                }
            };
            q.drain(..).collect()
        };

        // Build JSON array manually
        let mut json = String::with_capacity(events.len() * 256 + 2);
        json.push('[');
        for (i, evt) in events.iter().enumerate() {
            if i > 0 {
                json.push(',');
            }
            json.push_str(evt);
        }
        json.push(']');

        match CString::new(json) {
            Ok(cs) => cs.into_raw(),
            Err(e) => {
                set_last_error(&format!("JSON contains interior NUL byte: {}", e));
                std::ptr::null_mut()
            }
        }
    }));

    match result {
        Ok(ptr) => ptr,
        Err(_) => {
            set_last_error("panic occurred in rumqttc_poll_all_events");
            std::ptr::null_mut()
        }
    }
}

/// Free a string returned by `rumqttc_poll_event` or `rumqttc_poll_all_events`.
///
/// Passing NULL is safe and will be a no-op.
///
/// # Safety
/// `s` must be NULL or a pointer previously returned by one of the above functions.
#[no_mangle]
pub unsafe extern "C" fn rumqttc_free_string(s: *mut c_char) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if s.is_null() {
            return;
        }
        let _ = CString::from_raw(s);
    }));
}

/// Return the last error message as a null-terminated UTF-8 string.
///
/// The returned pointer is valid until the next FFI call on the same thread.
/// Returns NULL if no error has been recorded.
///
/// # Safety
/// The caller must not free the returned pointer.
#[no_mangle]
pub unsafe extern "C" fn rumqttc_last_error() -> *const c_char {
    let result = catch_unwind(AssertUnwindSafe(|| {
        LAST_ERROR.with(|cell| {
            let borrow = cell.borrow();
            match borrow.as_ref() {
                Some(cstr) => cstr.as_ptr(),
                None => std::ptr::null(),
            }
        })
    }));

    match result {
        Ok(ptr) => ptr,
        Err(_) => std::ptr::null(),
    }
}

// ============================================================
// JNI exports for Android (Kotlin class:
// cn.tdcare.smartward.rust.mqtt.RumqttcClient)
//
// Thin wrappers over the C ABI above. Kept in sync with
// smartward-rust-bridge/src/main/java/.../mqtt/RumqttcClient.kt
// ============================================================

#[cfg(target_os = "android")]
mod android_jni {
    use super::*;
    use jni::objects::{JByteArray, JClass, JString};
    use jni::sys::{jbyteArray, jint, jlong, jstring};
    use jni::JNIEnv;

    /// Convert a JString to CString (empty string treated as empty, never fails hard)
    fn jstring_to_cstring(env: &mut JNIEnv, s: &JString) -> CString {
        let rust_str: String = env
            .get_string(s)
            .map(|js| js.into())
            .unwrap_or_default();
        CString::new(rust_str).unwrap_or_default()
    }

    #[no_mangle]
    pub extern "system" fn Java_cn_tdcare_smartward_rust_mqtt_RumqttcClient_nativeCreate(
        mut env: JNIEnv,
        _class: JClass,
        broker_url: JString,
        client_id: JString,
        username: JString,
        password: JString,
        keep_alive_secs: jint,
        clean_session: jint,
    ) -> jlong {
        let url = jstring_to_cstring(&mut env, &broker_url);
        let id = jstring_to_cstring(&mut env, &client_id);
        let user = jstring_to_cstring(&mut env, &username);
        let pass = jstring_to_cstring(&mut env, &password);

        // Empty username/password mean "no credentials" — pass NULL
        let user_ptr = if user.as_bytes().is_empty() { std::ptr::null() } else { user.as_ptr() };
        let pass_ptr = if pass.as_bytes().is_empty() { std::ptr::null() } else { pass.as_ptr() };

        let handle = unsafe {
            rumqttc_create(
                url.as_ptr(),
                id.as_ptr(),
                user_ptr,
                pass_ptr,
                keep_alive_secs.max(0) as u32,
                clean_session as c_int,
            )
        };
        handle as jlong
    }

    #[no_mangle]
    pub extern "system" fn Java_cn_tdcare_smartward_rust_mqtt_RumqttcClient_nativeFree(
        _env: JNIEnv,
        _class: JClass,
        handle: jlong,
    ) {
        unsafe { rumqttc_free(handle as *mut RumqttcClient) }
    }

    #[no_mangle]
    pub extern "system" fn Java_cn_tdcare_smartward_rust_mqtt_RumqttcClient_nativeSubscribe(
        mut env: JNIEnv,
        _class: JClass,
        handle: jlong,
        topic: JString,
        qos: jint,
    ) -> jint {
        let topic_c = jstring_to_cstring(&mut env, &topic);
        unsafe { rumqttc_subscribe(handle as *mut RumqttcClient, topic_c.as_ptr(), qos as c_int) }
    }

    #[no_mangle]
    pub extern "system" fn Java_cn_tdcare_smartward_rust_mqtt_RumqttcClient_nativeUnsubscribe(
        mut env: JNIEnv,
        _class: JClass,
        handle: jlong,
        topic: JString,
    ) -> jint {
        let topic_c = jstring_to_cstring(&mut env, &topic);
        unsafe { rumqttc_unsubscribe(handle as *mut RumqttcClient, topic_c.as_ptr()) }
    }

    #[no_mangle]
    pub extern "system" fn Java_cn_tdcare_smartward_rust_mqtt_RumqttcClient_nativePublish(
        mut env: JNIEnv,
        _class: JClass,
        handle: jlong,
        topic: JString,
        payload: jbyteArray,
        qos: jint,
        retain: jint,
    ) -> jint {
        let topic_c = jstring_to_cstring(&mut env, &topic);
        let payload_arr = unsafe { JByteArray::from_raw(payload) };
        let bytes: Vec<u8> = env
            .convert_byte_array(&payload_arr)
            .unwrap_or_default();
        unsafe {
            rumqttc_publish(
                handle as *mut RumqttcClient,
                topic_c.as_ptr(),
                if bytes.is_empty() { std::ptr::null() } else { bytes.as_ptr() },
                bytes.len() as u32,
                qos as c_int,
                retain as c_int,
            )
        }
    }

    #[no_mangle]
    pub extern "system" fn Java_cn_tdcare_smartward_rust_mqtt_RumqttcClient_nativeDisconnect(
        _env: JNIEnv,
        _class: JClass,
        handle: jlong,
    ) -> jint {
        unsafe { rumqttc_disconnect(handle as *mut RumqttcClient) }
    }

    #[no_mangle]
    pub extern "system" fn Java_cn_tdcare_smartward_rust_mqtt_RumqttcClient_nativeIsConnected(
        _env: JNIEnv,
        _class: JClass,
        handle: jlong,
    ) -> jint {
        unsafe { rumqttc_is_connected(handle as *mut RumqttcClient) }
    }

    #[no_mangle]
    pub extern "system" fn Java_cn_tdcare_smartward_rust_mqtt_RumqttcClient_nativePollAllEvents(
        env: JNIEnv,
        _class: JClass,
        handle: jlong,
    ) -> jstring {
        let ptr = unsafe { rumqttc_poll_all_events(handle as *mut RumqttcClient) };
        if ptr.is_null() {
            return std::ptr::null_mut();
        }
        let json = unsafe { CStr::from_ptr(ptr).to_string_lossy().into_owned() };
        unsafe { rumqttc_free_string(ptr) };
        match env.new_string(json) {
            Ok(s) => s.into_raw(),
            Err(_) => std::ptr::null_mut(),
        }
    }
}
