use std::error::Error;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener};
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use tokio::sync::mpsc::UnboundedSender;
use tokio::task;

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
            println!("\tTCP control address: {}", self.tcp_addr);
            println!("\tUDP data address: {}", self.udp_addr);
            println!("\tLog file: {}", self.log_file);
        } else {
            println!("SFU server starting...");
        }

        // === UDP TASK ===========================================================================
        let udp = UdpSocket::bind(&self.udp_addr).await?;
        let udp_sessions = self.sessions.clone();
        task::spawn(Self::udp_loop(udp, udp_sessions));

        // control messages
        let tcp_listener = TcpListener::bind(&self.tcp_addr)?;
        logger.info(&format!(
            "TCP control channel listening on: {}",
            self.tcp_addr
        ))?;

        loop {
            let (mut socket, addr) = match tcp_listener.accept() {
                Ok((s, a)) => (s, a),
                Err(e) => {
                    eprintln!("TCP accept error: {}", e);
                    continue;
                }
            };

            let mut sessions = self.sessions.clone();
            task::spawn(async move {
                let (tx, mut rx) = mpsc::unbounded_channel::<Message>();
                
                let mut writer = socket.try_clone().unwrap();
                task::spawn(async move {
                    while let Some(msg) = rx.recv().await {
                        let _ = match msg {
                            Message::Connect(_) => writer.write_all(b"CONNECTED\n"),
                            Message::Disconnect => writer.write_all(b"DISCONNECTED\n"),
                            Message::AsciiFrame(..) => Ok(()) // frames are NEVER sent on TCP! 
                        };
                    }
                });
                
                // TODO: does semicolon get in the way of control parsing?
                let mut buf = [0u8; 1024];
                loop {
                    let n = match socket.read(&mut buf) {
                        Ok(0) => break,    // closed connection
                        Ok(n) => n,
                        Err(_) => break,
                    };
                    let line = std::str::from_utf8(&buf[..n]).unwrap();
                    let mut parts = line.trim().split_whitespace();
                    match parts.next() {
                        Some("JOIN") => {
                            if let Some(id) = parts.next() {
                                sessions.ensure_session(id).await;
                                if sessions.add_client(id, addr, tx.clone()).await {
                                    let _ = socket.write_all(b"OK: joined session\n");
                                    sessions.notify_peer(&addr, Message::Connect(id.to_owned())).await;
                                    
                                    // if sessions.session_full(id).await {
                                    //     let _ = tx.send(Message::Connect(id.to_owned()));
                                    // }
                                    
                                    // if let Some(peer_udp) = sessions.get_peer_udp_from_tcp(&addr).await {
                                    //     let _ = tx.send(Message::Connect(id.to_owned()));
                                    // }
                                    // if sessions.session_full(id).await {
                                    //     let _ = tx.send(Message::Connect(id.to_owned()));
                                    // }
                                } else {
                                    let _ = socket.write_all(b"ERROR: session full\n");
                                }
                            }
                        }
                        Some("LEAVE") => {
                            sessions.notify_peer(&addr, Message::Disconnect).await;
                            sessions.remove_client(&addr).await;
                            let _ = socket.write_all(b"OK: left session\n");
                        }
                        Some("REGISTER_UDP") => {
                            if let Some(p) = parts.next().and_then(|p| p.parse::<u16>().ok()) {
                                let udp = SocketAddr::new(addr.ip(), p);
                                println!("TCP->REGISTER_UDP from {addr}: {udp}");
                                sessions.register_udp(addr, udp).await;
                                println!(">> mapped UDP {udp} -> TCP {addr}");
                                let _ = socket.write_all(b"OK: registered UDP\n");
                                
                                if let Some(id) = sessions.session_id_for(&addr).await {
                                    if sessions.session_full(&id).await {
                                        let _ = tx.send(Message::Connect(id.to_owned()));
                                    }
                                }
                                // println!("TCP→REGISTER_UDP from {addr}: {udp}");
                                // sessions.register_udp(addr, udp).await;
                                // println!(">> mapped UDP {udp} -> TCP {addr}");
                                // let _ = socket.write_all(b"OK: registered UDP\n");
                            }
                        }
                        _ => {
                            let _ = socket.write_all(b"ERROR\n");
                        }
                    }
                }
                
                sessions.notify_peer(&addr, Message::Disconnect).await;
                sessions.remove_client(&addr).await;
                println!("closed control for {}", addr);
            });
        }
    }

    pub async fn udp_loop(
        socket: UdpSocket,
        sessions: Arc<SessionManager>,
    ) {
        let mut buf = vec![0u8; 65536];
        
        loop {
            let (n, src) = match socket.recv_from(&mut buf).await {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("udp recv error: {e}");
                    continue;
                }
            };
            println!("<< got {} bytes from UDP sr{}", n, src);
            if let Some(dst) = sessions.get_peer_udp(&src).await {
                match socket.send_to(&buf[..n], dst).await {
                    Ok(sent) => {
                        println!("forwarded {} bytes from {} -> {}", sent, src, dst);
                    }
                    Err(e) => {
                        eprintln!("udp send error to {dst}: {e}");
                    }
                }
            } else {
                eprintln!("no peer for UDP {src}");
            }
        }
    }
}
