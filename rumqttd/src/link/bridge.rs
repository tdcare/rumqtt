use flume::Sender;

#[cfg(feature = "use-rustls")]
use std::{
    fs,
    io::{BufReader, Cursor},
    path::Path,
    sync::Arc,
};

use std::{io, net::AddrParseError, time::Duration};

use tokio::{
    net::TcpStream,
    time::{sleep, sleep_until, Instant},
};

#[cfg(feature = "use-rustls")]
use rustls_pemfile::Item;

#[cfg(feature = "use-rustls")]
use tokio_rustls::{
    rustls::{
        pki_types::{InvalidDnsNameError, ServerName},
        ClientConfig, Error as TLSError, RootCertStore,
    },
    TlsConnector,
};

use tracing::*;

use crate::{
    link::{local::LinkError, network::Network},
    local::LinkBuilder,
    protocol::{self, Connect, Login, Packet, PingReq, Protocol, QoS, RetainForwardRule, Subscribe},
    router::Event,
    BridgeConfig, ConnectionId, Notification, Transport,
};

use super::network;

#[cfg(feature = "use-rustls")]
use super::network::N;
#[cfg(feature = "use-rustls")]
use crate::ClientAuth;

#[cfg(feature = "websocket")]
use async_tungstenite::tokio::client_async_with_config;
#[cfg(feature = "websocket")]
use async_tungstenite::tungstenite::client::IntoClientRequest;
#[cfg(feature = "websocket")]
use async_tungstenite::tungstenite::protocol::WebSocketConfig;
#[cfg(feature = "websocket")]
use ws_stream_tungstenite::WsStream;

pub async fn start<P>(
    config: BridgeConfig,
    router_tx: Sender<(ConnectionId, Event)>,
    protocol: P,
) -> Result<(), BridgeError>
where
    P: Protocol + Clone + Send + 'static,
{
    let span = tracing::info_span!("bridge_link");
    let _guard = span.enter();

    eprintln!("[bridge] Starting bridge '{}' -> {} sub='{}' transport={:?}",
        config.name, config.addr, config.sub_path, config.transport);
    info!(
        client_id = config.name,
        remote_addr = &config.addr,
        "Starting bridge with subscription on filter \"{}\"",
        &config.sub_path,
    );
    let (mut tx, mut rx, _ack) = LinkBuilder::new(&config.name, router_tx)
        .dynamic_filters(true)
        .build()?;

    'outer: loop {
        let mut network = match network_connect(&config, &config.addr, protocol.clone()).await {
            Ok(v) => v,
            Err(e) => {
                eprintln!("[bridge] Connection error: {:?}", e);
                error!(error=?e, "Error, retrying");
                sleep(Duration::from_secs(config.reconnection_delay)).await;
                continue;
            }
        };
        info!(remote_addr = &config.addr, "Connected to remote");
        eprintln!("[bridge] TCP/WS connected to {}", config.addr);
        if let Err(e) = network_init(&config, &mut network).await {
            eprintln!("[bridge] network_init error: {:?}", e);
            warn!(
                "Unable to connect and subscribe to remote broker, reconnecting - {}",
                e
            );
            sleep(Duration::from_secs(config.reconnection_delay)).await;
            continue;
        }

        let ping_req = Packet::PingReq(PingReq);
        debug!("Received suback from {}", &config.addr);
        eprintln!("[bridge] CONNECT+SUBSCRIBE OK, entering main loop for {}", config.addr);

        let mut ping_time = Instant::now();
        let mut timeout = sleep_until(ping_time + Duration::from_secs(config.ping_delay));
        let mut ping_unacked = false;

        loop {
            tokio::select! {
                packet_res = network.read() => {
                    // resetting timeout because tokio::select! consumes the old timeout future
                    timeout = sleep_until(ping_time + Duration::from_secs(config.ping_delay));
                    let packet = match packet_res {
                        Ok(v) => v,
                        Err(e) => {
                            warn!("Unable to read from network stream, reconnecting - {}", e);
                            sleep(Duration::from_secs(config.reconnection_delay)).await;
                            continue 'outer;
                        }
                    };

                    match packet {
                        Packet::Publish(publish, publish_prop) => {
                            if let Err(e) = tx.send(Packet::Publish(publish, publish_prop)).await {
                                error!(error=?e, "Failed to forward publish to local router, reconnecting");
                                sleep(Duration::from_secs(config.reconnection_delay)).await;
                                continue 'outer;
                            }
                        }
                        Packet::PingResp(_) => ping_unacked = false,
                        // TODO: Handle incoming pubrel incase of QoS subscribe
                        packet => warn!("Expected publish, got {:?}", packet),
                    }
                }
                o = rx.next() => {
                    let notif = match o {
                        Ok(notif) => notif,
                        Err(e) => {
                            warn!("Local link error, reconnecting - {}", e);
                            sleep(Duration::from_secs(config.reconnection_delay)).await;
                            continue 'outer;
                        }
                    };
                    if let Some(notif) = notif {
                        match notif {
                            Notification::DeviceAck(ack) => {
                                if let Err(e) = network.write(ack.into()).await {
                                    warn!("Failed to write ack to remote, reconnecting - {}", e);
                                    sleep(Duration::from_secs(config.reconnection_delay)).await;
                                    continue 'outer;
                                }
                            },
                            other => warn!("Unexpected notification from router: {:?}", other),
                        }

                    }
                    timeout = sleep_until(ping_time + Duration::from_secs(config.ping_delay));

                }
                _ = timeout => {
                    // retry connection if ping not acked till next timeout
                    if ping_unacked {
                        warn!("No response to previous ping, reconnecting");
                        sleep(Duration::from_secs(config.reconnection_delay)).await;
                        continue 'outer;
                    }

                    if let Err(e) = network.write(ping_req.clone()).await {
                        warn!("Unable to write PINGREQ to network stream, reconnecting - {}", e);
                        sleep(Duration::from_secs(config.reconnection_delay)).await;
                        continue 'outer;
                    };
                    ping_unacked = true;

                    ping_time = Instant::now();
                    // resetting timeout because tokio::select! consumes the old timeout future
                    timeout = sleep_until(ping_time + Duration::from_secs(config.ping_delay));
                }
            }
        }
    }
}

async fn network_connect<P: Protocol>(
    config: &BridgeConfig,
    addr: &str,
    protocol: P,
) -> Result<Network<P>, BridgeError> {
    match &config.transport {
        Transport::Tcp => {
            let socket = TcpStream::connect(addr).await?;
            Ok(Network::new(
                Box::new(socket),
                config.connections.max_payload_size,
                config.connections.max_inflight_count,
                protocol,
            ))
        }
        #[cfg(feature = "use-rustls")]
        Transport::Tls { ca, client_auth } => {
            let tcp_stream = TcpStream::connect(addr).await?;
            // addr should be in format host:port
            let host = addr.split(':').next().unwrap();
            let socket = tls_connect(host, &ca, client_auth, tcp_stream).await?;
            Ok(Network::new(
                Box::new(socket),
                config.connections.max_payload_size,
                config.connections.max_inflight_count,
                protocol,
            ))
        }
        #[cfg(not(feature = "use-rustls"))]
        Transport::Tls { .. } => {
            panic!("Need to enable use-rustls feature to use tls");
        }
        #[cfg(feature = "websocket")]
        Transport::Ws => {
            eprintln!("[bridge] WS: connecting TCP to {}", addr);
            let tcp = TcpStream::connect(addr).await?;
            eprintln!("[bridge] WS: TCP connected, starting WebSocket handshake");
            let ws_path = config.ws_path.as_deref().unwrap_or("/mqtt");
            let url = format!("ws://{}{}", addr, ws_path);
            eprintln!("[bridge] WS: url={}", url);
            let mut request = url.into_client_request().map_err(|e| {
                io::Error::new(io::ErrorKind::InvalidInput, format!("WebSocket request error: {}", e))
            })?;
            request.headers_mut().insert(
                "Sec-WebSocket-Protocol",
                "mqtt".parse().unwrap(),
            );
            let ws_config = WebSocketConfig {
                max_frame_size: Some(config.connections.max_payload_size),
                ..Default::default()
            };
            let (ws_stream, _response) = client_async_with_config(request, tcp, Some(ws_config))
                .await
                .map_err(|e| io::Error::new(io::ErrorKind::ConnectionRefused, format!("WebSocket handshake error: {}", e)))?;
            eprintln!("[bridge] WS: handshake OK");
            let ws_stream = WsStream::new(ws_stream);
            Ok(Network::new(
                Box::new(ws_stream),
                config.connections.max_payload_size,
                config.connections.max_inflight_count,
                protocol,
            ))
        }
        #[cfg(not(feature = "websocket"))]
        Transport::Ws => {
            panic!("Need to enable websocket feature to use ws transport");
        }
        #[cfg(all(feature = "use-rustls", feature = "websocket"))]
        Transport::Wss { ca, client_auth } => {
            let tcp = TcpStream::connect(addr).await?;
            let host = addr.split(':').next().unwrap();
            let tls_stream = tls_connect(host, &ca, client_auth, tcp).await?;

            let ws_path = config.ws_path.as_deref().unwrap_or("/mqtt");
            let url = format!("wss://{}{}", addr, ws_path);
            let mut request = url.into_client_request().map_err(|e| {
                io::Error::new(io::ErrorKind::InvalidInput, format!("WebSocket request error: {}", e))
            })?;
            request.headers_mut().insert(
                "Sec-WebSocket-Protocol",
                "mqtt".parse().unwrap(),
            );
            let ws_config = WebSocketConfig {
                max_frame_size: Some(config.connections.max_payload_size),
                ..Default::default()
            };
            let (ws_stream, _response) = client_async_with_config(request, tls_stream, Some(ws_config))
                .await
                .map_err(|e| io::Error::new(io::ErrorKind::ConnectionRefused, format!("WSS handshake error: {}", e)))?;
            let ws_stream = WsStream::new(ws_stream);
            Ok(Network::new(
                Box::new(ws_stream),
                config.connections.max_payload_size,
                config.connections.max_inflight_count,
                protocol,
            ))
        }
        #[cfg(not(all(feature = "use-rustls", feature = "websocket")))]
        Transport::Wss { .. } => {
            panic!("Need to enable both use-rustls and websocket features to use wss transport");
        }
    }
}

#[cfg(feature = "use-rustls")]
pub async fn tls_connect<P: AsRef<Path>>(
    host: &str,
    ca_file: P,
    client_auth_opt: &Option<ClientAuth>,
    tcp: TcpStream,
) -> Result<Box<dyn N>, BridgeError> {
    let mut root_cert_store = RootCertStore::empty();

    for cert in rustls_pemfile::certs(&mut BufReader::new(Cursor::new(fs::read(ca_file)?))) {
        root_cert_store.add(cert?)?;
    }

    if root_cert_store.is_empty() {
        return Err(BridgeError::NoValidCertInChain);
    }

    let config = ClientConfig::builder().with_root_certificates(root_cert_store);

    let config = if let Some(ClientAuth {
        certs: certs_path,
        key: key_path,
    }) = client_auth_opt
    {
        let certs = rustls_pemfile::certs(&mut BufReader::new(Cursor::new(fs::read(certs_path)?)))
            .collect::<Result<Vec<_>, _>>()?;

        let key = loop {
            match rustls_pemfile::read_one(&mut BufReader::new(Cursor::new(fs::read(key_path)?)))? {
                Some(Item::Pkcs1Key(key)) => break key.into(),
                Some(Item::Pkcs8Key(key)) => break key.into(),
                Some(Item::Sec1Key(key)) => break key.into(),
                None => return Err(BridgeError::NoValidCertInChain),
                _ => {}
            };
        };

        config.with_client_auth_cert(certs, key)?
    } else {
        config.with_no_client_auth()
    };

    let connector = TlsConnector::from(Arc::new(config));
    let domain = ServerName::try_from(host)?.to_owned();
    Ok(Box::new(connector.connect(domain, tcp).await?))
}

async fn network_init<P: Protocol>(
    config: &BridgeConfig,
    network: &mut Network<P>,
) -> Result<(), BridgeError> {
    // Use ping_delay as keep_alive so the remote server's keepalive timeout
    // (1.5x keep_alive) is consistent with our actual ping interval.
    let keep_alive = config.ping_delay as u16;
    let connect = Connect {
        keep_alive,
        client_id: config.name.clone(),
        clean_session: true,
    };

    let login = config.username.as_ref().map(|u| Login {
        username: u.clone(),
        password: config.password.clone().unwrap_or_default(),
    });

    eprintln!("[bridge] network_init: sending CONNECT client_id='{}' keep_alive={} login={:?}",
        config.name, keep_alive, login.as_ref().map(|l| &l.username));

    let packet = Packet::Connect(connect, None, None, None, login);

    send_and_recv(network, packet, |packet| {
        match &packet {
            Packet::ConnAck(connack, _) => {
                eprintln!("[bridge] CONNACK received: code={:?}", connack.code);
                true
            }
            other => {
                eprintln!("[bridge] Expected CONNACK, got: {:?}", other);
                false
            }
        }
    })
    .await?;

    // connecting to other router
    let qos = match config.qos {
        0 => QoS::AtMostOnce,
        1 => QoS::AtLeastOnce,
        2 => QoS::ExactlyOnce,
        _ => return Err(BridgeError::InvalidQos),
    };

    let filters = vec![protocol::Filter {
        path: config.sub_path.clone(),
        qos,
        nolocal: false,
        preserve_retain: false,
        retain_forward_rule: RetainForwardRule::Never,
    }];

    let subscribe = Subscribe { pkid: 0, filters };
    let packet = Packet::Subscribe(subscribe, None);
    send_and_recv(network, packet, |packet| {
        matches!(packet, Packet::SubAck(..))
    })
    .await
}

async fn send_and_recv<F: FnOnce(Packet) -> bool, P: Protocol>(
    network: &mut Network<P>,
    send_packet: Packet,
    accept_recv: F,
) -> Result<(), BridgeError> {
    network.write(send_packet).await?;
    match accept_recv(network.read().await?) {
        true => Ok(()),
        false => Err(BridgeError::InvalidPacket),
    }
}

#[derive(Debug, thiserror::Error)]
pub enum BridgeError {
    #[error("Addr - {0}")]
    Addr(#[from] AddrParseError),
    #[error("I/O - {0}")]
    Io(#[from] io::Error),
    #[error("Network - {0}")]
    Network(#[from] network::Error),
    #[error("Web Pki - {0}")]
    #[cfg(feature = "use-rustls")]
    WebPki(#[from] webpki::Error),
    #[error("DNS name - {0}")]
    #[cfg(feature = "use-rustls")]
    DNSName(#[from] InvalidDnsNameError),
    #[error("TLS error - {0}")]
    #[cfg(feature = "use-rustls")]
    Tls(#[from] TLSError),
    #[error("local link - {0}")]
    Link(#[from] LinkError),
    #[error("Invalid qos")]
    InvalidQos,
    #[error("Invalid packet")]
    InvalidPacket,
    #[cfg(feature = "use-rustls")]
    #[error("Invalid trust_anchor")]
    NoValidCertInChain,
}
