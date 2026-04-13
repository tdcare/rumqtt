// NOTE: 需要在 Cargo.toml 的 [dependencies] 中添加: toml = "0.8"
// 当前 Cargo.toml 中没有 toml crate，反序列化 TOML 配置字符串需要此依赖。

use std::cell::RefCell;
use std::ffi::{c_char, c_int, CStr, CString};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use flume::Sender;

use crate::link::alerts::AlertsLink;
use crate::link::meters::MetersLink;
use crate::router::{ConnectionInfo, Event};
use crate::{Broker, Config, ConnectionId};

/// Opaque handle exposed to C callers.
///
/// Internal state is protected by a `Mutex` to allow safe concurrent access
/// from multiple threads.
pub struct RumqttdBroker {
    inner: Mutex<RumqttdBrokerInner>,
}

struct RumqttdBrokerInner {
    broker: Option<Broker>,
    handle: Option<JoinHandle<()>>,
    running: Arc<AtomicBool>,
    router_tx: Option<Sender<(ConnectionId, Event)>>,
    meters_link: Option<MetersLink>,
    alerts_link: Option<AlertsLink>,
}

thread_local! {
    static LAST_ERROR: RefCell<Option<CString>> = RefCell::new(None);
}

/// Store an error message so that `rumqttd_last_error` can return it.
fn set_last_error(msg: &str) {
    LAST_ERROR.with(|cell| {
        *cell.borrow_mut() = CString::new(msg).ok();
    });
}

/// Helper: serialize a value to JSON C string. Returns null on failure.
fn to_json_c_string<T: serde::Serialize>(value: &T) -> *mut c_char {
    match serde_json::to_string(value) {
        Ok(json) => match CString::new(json) {
            Ok(cs) => cs.into_raw(),
            Err(e) => {
                set_last_error(&format!("JSON contains interior NUL byte: {e}"));
                std::ptr::null_mut()
            }
        },
        Err(e) => {
            set_last_error(&format!("JSON serialization failed: {e}"));
            std::ptr::null_mut()
        }
    }
}

/// Create a `RumqttdBroker` from a TOML configuration string.
///
/// Returns a heap-allocated pointer on success, or null on failure.
/// On failure the error message is available via `rumqttd_last_error`.
///
/// # Safety
/// `config_toml` must be a valid, non-null, null-terminated UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn rumqttd_create(config_toml: *const c_char) -> *mut RumqttdBroker {
    let result = catch_unwind(AssertUnwindSafe(|| {
        if config_toml.is_null() {
            set_last_error("config_toml pointer is null");
            return std::ptr::null_mut();
        }

        let c_str = match CStr::from_ptr(config_toml).to_str() {
            Ok(s) => s,
            Err(e) => {
                set_last_error(&format!("Invalid UTF-8 in config_toml: {e}"));
                return std::ptr::null_mut();
            }
        };

        let config: Config = match toml::from_str(c_str) {
            Ok(c) => c,
            Err(e) => {
                set_last_error(&format!("Failed to parse TOML config: {e}"));
                return std::ptr::null_mut();
            }
        };

        let broker = Broker::new(config);

        // Obtain links and router_tx before the broker is consumed by start().
        // The router is already running after Broker::new(), so these are valid.
        let router_tx = Some(broker.router_tx());

        let meters_link = match broker.meters() {
            Ok(link) => Some(link),
            Err(e) => {
                set_last_error(&format!("Failed to create meters link: {e}"));
                return std::ptr::null_mut();
            }
        };

        let alerts_link = match broker.alerts() {
            Ok(link) => Some(link),
            Err(e) => {
                set_last_error(&format!("Failed to create alerts link: {e}"));
                return std::ptr::null_mut();
            }
        };

        let handle = Box::new(RumqttdBroker {
            inner: Mutex::new(RumqttdBrokerInner {
                broker: Some(broker),
                handle: None,
                running: Arc::new(AtomicBool::new(false)),
                router_tx,
                meters_link,
                alerts_link,
            }),
        });

        Box::into_raw(handle)
    }));

    match result {
        Ok(ptr) => ptr,
        Err(_) => {
            set_last_error("panic occurred in rumqttd_create");
            std::ptr::null_mut()
        }
    }
}

/// Start the broker in a background thread.
///
/// Returns 0 on success, -1 on failure.
///
/// # Safety
/// `broker` must be a valid pointer returned by `rumqttd_create` and must not
/// have been freed.
#[no_mangle]
pub unsafe extern "C" fn rumqttd_start(broker: *mut RumqttdBroker) -> c_int {
    let result = catch_unwind(AssertUnwindSafe(|| {
        if broker.is_null() {
            set_last_error("broker pointer is null");
            return -1;
        }

        let mut inner = match (*broker).inner.lock() {
            Ok(guard) => guard,
            Err(_) => {
                set_last_error("mutex poisoned");
                return -1;
            }
        };

        if inner.handle.is_some() {
            set_last_error("broker is already running");
            return -1;
        }

        // Take ownership of the inner Broker so we can move it into the thread.
        let mut broker_instance = match inner.broker.take() {
            Some(b) => b,
            None => {
                set_last_error("broker has already been consumed (started or freed)");
                return -1;
            }
        };

        let running = inner.running.clone();
        running.store(true, Ordering::SeqCst);

        let handle = match std::thread::Builder::new()
            .name("rumqttd-ffi".to_owned())
            .spawn(move || {
                if let Err(e) = broker_instance.start() {
                    // The error is only visible from within the thread; we cannot
                    // propagate it back through the join handle easily, so we log it.
                    eprintln!("rumqttd broker error: {e}");
                }
            }) {
            Ok(h) => h,
            Err(e) => {
                set_last_error(&format!("Failed to spawn broker thread: {e}"));
                return -1;
            }
        };

        inner.handle = Some(handle);
        0
    }));

    match result {
        Ok(code) => code,
        Err(_) => {
            set_last_error("panic occurred in rumqttd_start");
            -1
        }
    }
}

/// Signal the broker to stop and detach the background thread.
///
/// NOTE: Currently this marks the broker as stopped and detaches the background
/// thread, but does NOT perform a graceful shutdown of internal server threads.
/// The broker thread resources may not be fully released until process exit.
///
/// Returns 0 on success, -1 on failure.
///
/// # Safety
/// `broker` must be a valid pointer returned by `rumqttd_create`.
#[no_mangle]
pub unsafe extern "C" fn rumqttd_stop(broker: *mut RumqttdBroker) -> c_int {
    let result = catch_unwind(AssertUnwindSafe(|| {
        if broker.is_null() {
            set_last_error("broker pointer is null");
            return -1;
        }

        let mut inner = match (*broker).inner.lock() {
            Ok(guard) => guard,
            Err(_) => {
                set_last_error("mutex poisoned");
                return -1;
            }
        };

        // Signal the running flag (other components can check this).
        inner.running.store(false, Ordering::SeqCst);

        // Take the thread handle and wait for it to finish.
        // Because Broker::start() blocks on thread::join internally, the only
        // reliable way to stop it from the outside is to drop the handle (which
        // detaches the thread) or wait. We take the handle and drop it to detach.
        if let Some(handle) = inner.handle.take() {
            // We cannot forcibly stop the broker because it does not expose a
            // shutdown mechanism. Detach the thread so the caller is unblocked.
            drop(handle);
        }

        0
    }));

    match result {
        Ok(code) => code,
        Err(_) => {
            set_last_error("panic occurred in rumqttd_stop");
            -1
        }
    }
}

/// Free all resources associated with a `RumqttdBroker`.
///
/// After this call the pointer is invalid and must not be used again.
/// It is safe to pass a null pointer (the call is a no-op).
///
/// NOTE: If the broker is still running, this will release the Rust-side handle
/// but internal server threads may continue until process exit.
/// Call `rumqttd_stop()` first if possible.
///
/// # Safety
/// `broker` must be null or a valid pointer returned by `rumqttd_create`.
#[no_mangle]
pub unsafe extern "C" fn rumqttd_free(broker: *mut RumqttdBroker) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if broker.is_null() {
            return;
        }
        // Reconstruct the Box and let it drop.
        let _ = Box::from_raw(broker);
    }));
}

/// Return the last error message as a null-terminated UTF-8 string.
///
/// The returned pointer is valid until the next FFI call **on the same thread**.
/// Returns null if no error has been recorded.
///
/// # Safety
/// The caller must not free the returned pointer.
#[no_mangle]
pub unsafe extern "C" fn rumqttd_last_error() -> *const c_char {
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

/// Get active connections as a JSON string.
///
/// Returns a null-terminated UTF-8 JSON array string. The caller must free the
/// returned string with `rumqttd_free_string`.
/// Returns NULL on failure; call `rumqttd_last_error()` for details.
///
/// # Safety
/// `broker` must be a valid, non-null pointer returned by `rumqttd_create`.
#[no_mangle]
pub unsafe extern "C" fn rumqttd_get_connections(broker: *mut RumqttdBroker) -> *mut c_char {
    let result = catch_unwind(AssertUnwindSafe(|| {
        if broker.is_null() {
            set_last_error("broker pointer is null");
            return std::ptr::null_mut();
        }

        let inner = match (*broker).inner.lock() {
            Ok(guard) => guard,
            Err(_) => {
                set_last_error("mutex poisoned");
                return std::ptr::null_mut();
            }
        };

        let router_tx = match &inner.router_tx {
            Some(tx) => tx,
            None => {
                set_last_error("router_tx is not available");
                return std::ptr::null_mut();
            }
        };

        // Send a GetConnections event and wait for the reply (blocking).
        let (tx, rx) = flume::bounded::<Vec<ConnectionInfo>>(1);
        if let Err(e) = router_tx.send((0, Event::GetConnections(tx))) {
            set_last_error(&format!("Failed to send GetConnections event: {e}"));
            return std::ptr::null_mut();
        }

        // Drop the lock before blocking on recv to avoid holding it too long.
        drop(inner);

        match rx.recv() {
            Ok(connections) => to_json_c_string(&connections),
            Err(e) => {
                set_last_error(&format!("Failed to receive connections: {e}"));
                std::ptr::null_mut()
            }
        }
    }));

    match result {
        Ok(ptr) => ptr,
        Err(_) => {
            set_last_error("panic occurred in rumqttd_get_connections");
            std::ptr::null_mut()
        }
    }
}

/// Get router metrics as a JSON string (non-blocking).
///
/// Returns the latest metrics data as a JSON array. If no new data is available,
/// returns the string `"[]"`.
/// The caller must free the returned string with `rumqttd_free_string`.
/// Returns NULL on failure; call `rumqttd_last_error()` for details.
///
/// # Safety
/// `broker` must be a valid, non-null pointer returned by `rumqttd_create`.
#[no_mangle]
pub unsafe extern "C" fn rumqttd_get_meters(broker: *mut RumqttdBroker) -> *mut c_char {
    let result = catch_unwind(AssertUnwindSafe(|| {
        if broker.is_null() {
            set_last_error("broker pointer is null");
            return std::ptr::null_mut();
        }

        let inner = match (*broker).inner.lock() {
            Ok(guard) => guard,
            Err(_) => {
                set_last_error("mutex poisoned");
                return std::ptr::null_mut();
            }
        };

        let meters_link = match &inner.meters_link {
            Some(link) => link,
            None => {
                set_last_error("meters_link is not available");
                return std::ptr::null_mut();
            }
        };

        // Drain all available meter batches to avoid channel backlog.
        // The timer pushes once per second; if the consumer polls less
        // frequently, old batches accumulate. We keep only the latest.
        let mut latest_meters: Option<Vec<crate::Meter>> = None;
        while let Ok(meters) = meters_link.recv() {
            latest_meters = Some(meters);
        }

        match latest_meters {
            Some(meters) => to_json_c_string(&meters),
            None => {
                // No new data available – return empty JSON array.
                let empty: Vec<()> = Vec::new();
                to_json_c_string(&empty)
            }
        }
    }));

    match result {
        Ok(ptr) => ptr,
        Err(_) => {
            set_last_error("panic occurred in rumqttd_get_meters");
            std::ptr::null_mut()
        }
    }
}

/// Get alerts as a JSON string (non-blocking).
///
/// Returns the latest alerts as a JSON array. If no new alerts are available,
/// returns the string `"[]"`.
/// The caller must free the returned string with `rumqttd_free_string`.
/// Returns NULL on failure; call `rumqttd_last_error()` for details.
///
/// # Safety
/// `broker` must be a valid, non-null pointer returned by `rumqttd_create`.
#[no_mangle]
pub unsafe extern "C" fn rumqttd_get_alerts(broker: *mut RumqttdBroker) -> *mut c_char {
    let result = catch_unwind(AssertUnwindSafe(|| {
        if broker.is_null() {
            set_last_error("broker pointer is null");
            return std::ptr::null_mut();
        }

        let inner = match (*broker).inner.lock() {
            Ok(guard) => guard,
            Err(_) => {
                set_last_error("mutex poisoned");
                return std::ptr::null_mut();
            }
        };

        let alerts_link = match &inner.alerts_link {
            Some(link) => link,
            None => {
                set_last_error("alerts_link is not available");
                return std::ptr::null_mut();
            }
        };

        // Non-blocking try_recv via AlertsLink::recv() which uses try_recv internally.
        match alerts_link.recv() {
            Ok(alerts) => to_json_c_string(&alerts),
            Err(_) => {
                // No new data available – return empty JSON array.
                let empty: Vec<()> = Vec::new();
                to_json_c_string(&empty)
            }
        }
    }));

    match result {
        Ok(ptr) => ptr,
        Err(_) => {
            set_last_error("panic occurred in rumqttd_get_alerts");
            std::ptr::null_mut()
        }
    }
}

/// Free a string returned by `rumqttd_get_connections`, `rumqttd_get_meters`,
/// or `rumqttd_get_alerts`.
///
/// Passing NULL is safe and will be a no-op.
///
/// # Safety
/// `s` must be NULL or a pointer previously returned by one of the above functions,
/// and must not have been freed already.
#[no_mangle]
pub unsafe extern "C" fn rumqttd_free_string(s: *mut c_char) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if s.is_null() {
            return;
        }
        // Reconstruct the CString and let it drop to free the memory.
        let _ = CString::from_raw(s);
    }));
}
