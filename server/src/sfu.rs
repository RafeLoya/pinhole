use std::error::Error;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpStream, UdpSocket};
use tokio::sync::mpsc;
use tokio::{select, task};

use crate::sessions::{Message, SessionManager};
use common::logger::Logger;

// TODO: JSON structuring vs regular sentence!

/// Server acting as a Selective Forwarding Unit for connected clients,  
/// responsible for session control (TCP) and frame forwarding (UDP)  
pub struct SFU {
    /// Address for sending control messages to clients  
    tcp_addr: String,
    /// Address for forwarding frame datagrams between peers  
    udp_addr: String,
    /// Record of server activity  
    log_file: String,
    /// Option to have a finer level of detail in the log file  
    verbose: bool,
    /// Thread-safe session manager for client/session tracking  
    sessions: Arc<SessionManager>,
}

impl SFU {
    pub fn new(tcp_addr: String, udp_addr: String, log_file: String, verbose: bool) -> Self {
        Self {
            tcp_addr,
            udp_addr,
            log_file,
            verbose,
            sessions: Arc::new(SessionManager::new()),
        }
    }

    /// Starts SFU, which does the following:  
    /// - Binds UDP and TCP sockets    
    /// - Spawns handler threads for both protocols    
    /// - Continuously accepts TCP connections for control    
    pub async fn run(&self) -> Result<(), Box<dyn Error>> {
        let logger = Logger::with_file_name(&self.log_file)?;
        logger.info("starting SFU server for ASCII video streaming")?;
        if self.verbose {
            println!("SFU server starting with configurations:");
            println!(" - TCP control address: {}", self.tcp_addr);
            println!(" - UDP data address: {}", self.udp_addr);
            println!(" - Log file: {}", self.log_file);
        } else {
            println!("SFU server starting...");
        }

        // === UDP TASK ===========================================================================
        let udp = UdpSocket::bind(&self.udp_addr).await?;
        let udp_sessions = self.sessions.clone();
        task::spawn(Self::udp_loop(udp, udp_sessions));

        let tcp_listener = tokio::net::TcpListener::bind(&self.tcp_addr).await?;
        logger.info(&format!(
            "TCP control channel listening on: {}",
            self.tcp_addr
        ))?;

        // === TCP CONTROL TASK ===================================================================
        loop {
            let (socket, addr) = tcp_listener.accept().await?;
            logger.info(&format!("new TCP control connection from: {}", addr))?;

            let sessions = self.sessions.clone();
            task::spawn(async move {
                if let Err(e) = Self::handle_client(socket, addr, sessions).await {
                    eprintln!("connection {} error: {}", addr, e);
                }
            });
        }
    }

    async fn handle_client(
        socket: TcpStream,
        addr: SocketAddr,
        sessions: Arc<SessionManager>,
    ) -> Result<(), Box<dyn Error>> {
        let (mut rd, mut wr) = socket.into_split();

        let (peer_tx, mut peer_rx) = mpsc::unbounded_channel::<Message>();

        let mut cmd_buf = vec![0u8; 1024];
        loop {
            select! {
                    // session notifications
                    Some(msg) = peer_rx.recv() => {
                        let line: &str = match msg {
                            Message::Connect(_) => "CONNECTED\n",
                            Message::Disconnect => "DISCONNECTED\n",
                            _ => continue
                        };
                        println!("Sending to {}: {}", addr, line.trim());
                        wr.write_all(line.as_bytes()).await?;
                    }
                    result = rd.read(&mut cmd_buf) => {
                        let n = result?;
                        if n == 0 {
                            // client has closed connection
                            break;
                        }
                        let line = std::str::from_utf8(&cmd_buf[..n])?.trim();
                        let mut parts = line.split_whitespace();
                        match parts.next() {
                            Some("JOIN") => {
                                if let Some(id) = parts.next() {
                                    sessions.ensure_session(id).await;
                                    if sessions.add_client(id.clone(), addr, peer_tx.clone()).await {
                                        println!("Sending to {}: OK: joined session", addr);
                                        wr.write_all(b"OK: joined session\n").await?;
                                    } else {
                                        println!("Sending to {}: ERROR: session full", addr);
                                        wr.write_all(b"ERROR: session full\n").await?;
                                    }
                                }
                            }
                            Some("LEAVE") => {
                                sessions.notify_peer(&addr, Message::Disconnect).await;
                                sessions.remove_client(&addr).await;
                                println!("Sending to {}: OK: left session", addr);
                                wr.write_all(b"OK: left session\n").await?;
                            }
                            _ => {
                                // println!("Sending to {}: ERROR: unknown command", addr);
                                // wr.write_all(b"ERROR: unknown command\n").await?;
                        }
                    }
                }
            }
        }

        sessions.notify_peer(&addr, Message::Disconnect).await;
        sessions.remove_client(&addr).await;
        Ok(())
    }

    pub async fn udp_loop(socket: UdpSocket, sessions: Arc<SessionManager>) {
        let mut buf = vec![0u8; 65536];

        loop {
            let (n, src_udp) = match socket.recv_from(&mut buf).await {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("udp recv error: {e}");
                    continue;
                }
            };
            println!("<< got {} bytes from UDP src: {}", n, src_udp);

            sessions.map_udp_to_tcp(src_udp).await;
            if let Some(dst_udp) = sessions.get_peer_udp(&src_udp).await {
                if let (Some(src_tcp), Some(dst_tcp)) = (
                    sessions.tcp_for_udp(&src_udp).await,
                    sessions.tcp_for_udp(&dst_udp).await,
                ) {
                    if let Some(session_id) = sessions.session_id_for(&dst_tcp).await {
                        if !sessions.is_connected(&session_id).await {
                            sessions
                                .notify_peer(&src_tcp, Message::Connect(session_id.clone()))
                                .await;
                            sessions
                                .notify_peer(&dst_tcp, Message::Connect(session_id.clone()))
                                .await;
                            sessions.mark_connected(&session_id).await;
                        }
                    }
                }

                match socket.send_to(&buf[..n], &dst_udp).await {
                    Ok(sent) => println!("forwarded {sent} bytes {src_udp} -> {dst_udp}"),
                    Err(e) => eprintln!("udp send error {dst_udp}: {e}"),
                }
            } else {
                eprintln!("no peer for UDP {src_udp}")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::net::TcpStream;
    use tokio::time::sleep;

    fn init_sfu() -> SFU {
        SFU::new(
            "127.0.0.1:8043".to_string(),
            "127.0.0.1:8044".to_string(),
            "sfu_test.log".to_string(),
            false,
        )
    }

    #[tokio::test]
    async fn test_tcp_join_and_leave() {
        let sfu = init_sfu();

        // Start the SFU server in background
        tokio::spawn(async move {
            sfu.run().await.expect("SFU run failed");
        });

        // Let the server start
        sleep(Duration::from_millis(100)).await;

        // Connect to TCP
        let mut stream = TcpStream::connect("127.0.0.1:8043").await.unwrap();

        // Send JOIN command
        stream.write_all(b"JOIN room1\n").await.unwrap();
        let mut response = vec![0u8; 1024];
        let n = stream.read(&mut response).await.unwrap();
        let text = String::from_utf8_lossy(&response[..n]);
        assert!(text.contains("OK: joined session"));

        // Send LEAVE command
        stream.write_all(b"LEAVE\n").await.unwrap();
        let n = stream.read(&mut response).await.unwrap();
        let text = String::from_utf8_lossy(&response[..n]);
        assert!(text.contains("OK: left session"));
    }
}
