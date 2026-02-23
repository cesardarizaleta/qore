#![deny(clippy::all)]

use napi::bindgen_prelude::*;
use napi::threadsafe_function::{ThreadsafeFunction, ThreadsafeFunctionCallMode};
use napi_derive::napi;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use tokio::sync::Mutex;

const MAX_DATAGRAM_SIZE: usize = 1350;

// Estructura para enviar eventos a Node.js
pub enum ServerEvent {
    Connection(String),
    Data(String, u64, Vec<u8>), // peer_addr, stream_id, data
    Closed(String),
}

// Estructura para recibir comandos desde Node.js
pub enum ServerCommand {
    SendData(String, u64, Vec<u8>), // peer_addr, stream_id, data
}

#[napi(object)]
pub struct QoreEvent {
    pub event_type: String,
    pub peer: String,
    pub stream_id: Option<u32>,
    pub data: Option<Uint8Array>,
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
        self.tx.send(ServerCommand::SendData(peer, stream_id as u64, vec_data))
            .await
            .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to send command: {}", e)))?;
        Ok(())
    }
}

#[napi]
pub async fn start_server(
    port: u32,
    cert_path: String,
    key_path: String,
    #[napi(ts_arg_type = "(event: QoreEvent) => void")]
    callback: ThreadsafeFunction<QoreEvent>,
) -> Result<QoreServerHandle> {
    let mut config = quiche::Config::new(quiche::PROTOCOL_VERSION)
        .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to create quiche config: {}", e)))?;

    config.load_cert_chain_from_pem_file(&cert_path)
        .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to load cert: {}", e)))?;
    config.load_priv_key_from_pem_file(&key_path)
        .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to load key: {}", e)))?;

    config.set_application_protos(&[b"qore"]).unwrap();
    config.set_max_idle_timeout(5000);
    config.set_max_recv_udp_payload_size(MAX_DATAGRAM_SIZE);
    config.set_max_send_udp_payload_size(MAX_DATAGRAM_SIZE);
    config.set_initial_max_data(10_000_000);
    config.set_initial_max_stream_data_bidi_local(1_000_000);
    config.set_initial_max_stream_data_bidi_remote(1_000_000);
    config.set_initial_max_stream_data_uni(1_000_000);
    config.set_initial_max_streams_bidi(100);
    config.set_initial_max_streams_uni(100);
    config.set_disable_active_migration(true);

    let addr: SocketAddr = format!("0.0.0.0:{}", port).parse().unwrap();
    let socket = UdpSocket::bind(addr).await
        .map_err(|e| Error::new(Status::GenericFailure, format!("Failed to bind socket: {}", e)))?;

    println!("Qore Server listening on {}", addr);

    let socket = Arc::new(socket);
    let mut buf = [0; 65535];
    let mut out = [0; MAX_DATAGRAM_SIZE];

    // Canal para recibir comandos desde Node.js
    let (tx, mut rx) = mpsc::channel::<ServerCommand>(100);

    // Mapa para guardar las conexiones activas
    let mut connections: HashMap<quiche::ConnectionId<'static>, quiche::Connection> = HashMap::new();
    // Mapa inverso para buscar por IP:Puerto
    let mut peer_to_conn_id: HashMap<String, quiche::ConnectionId<'static>> = HashMap::new();

    let handle = tokio::spawn(async move {
        loop {
            tokio::select! {
                // Recibir paquetes UDP
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

                    // Buscar la conexión o crear una nueva si es un paquete Initial
                    let conn = if let Some(c) = connections.get_mut(&conn_id) {
                        c
                    } else if hdr.ty == quiche::Type::Initial {
                        let scid = quiche::ConnectionId::from_ref(&[0xba; quiche::MAX_CONN_ID_LEN]);
                        let new_conn = match quiche::accept(&scid, None, local_addr, src, &mut config) {
                            Ok(v) => v,
                            Err(e) => {
                                eprintln!("quiche::accept failed: {:?}", e);
                                continue;
                            }
                        };
                        
                        // Notificar a Node.js de la nueva conexión
                        callback.call(
                            Ok(QoreEvent {
                                event_type: "connection".to_string(),
                                peer: src.to_string(),
                                stream_id: None,
                                data: None,
                            }),
                            ThreadsafeFunctionCallMode::NonBlocking,
                        );

                        connections.insert(conn_id.clone(), new_conn);
                        peer_to_conn_id.insert(src.to_string(), conn_id.clone());
                        connections.get_mut(&conn_id).unwrap()
                    } else {
                        // Paquete para una conexión desconocida
                        continue;
                    };

                    // Procesar el paquete recibido
                    let _read = match conn.recv(&mut buf[..len], quiche::RecvInfo {
                        to: local_addr,
                        from: src,
                    }) {
                        Ok(v) => v,
                        Err(e) => {
                            // Ignore CryptoFail errors as they are common during handshake
                            if !matches!(e, quiche::Error::CryptoFail) {
                                eprintln!("conn.recv failed: {:?}", e);
                            }
                            continue;
                        }
                    };

                    // Leer datos de los streams
                    if conn.is_established() {
                        for stream_id in conn.readable() {
                            let mut stream_buf = [0; 65535];
                            while let Ok((read, _fin)) = conn.stream_recv(stream_id, &mut stream_buf) {
                                let data = stream_buf[..read].to_vec();
                                
                                // Enviar los datos a Node.js usando Uint8Array para evitar copias innecesarias
                                // N-API Uint8Array::new toma ownership del Vec<u8> y lo expone a JS sin copiar
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

                    // Enviar paquetes pendientes
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

                    // Limpiar conexiones cerradas
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
                
                // Recibir comandos desde Node.js
                cmd = rx.recv() => {
                    if let Some(ServerCommand::SendData(peer, stream_id, data)) = cmd {
                        if let Some(conn_id) = peer_to_conn_id.get(&peer) {
                            if let Some(conn) = connections.get_mut(conn_id) {
                                if conn.is_established() {
                                    if let Err(e) = conn.stream_send(stream_id, &data, true) {
                                        eprintln!("Failed to send data on stream {}: {:?}", stream_id, e);
                                    }
                                    
                                    // Enviar paquetes pendientes después de encolar los datos
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

    // No esperamos al handle aquí porque bloquearía el Event Loop de Node.js
    // En su lugar, devolvemos el handle para que Node.js pueda enviar comandos
    Ok(QoreServerHandle { tx })
}
