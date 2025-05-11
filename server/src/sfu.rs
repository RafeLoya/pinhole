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
    /// Option to have a finer level of detail in the log file
    verbose: bool,
    /// Thread-safe session manager for client/session tracking
    sessions: Arc<SessionManager>,
    /// Record of server activity
    logger: Arc<Logger>,
}

impl SFU {
    pub fn new(tcp_addr: String, udp_addr: String, log_file: String, verbose: bool) -> Result<Self, Box<dyn Error>> {
        let logger = Arc::new(Logger::with_file_name(&log_file)?);

        Ok(Self {
            tcp_addr,
            udp_addr,
            verbose,
            sessions: Arc::new(SessionManager::new(logger.clone())),
            logger,
        })
    }

    /// Starts SFU, which does the following:
    /// - Binds UDP and TCP sockets
    /// - Spawns handler threads for both protocols
    /// - Continuously accepts TCP connections for control
    pub async fn run(&self) -> Result<(), Box<dyn Error>> {

        let logger = self.logger.clone();

        if self.verbose {
            println!("SFU server starting with configurations:");
            println!("\tTCP control address: {}", self.tcp_addr);
            println!("\tUDP data address: {}", self.udp_addr);
            println!("\tLog file: {}", self.logger.get_log_file());
        }

        logger.info("SFU server starting")?;

        // === UDP TASK ===========================================================================

        let udp = UdpSocket::bind(&self.udp_addr).await?;
        let udp_sessions = self.sessions.clone();
        let udp_logger = logger.clone();
        task::spawn(async move {
            if let Err(e) = Self::udp_loop(udp, udp_sessions, udp_logger.clone()).await {
                udp_logger.error(&format!("UDP loop error: {}", e)).ok();
            }
        });

        // === TCP CONTROL TASK ===================================================================
        let tcp_listener = tokio::net::TcpListener::bind(&self.tcp_addr).await?;
        let tcp_logger = logger.clone(); // clone it here
        loop {
            let (socket, addr) = tcp_listener.accept().await?;
            tcp_logger.info(&format!("New TCP control connection from: {}", addr))?;

            let sessions = self.sessions.clone();
            let conn_logger = tcp_logger.clone(); // <- clone again for the task
            task::spawn(async move {
                if let Err(e) = Self::handle_client(socket, addr, sessions, conn_logger.clone()).await {
                    conn_logger.error(&format!("Connection {} error: {}", addr, e)).ok();
                }
            });
        }
    }

    async fn handle_client(
        socket: TcpStream,
        addr: SocketAddr,
        sessions: Arc<SessionManager>,
        logger: Arc<Logger>,
    ) -> Result<(), Box<dyn Error>> {
        let (mut rd, mut wr) = socket.into_split();
        let (peer_tx, mut peer_rx) = mpsc::unbounded_channel::<Message>();

        let mut cmd_buf = vec![0u8; 1024];
        loop {
            select! {
                Some(msg) = peer_rx.recv() => {
                    let line: &str = match msg {
                        Message::Connect(_) => "CONNECTED\n",
                        Message::Disconnect => "DISCONNECTED\n",
                        _ => continue
                    };
                    logger.info(&format!("Sending to {}: {}", addr, line.trim()))?;
                    wr.write_all(line.as_bytes()).await?;
                }
                result = rd.read(&mut cmd_buf) => {
                    let n = result?;
                    if n == 0 {
                        logger.info(&format!("Client {} closed connection", addr))?;
                        break;
                    }
                    let line = std::str::from_utf8(&cmd_buf[..n])?.trim();
                    logger.info(&format!("Received from {}: {}", addr, line))?;
                    let mut parts = line.split_whitespace();
                    match parts.next() {
                        Some("JOIN") => {
                            if let Some(id) = parts.next() {
                                sessions.ensure_session(id).await;
                                if sessions.add_client(id.clone(), addr, peer_tx.clone()).await {
                                    logger.info(&format!("{} joined session {}", addr, id))?;
                                    wr.write_all(b"OK: joined session\n").await?;
                                } else {
                                    logger.info(&format!("{} failed to join session {} (full)", addr, id))?;
                                    wr.write_all(b"ERROR: session full\n").await?;
                                }
                            }
                        }
                        Some("LEAVE") => {
                            sessions.notify_peer(&addr, Message::Disconnect).await;
                            sessions.remove_client(&addr).await;
                            logger.info(&format!("{} left session", addr))?;
                            wr.write_all(b"OK: left session\n").await?;
                        }
                        _ => {
                            logger.info(&format!("{} sent unknown command", addr))?;
                            wr.write_all(b"ERROR: unknown command\n").await?;
                        }
                    }
                }
            }
        }

        sessions.notify_peer(&addr, Message::Disconnect).await;
        sessions.remove_client(&addr).await;
        logger.info(&format!("{} disconnected", addr))?;
        Ok(())
    }


    pub async fn udp_loop(
        socket: UdpSocket,
        sessions: Arc<SessionManager>,
        logger: Arc<Logger>,
    ) -> Result<(), Box<dyn Error>> {
        let mut buf = vec![0u8; 65536];

        loop {
            let (n, src_udp) = match socket.recv_from(&mut buf).await {
                Ok(v) => v,
                Err(e) => {
                    logger.error(&format!("UDP recv error: {}", e))?;
                    continue;
                }
            };

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
                            logger.info(&format!("Session {} marked connected", session_id))?;
                        }
                    }
                }

                match socket.send_to(&buf[..n], &dst_udp).await {
                    Ok(sent) => {
                        if sent != n {
                            logger.error(&format!("UDP send incomplete: sent {} bytes, expected {}", sent, n))?;
                        }
                    }
                    Err(e) => {
                        logger.error(&format!("UDP send error to {dst_udp}: {e}"))?;
                    }
                }
            } else {
                logger.error(&format!("No peer found for UDP source {src_udp}"))?;
            }
        }
    }

}
