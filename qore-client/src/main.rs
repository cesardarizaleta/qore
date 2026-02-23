use std::net::SocketAddr;
use tokio::net::UdpSocket;

const MAX_DATAGRAM_SIZE: usize = 1350;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = quiche::Config::new(quiche::PROTOCOL_VERSION)?;
    
    // We don't verify the cert for this local test
    config.verify_peer(false);
    
    config.set_application_protos(&[b"qore"])?;
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

    let scid = quiche::ConnectionId::from_ref(&[0xba; quiche::MAX_CONN_ID_LEN]);
    let local_addr: SocketAddr = "0.0.0.0:0".parse()?;
    let peer_addr: SocketAddr = "127.0.0.1:4433".parse()?;

    let socket = UdpSocket::bind(local_addr).await?;
    
    let mut conn = quiche::connect(None, &scid, local_addr, peer_addr, &mut config)?;
    
    let mut out = [0; MAX_DATAGRAM_SIZE];
    let (write, send_info) = conn.send(&mut out)?;
    
    println!("Sending {} bytes to {}", write, send_info.to);
    socket.send_to(&out[..write], send_info.to).await?;

    let mut buf = [0; 65535];
    let mut stream_sent = false;
    
    loop {
        let (len, src) = socket.recv_from(&mut buf).await?;
        println!("Received {} bytes from {}", len, src);
        
        let read = conn.recv(&mut buf[..len], quiche::RecvInfo {
            to: local_addr,
            from: src,
        })?;
        
        println!("Processed {} bytes", read);
        
        if conn.is_established() && !stream_sent {
            println!("QUIC connection established successfully!");
            
            // Send some data on stream 4
            let data = b"Hello from Qore Client!";
            conn.stream_send(4, data, true)?;
            println!("Sent data on stream 4");
            stream_sent = true;
        }

        // Read data from streams
        if conn.is_established() {
            for stream_id in conn.readable() {
                let mut stream_buf = [0; 65535];
                while let Ok((read, fin)) = conn.stream_recv(stream_id, &mut stream_buf) {
                    let data = &stream_buf[..read];
                    println!("Received data from server on stream {}: {}", stream_id, String::from_utf8_lossy(data));
                    
                    if fin {
                        println!("Stream {} finished", stream_id);
                    }
                }
            }
        }
        
        // Send any pending packets
        loop {
            let (write, send_info) = match conn.send(&mut out) {
                Ok(v) => v,
                Err(quiche::Error::Done) => break,
                Err(e) => {
                    eprintln!("conn.send failed: {:?}", e);
                    break;
                }
            };

            socket.send_to(&out[..write], send_info.to).await?;
        }

        if conn.is_closed() {
            println!("Connection closed");
            break;
        }
    }

    Ok(())
}
