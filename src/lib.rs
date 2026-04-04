#![deny(clippy::all)]

use napi::bindgen_prelude::*;
use napi::threadsafe_function::{ThreadsafeFunction, ThreadsafeFunctionCallMode};
use napi_derive::napi;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;

const MAX_DATAGRAM_SIZE: usize = 1350;

// ── Events sent to Node.js ──────────────────────────────────────────

#[napi(object)]
pub struct QoreEvent {
    pub event_type: String,
    pub peer: String,
    pub stream_id: Option<u32>,
    pub data: Option<Uint8Array>,
}

// ══════════════════════════════════════════════════════════════════════
//  SERVER
// ══════════════════════════════════════════════════════════════════════

pub enum ServerCommand {
    SendData(String, u64, Vec<u8>),
}

#[napi]
pub struct QoreServerHandle {
    tx: mpsc::Sender<ServerCommand>,
}

#[napi]
impl QoreServerHandle {
    #[napi]
    pub async fn send_data(&self, peer: String, stream_id: u32, data: Uint8Array) -> Result<()> {
        let vec_data = data.to_vec();
        self.tx
            .send(ServerCommand::SendData(peer, stream_id as u64, vec_data))
            .await
            .map_err(|e| {
                Error::new(
                    Status::GenericFailure,
                    format!("Failed to send command: {}", e),
                )
            })?;
        Ok(())
    }
}

#[napi]
pub async fn start_server(
    port: u32,
    cert_path: String,
    key_path: String,
    #[napi(ts_arg_type = "(event: QoreEvent) => void")] callback: ThreadsafeFunction<QoreEvent>,
) -> Result<QoreServerHandle> {
    let mut config = quiche::Config::new(quiche::PROTOCOL_VERSION).map_err(|e| {
        Error::new(
            Status::GenericFailure,
            format!("Failed to create quiche config: {}", e),
        )
    })?;

    config.load_cert_chain_from_pem_file(&cert_path).map_err(|e| {
        Error::new(
            Status::GenericFailure,
            format!("Failed to load cert: {}", e),
        )
    })?;
    config.load_priv_key_from_pem_file(&key_path).map_err(|e| {
        Error::new(
            Status::GenericFailure,
            format!("Failed to load key: {}", e),
        )
    })?;

    config.set_application_protos(&[b"qore"]).unwrap();
    config.set_max_idle_timeout(30000);
    config.set_max_recv_udp_payload_size(MAX_DATAGRAM_SIZE);
    config.set_max_send_udp_payload_size(MAX_DATAGRAM_SIZE);
    config.set_initial_max_data(10_000_000);
    config.set_initial_max_stream_data_bidi_local(1_000_000);
    config.set_initial_max_stream_data_bidi_remote(1_000_000);
    config.set_initial_max_stream_data_uni(1_000_000);
    config.set_initial_max_streams_bidi(100);
    config.set_initial_max_streams_uni(100);
    config.set_disable_active_migration(true);

    let addr: SocketAddr = format!("0.0.0.0:{}", port).parse().map_err(|e| {
        Error::new(
            Status::InvalidArg,
            format!("Invalid port {}: {}", port, e),
        )
    })?;

    let socket = tokio::net::UdpSocket::bind(addr).await.map_err(|e| {
        Error::new(
            Status::GenericFailure,
            format!("Failed to bind socket: {}", e),
        )
    })?;

    let socket = Arc::new(socket);

    let (tx, mut rx) = mpsc::channel::<ServerCommand>(256);

    let _handle = tokio::spawn(async move {
        let mut buf = [0; 65535];
        let mut out = [0; MAX_DATAGRAM_SIZE];
        let mut connections: HashMap<quiche::ConnectionId<'static>, quiche::Connection> =
            HashMap::new();
        let mut peer_to_conn_id: HashMap<String, quiche::ConnectionId<'static>> = HashMap::new();
        let mut conn_counter: u64 = 0;
        let mut timer = tokio::time::interval(std::time::Duration::from_millis(50));

        loop {
            tokio::select! {
                _ = timer.tick() => {
                    let mut closed_ids = Vec::new();
                    for (conn_id, conn) in connections.iter_mut() {
                        conn.on_timeout();
                        loop {
                            let (write, send_info) = match conn.send(&mut out) {
                                Ok(v) => v,
                                Err(quiche::Error::Done) => break,
                                Err(_) => break,
                            };
                            let _ = socket.send_to(&out[..write], send_info.to).await;
                        }
                        if conn.is_closed() {
                            closed_ids.push(conn_id.clone());
                        }
                    }
                    for id in closed_ids {
                        let peer_opt = peer_to_conn_id.iter().find(|(_, val)| *val == &id).map(|(k, _)| k.clone());
                        if let Some(peer) = peer_opt {
                            callback.call(
                                Ok(QoreEvent {
                                    event_type: "closed".to_string(),
                                    peer: peer.clone(),
                                    stream_id: None,
                                    data: None,
                                }),
                                ThreadsafeFunctionCallMode::NonBlocking,
                            );
                            peer_to_conn_id.remove(&peer);
                        }
                        connections.remove(&id);
                    }
                }

                recv_result = socket.recv_from(&mut buf) => {
                    let (len, src) = match recv_result {
                        Ok(v) => v,
                        Err(e) => {
                            eprintln!("Error receiving from socket: {}", e);
                            continue;
                        }
                    };

                    let hdr = match quiche::Header::from_slice(&mut buf[..len], quiche::MAX_CONN_ID_LEN) {
                        Ok(v) => v,
                        Err(e) => {
                            eprintln!("Parsing packet header failed: {:?}", e);
                            continue;
                        }
                    };

                    let conn_id = quiche::ConnectionId::from_vec(hdr.dcid.to_vec());
                    let local_addr = socket.local_addr().unwrap();

                    let conn = if let Some(c) = connections.get_mut(&conn_id) {
                        c
                    } else if hdr.ty == quiche::Type::Initial {
                        // Generate a unique scid for each connection
                        conn_counter += 1;
                        let mut scid_bytes = [0u8; quiche::MAX_CONN_ID_LEN];
                        let counter_bytes = conn_counter.to_be_bytes();
                        let copy_len = std::cmp::min(counter_bytes.len(), scid_bytes.len());
                        scid_bytes[..copy_len].copy_from_slice(&counter_bytes[..copy_len]);
                        let scid = quiche::ConnectionId::from_vec(scid_bytes.to_vec());

                        let new_conn = match quiche::accept(&scid, None, local_addr, src, &mut config) {
                            Ok(v) => v,
                            Err(e) => {
                                eprintln!("quiche::accept failed: {:?}", e);
                                continue;
                            }
                        };

                        callback.call(
                            Ok(QoreEvent {
                                event_type: "connection".to_string(),
                                peer: src.to_string(),
                                stream_id: None,
                                data: None,
                            }),
                            ThreadsafeFunctionCallMode::NonBlocking,
                        );

                        // Store under original dcid AND our scid (subsequent packets use our scid as dcid)
                        connections.insert(scid.clone(), new_conn);
                        peer_to_conn_id.insert(src.to_string(), scid.clone());
                        connections.get_mut(&scid).unwrap()
                    } else {
                        continue;
                    };

                    let _read = match conn.recv(&mut buf[..len], quiche::RecvInfo {
                        to: local_addr,
                        from: src,
                    }) {
                        Ok(v) => v,
                        Err(e) => {
                            if !matches!(e, quiche::Error::CryptoFail) {
                                eprintln!("conn.recv failed: {:?}", e);
                            }
                            continue;
                        }
                    };

                    if conn.is_established() {
                        for stream_id in conn.readable() {
                            let mut stream_buf = [0; 65535];
                            while let Ok((read, _fin)) = conn.stream_recv(stream_id, &mut stream_buf) {
                                let data = stream_buf[..read].to_vec();
                                callback.call(
                                    Ok(QoreEvent {
                                        event_type: "data".to_string(),
                                        peer: src.to_string(),
                                        stream_id: Some(stream_id as u32),
                                        data: Some(Uint8Array::new(data)),
                                    }),
                                    ThreadsafeFunctionCallMode::NonBlocking,
                                );
                            }
                        }
                    }

                    loop {
                        let (write, send_info) = match conn.send(&mut out) {
                            Ok(v) => v,
                            Err(quiche::Error::Done) => break,
                            Err(e) => {
                                eprintln!("conn.send failed: {:?}", e);
                                break;
                            }
                        };
                        if let Err(e) = socket.send_to(&out[..write], send_info.to).await {
                            eprintln!("socket.send_to failed: {:?}", e);
                        }
                    }

                    if conn.is_closed() {
                        callback.call(
                            Ok(QoreEvent {
                                event_type: "closed".to_string(),
                                peer: src.to_string(),
                                stream_id: None,
                                data: None,
                            }),
                            ThreadsafeFunctionCallMode::NonBlocking,
                        );
                        connections.remove(&conn_id);
                        peer_to_conn_id.remove(&src.to_string());
                    }
                }

                cmd = rx.recv() => {
                    if let Some(ServerCommand::SendData(peer, stream_id, data)) = cmd {
                        if let Some(conn_id) = peer_to_conn_id.get(&peer) {
                            if let Some(conn) = connections.get_mut(conn_id) {
                                if conn.is_established() {
                                    if let Err(e) = conn.stream_send(stream_id, &data, true) {
                                        eprintln!("Failed to send data on stream {}: {:?}", stream_id, e);
                                    }
                                    loop {
                                        let (write, send_info) = match conn.send(&mut out) {
                                            Ok(v) => v,
                                            Err(quiche::Error::Done) => break,
                                            Err(e) => {
                                                eprintln!("conn.send failed: {:?}", e);
                                                break;
                                            }
                                        };
                                        if let Err(e) = socket.send_to(&out[..write], send_info.to).await {
                                            eprintln!("socket.send_to failed: {:?}", e);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    });

    Ok(QoreServerHandle { tx })
}

// ══════════════════════════════════════════════════════════════════════
//  CLIENT
// ══════════════════════════════════════════════════════════════════════

pub enum ClientCommand {
    SendOnStream(u64, Vec<u8>, bool),
}

#[napi]
pub struct QoreClientHandle {
    tx: mpsc::Sender<ClientCommand>,
}

#[napi]
impl QoreClientHandle {
    #[napi]
    pub async fn send_on_stream(
        &self,
        stream_id: u32,
        data: Uint8Array,
        fin: bool,
    ) -> Result<()> {
        let vec_data = data.to_vec();
        self.tx
            .send(ClientCommand::SendOnStream(
                stream_id as u64,
                vec_data,
                fin,
            ))
            .await
            .map_err(|e| {
                Error::new(
                    Status::GenericFailure,
                    format!("Failed to send command: {}", e),
                )
            })?;
        Ok(())
    }
}

#[napi]
pub async fn connect_to_server(
    host: String,
    port: u32,
    #[napi(ts_arg_type = "(event: QoreEvent) => void")] callback: ThreadsafeFunction<QoreEvent>,
) -> Result<QoreClientHandle> {
    let mut config = quiche::Config::new(quiche::PROTOCOL_VERSION).map_err(|e| {
        Error::new(
            Status::GenericFailure,
            format!("Failed to create config: {}", e),
        )
    })?;

    // Skip certificate verification for development
    config.verify_peer(false);

    config.set_application_protos(&[b"qore"]).unwrap();
    config.set_max_idle_timeout(30000);
    config.set_max_recv_udp_payload_size(MAX_DATAGRAM_SIZE);
    config.set_max_send_udp_payload_size(MAX_DATAGRAM_SIZE);
    config.set_initial_max_data(10_000_000);
    config.set_initial_max_stream_data_bidi_local(1_000_000);
    config.set_initial_max_stream_data_bidi_remote(1_000_000);
    config.set_initial_max_stream_data_uni(1_000_000);
    config.set_initial_max_streams_bidi(100);
    config.set_initial_max_streams_uni(100);
    config.set_disable_active_migration(true);

    let scid_bytes: Vec<u8> = (0..quiche::MAX_CONN_ID_LEN)
        .map(|_| rand_byte())
        .collect();
    let scid = quiche::ConnectionId::from_vec(scid_bytes);

    let local_addr: SocketAddr = "0.0.0.0:0".parse().unwrap();
    let peer_addr: SocketAddr = format!("{}:{}", host, port)
        .parse()
        .map_err(|e: std::net::AddrParseError| {
            Error::new(Status::GenericFailure, format!("Invalid address: {}", e))
        })?;

    let socket = UdpSocket::bind(local_addr).await.map_err(|e| {
        Error::new(
            Status::GenericFailure,
            format!("Failed to bind socket: {}", e),
        )
    })?;

    let mut conn = quiche::connect(None, &scid, socket.local_addr().unwrap(), peer_addr, &mut config)
        .map_err(|e| {
            Error::new(
                Status::GenericFailure,
                format!("quiche::connect failed: {}", e),
            )
        })?;

    // Send initial handshake packet
    let mut initial_out = [0; MAX_DATAGRAM_SIZE];
    let (write, send_info) = conn.send(&mut initial_out).map_err(|e| {
        Error::new(
            Status::GenericFailure,
            format!("Initial send failed: {}", e),
        )
    })?;
    socket
        .send_to(&initial_out[..write], send_info.to)
        .await
        .map_err(|e| {
            Error::new(
                Status::GenericFailure,
                format!("Socket send failed: {}", e),
            )
        })?;

    let socket = Arc::new(socket);
    let (tx, mut rx) = mpsc::channel::<ClientCommand>(256);

    // Background task: handles the QUIC event loop
    tokio::spawn(async move {
        let mut buf = [0; 65535];
        let mut out = [0; MAX_DATAGRAM_SIZE];
        let mut established_notified = false;
        let mut timer = tokio::time::interval(std::time::Duration::from_millis(50));

        loop {
            tokio::select! {
                _ = timer.tick() => {
                    conn.on_timeout();
                    loop {
                        let (write, send_info) = match conn.send(&mut out) {
                            Ok(v) => v,
                            Err(quiche::Error::Done) => break,
                            Err(_) => break,
                        };
                        let _ = socket.send_to(&out[..write], send_info.to).await;
                    }
                    if conn.is_closed() {
                        callback.call(
                            Ok(QoreEvent {
                                event_type: "closed".to_string(),
                                peer: peer_addr.to_string(),
                                stream_id: None,
                                data: None,
                            }),
                            ThreadsafeFunctionCallMode::NonBlocking,
                        );
                        break;
                    }
                }

                recv_result = socket.recv_from(&mut buf) => {
                    let (len, src) = match recv_result {
                        Ok(v) => v,
                        Err(e) => {
                            eprintln!("Client recv error: {}", e);
                            continue;
                        }
                    };

                    let local = socket.local_addr().unwrap();
                    let _read = match conn.recv(&mut buf[..len], quiche::RecvInfo {
                        to: local,
                        from: src,
                    }) {
                        Ok(v) => v,
                        Err(e) => {
                            if !matches!(e, quiche::Error::CryptoFail) {
                                eprintln!("Client conn.recv failed: {:?}", e);
                            }
                            continue;
                        }
                    };

                    // Notify connection established
                    if conn.is_established() && !established_notified {
                        established_notified = true;
                        callback.call(
                            Ok(QoreEvent {
                                event_type: "connection".to_string(),
                                peer: peer_addr.to_string(),
                                stream_id: None,
                                data: None,
                            }),
                            ThreadsafeFunctionCallMode::NonBlocking,
                        );
                    }

                    // Read data from streams
                    if conn.is_established() {
                        for stream_id in conn.readable() {
                            let mut stream_buf = [0; 65535];
                            while let Ok((read, _fin)) = conn.stream_recv(stream_id, &mut stream_buf) {
                                let data = stream_buf[..read].to_vec();
                                callback.call(
                                    Ok(QoreEvent {
                                        event_type: "data".to_string(),
                                        peer: peer_addr.to_string(),
                                        stream_id: Some(stream_id as u32),
                                        data: Some(Uint8Array::new(data)),
                                    }),
                                    ThreadsafeFunctionCallMode::NonBlocking,
                                );
                            }
                        }
                    }

                    // Flush pending packets
                    loop {
                        let (write, send_info) = match conn.send(&mut out) {
                            Ok(v) => v,
                            Err(quiche::Error::Done) => break,
                            Err(e) => {
                                eprintln!("conn.send failed: {:?}", e);
                                break;
                            }
                        };
                        if let Err(e) = socket.send_to(&out[..write], send_info.to).await {
                            eprintln!("socket.send_to failed: {:?}", e);
                        }
                    }

                    if conn.is_closed() {
                        callback.call(
                            Ok(QoreEvent {
                                event_type: "closed".to_string(),
                                peer: peer_addr.to_string(),
                                stream_id: None,
                                data: None,
                            }),
                            ThreadsafeFunctionCallMode::NonBlocking,
                        );
                        break;
                    }
                }

                cmd = rx.recv() => {
                    if let Some(ClientCommand::SendOnStream(stream_id, data, fin)) = cmd {
                        if conn.is_established() {
                            if let Err(e) = conn.stream_send(stream_id, &data, fin) {
                                eprintln!("Failed to send on stream {}: {:?}", stream_id, e);
                            }
                            loop {
                                let (write, send_info) = match conn.send(&mut out) {
                                    Ok(v) => v,
                                    Err(quiche::Error::Done) => break,
                                    Err(e) => {
                                        eprintln!("conn.send failed: {:?}", e);
                                        break;
                                    }
                                };
                                if let Err(e) = socket.send_to(&out[..write], send_info.to).await {
                                    eprintln!("socket.send_to failed: {:?}", e);
                                }
                            }
                        }
                    }
                }
            }
        }
    });

    Ok(QoreClientHandle { tx })
}

/// Cryptographically secure random byte generator
fn rand_byte() -> u8 {
    use ring::rand::SecureRandom;
    let rng = ring::rand::SystemRandom::new();
    let mut b = [0u8; 1];
    rng.fill(&mut b).unwrap_or_default();
    b[0]
}
