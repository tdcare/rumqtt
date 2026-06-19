//! NAPI bridge for rumqttd — exposes the MQTT broker to OpenHarmony (OHOS) ArkTS.
//!
//! Keeps the Broker alive (manual Server spawn) to preserve get_connections/get_meters.
//! The earlier instability was caused by missing [router] id=0 and the websocket cfg
//! guard, both now fixed.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Once};
use std::thread::JoinHandle;
use std::time::Duration;

use napi_derive_ohos::napi;
use napi_ohos::bindgen_prelude::*;

use rumqttd::alerts::AlertsLink;
use rumqttd::meters::MetersLink;
use rumqttd::protocol::v4::V4;
use rumqttd::protocol::v5::V5;
use rumqttd::{Broker, Config, ConnectionInfo, LinkType, Meter};

static INIT_LOGGING: Once = Once::new();

fn init_logging_once() {
    INIT_LOGGING.call_once(|| {
        use tracing_subscriber::{fmt, EnvFilter};
        let filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("rumqttd=info"));
        fmt().with_env_filter(filter).init();
    });
}

struct BrokerInner {
    config: Config,
    broker: Option<Broker>,
    running: AtomicBool,
    server_handles: Vec<JoinHandle<()>>,
    meters_link: Option<MetersLink>,
    alerts_link: Option<AlertsLink>,
}

#[napi(js_name = "RumqttdBroker")]
pub struct JsRumqttdBroker {
    inner: Mutex<BrokerInner>,
}

#[napi]
impl JsRumqttdBroker {
    #[napi(constructor)]
    pub fn new(config_toml: String) -> Result<Self> {
        init_logging_once();
        let config: Config = toml::from_str(&config_toml)
            .map_err(|e| Error::from_reason(format!("TOML: {e}")))?;
        let broker = Broker::new(config.clone());
        let meters_link = broker.meters().map_err(|e| Error::from_reason(format!("meters: {e}")))?;
        let alerts_link = broker.alerts().map_err(|e| Error::from_reason(format!("alerts: {e}")))?;
        Ok(Self { inner: Mutex::new(BrokerInner {
            config, broker: Some(broker), running: AtomicBool::new(false),
            server_handles: Vec::new(), meters_link: Some(meters_link), alerts_link: Some(alerts_link),
        })})
    }

    #[napi]
    pub fn start(&self) -> Result<()> {
        let mut inner = self.inner.lock().map_err(|_| Error::from_reason("poisoned"))?;
        if inner.running.load(Ordering::SeqCst) { return Err(Error::from_reason("already running")); }
        let broker = inner.broker.as_ref().ok_or_else(|| Error::from_reason("no broker"))?;
        let router_tx = broker.router_tx();
        let mut handles = Vec::new();

        if let Some(v4) = &inner.config.v4 {
            for cfg in v4.values().cloned() {
                let tx = router_tx.clone();
                handles.push(std::thread::Builder::new().name(cfg.name.clone()).spawn(move || {
                    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
                    rt.block_on(async { if let Err(e) = rumqttd::Server::new(cfg, tx, V4).start(LinkType::Remote).await { tracing::error!(error=?e,"V4"); }});
                }).map_err(|e| Error::from_reason(format!("V4: {e}")))?);
            }
        }
        if let Some(v5) = &inner.config.v5 {
            for cfg in v5.values().cloned() {
                let tx = router_tx.clone();
                handles.push(std::thread::Builder::new().name(cfg.name.clone()).spawn(move || {
                    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
                    rt.block_on(async { if let Err(e) = rumqttd::Server::new(cfg, tx, V5).start(LinkType::Remote).await { tracing::error!(error=?e,"V5"); }});
                }).map_err(|e| Error::from_reason(format!("V5: {e}")))?);
            }
        }
        if let Some(ws) = &inner.config.ws {
            for cfg in ws.values().cloned() {
                let tx = router_tx.clone();
                handles.push(std::thread::Builder::new().name(cfg.name.clone()).spawn(move || {
                    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
                    rt.block_on(async { if let Err(e) = rumqttd::Server::new(cfg, tx, V4).start(LinkType::Websocket).await { tracing::error!(error=?e,"WS"); }});
                }).map_err(|e| Error::from_reason(format!("WS: {e}")))?);
            }
        }
        inner.server_handles = handles;
        inner.running.store(true, Ordering::SeqCst);
        Ok(())
    }

    #[napi]
    pub fn stop(&self) -> Result<()> {
        let mut inner = self.inner.lock().map_err(|_| Error::from_reason("poisoned"))?;
        inner.running.store(false, Ordering::SeqCst);
        for h in std::mem::take(&mut inner.server_handles) { drop(h); }
        Ok(())
    }

    #[napi]
    pub fn get_connections(&self) -> Result<String> {
        let inner = self.inner.lock().map_err(|_| Error::from_reason("poisoned"))?;
        let broker = inner.broker.as_ref().ok_or_else(|| Error::from_reason("no broker"))?;
        match broker.get_connections() {
            Ok(conns) => serde_json::to_string(&conns).map_err(|e| Error::from_reason(format!("JSON: {e}"))),
            Err(e) => Err(Error::from_reason(format!("get_connections: {e}"))),
        }
    }

    #[napi]
    pub fn get_meters(&self) -> Result<String> {
        let inner = self.inner.lock().map_err(|_| Error::from_reason("poisoned"))?;
        let link = inner.meters_link.as_ref().ok_or_else(|| Error::from_reason("no meters"))?;
        let mut latest: Option<Vec<Meter>> = None;
        while let Ok(b) = link.recv() { latest = Some(b); }
        match latest {
            Some(m) => serde_json::to_string(&m).map_err(|e| Error::from_reason(format!("JSON: {e}"))),
            None => Ok("[]".to_string()),
        }
    }

    #[napi]
    pub fn get_alerts(&self) -> Result<String> {
        let inner = self.inner.lock().map_err(|_| Error::from_reason("poisoned"))?;
        let link = inner.alerts_link.as_ref().ok_or_else(|| Error::from_reason("no alerts"))?;
        match link.recv() {
            Ok(a) => serde_json::to_string(&a).map_err(|e| Error::from_reason(format!("JSON: {e}"))),
            Err(_) => Ok("[]".to_string()),
        }
    }

    #[napi(getter)]
    pub fn is_running(&self) -> bool {
        self.inner.lock().map(|i| i.running.load(Ordering::SeqCst)).unwrap_or(false)
    }

    #[napi]
    pub fn free(&self) -> Result<()> {
        let mut inner = self.inner.lock().map_err(|_| Error::from_reason("poisoned"))?;
        inner.running.store(false, Ordering::SeqCst);
        for h in std::mem::take(&mut inner.server_handles) { drop(h); }
        inner.broker = None;
        Ok(())
    }
}
