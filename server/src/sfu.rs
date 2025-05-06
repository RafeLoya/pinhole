use std::error::Error;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tokio::task;

use common::logger::Logger;
use crate::sessions::{Message, SessionManager};

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
    session_manager: Arc<Mutex<SessionManager>>,
}

impl SFU {
    pub fn new(
        tcp_addr: String,
        udp_addr: String,
        log_file: String,
        verbose: bool,
    ) -> Self {

        Self {
            tcp_addr,
            udp_addr,
            log_file,
            verbose,
            session_manager: Arc::new(Mutex::new(SessionManager::new())),
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

        // frame data
        let udp_socket = UdpSocket::bind(&self.udp_addr)?;
        let udp_socket = Arc::new(udp_socket);
        let udp_logger = Logger::with_file_name(&self.log_file)?;
        let udp_session_manager = self.session_manager.clone();

        // UDP handler
        task::spawn(async move {
            if let Err(e) = Self::handle_udp_frames(udp_socket, udp_session_manager, udp_logger).await {
                eprintln!("UDP handler error: {}", e)
            }
        });

        // control messages
        let tcp_listener = TcpListener::bind(&self.tcp_addr)?;
        logger.info(&format!("TCP control channel listening on: {}", self.tcp_addr))?;

        // accept tcp connections for control channel
        loop {
            let (socket, addr) = tcp_listener.accept()?;
            logger.info(&format!("new TCP control connection from: {}", addr))?;

            let tcp_logger = Logger::with_file_name(&self.log_file)?;
            let tcp_session_manager = self.session_manager.clone();

            task::spawn(async move {
                if let Err(e) = Self::handle_tcp_control(socket, addr, tcp_session_manager, tcp_logger).await {
                    eprintln!("TCP control error for {}: {}", addr, e);
                }
            });
        }
    }
    
    /// Given a valid received datagram, forward it to the correct client
    /// based on prior TCP-to-UDP registration
    async fn handle_udp_frames(
        socket: Arc<UdpSocket>,
        session_manager: Arc<Mutex<SessionManager>>,
        logger: Logger,
    ) -> Result<(), Box<dyn Error>> {

        let mut buf = vec![0u8; 65536];

        loop {
            let (len, src) = socket.recv_from(&mut buf)?;
            println!("received UDP frame from {} ({} bytes)", src, len);

            if len < 16 {
                logger.warn(&format!("received invalid frame size ({} bytes) from {}", len, src))?;
                continue;
            }

            let peer_udp_addr = {
                let sm = session_manager.lock().unwrap();

                if let Some(tcp_addr) = sm.udp_to_tcp.get(&src) {
                    if let Some(session) = sm.get_session_for_client(tcp_addr) {
                        //session.get_peer_udp(&src)
                        println!("Mapping UDP {} → TCP {}", src, tcp_addr);
                        println!("Resolved peer UDP: {:?}", session.get_peer_udp(tcp_addr));
                        session.get_peer_udp(tcp_addr)
                    } else {
                        None
                    }
                } else {
                    None
                }
            };

            if let Some(peer_addr) = peer_udp_addr {
                match socket.send_to(&buf[0..len], peer_addr) {
                    Ok(_) => {
                        logger.info(&format!("forwarded frame of size {} from {} to {}", len, src, peer_addr))?;
                    }
                    Err(e) => {
                        logger.warn(&format!("failed to forward frame to {}: {}", peer_addr, e))?;
                    }
                }
            } else {
                logger.warn(&format!("failed to forward frame from {}: no peer found", src))?;
            }
        }
    }

    /// Handles forwarding control message between clients,
    /// along with session creation and peer notifications
    async fn handle_tcp_control(
        mut socket: TcpStream,
        addr: SocketAddr,
        session_manager: Arc<Mutex<SessionManager>>,
        logger: Logger,
    ) -> Result<(), Box<dyn Error>> {

        // channel for sending messages to this client
        let (tx, mut rx) = mpsc::unbounded_channel::<Message>();

        // handle messages sent to this client
        let mut send_socket = socket.try_clone()?;
        task::spawn(async move {
            while let Some(msg) = rx.recv().await {
                match msg {
                    Message::AsciiFrame(_) => {
                        // frame data not sent over TCP
                        continue;
                    },
                    Message::Connect(msg) => {
                        println!("sending CONNECTED to client {}", addr);
                        if let Err(e) = send_socket.write_all(b"CONNECTED\n") {
                            eprintln!("failed to send connect message: {}", e);
                            break;
                        }
                    },
                    Message::Disconnect => {
                        // send disconnect notification
                        //logger.info("DISCONNECT received").expect("TODO: panic message");
                        if let Err(e) = send_socket.write_all(b"DISCONNECTED\n") {
                            eprintln!("failed to send disconnect message: {}", e);
                            break;
                        }
                    }
                }
            }
        });

        // read control messages
        let mut buffer = vec![0u8; 1024];
        loop {
            let n = socket.read(&mut buffer)?;
            if n == 0 {
                // connection closed
                logger.info(&format!("TCP control connection closed: {}", addr))?;
                break;
            }

            let cmd = String::from_utf8_lossy(&buffer[..n]);
            let parts: Vec<&str> = cmd.trim().splitn(2, ' ').collect();

            if parts.is_empty() {
                continue;
            }

            // not liking the strings :/
            match parts[0] {
                "JOIN" => {
                    if parts.len() < 2 {
                        socket.write_all(b"ERROR: usage: JOIN <session_id>\n")?;
                        continue;
                    }

                    let session_id = parts[1];

                    // create the session if it doesn't exist
                    {
                        let mut sm  = session_manager.lock().unwrap();
                        sm.create_session(session_id.to_string());
                    }

                    // attempt to join session
                    let join_result = {
                        let mut sm =  session_manager.lock().unwrap();
                        sm.add_client_to_session(session_id, addr,  tx.clone())
                    };

                    if join_result {
                        logger.info(&format!("client {} joined session {}", addr, session_id))?;
                        socket.write_all(b"OK: joined session\n")?;

                        // notify peer about connection
                        {
                            let sm =  session_manager.lock().unwrap();
                            if let Some(session) = sm.get_session_for_client(&addr) {
                                if let Some(peer_tx) = session.get_peer_tx(&addr) {
                                    println!("sending CONNECTED notification from {} to peer", addr);
                                    let _ = peer_tx.send(Message::Connect(session_id.to_string()));
                                }
                            }
                        }
                    } else {
                        logger.warn(&format!("client {} failed to join client session {}", addr, session_id))?;
                        socket.write_all(b"ERROR: failed to join session\n")?;
                    }
                },
                "LEAVE" => {
                    // remove client from its session
                    {
                        let mut sm  = session_manager.lock().unwrap();

                        // notify peer about disconnect
                        if let Some(session) = sm.get_session_for_client(&addr) {
                            if let Some(peer_tx) = session.get_peer_tx(&addr) {
                                let _ = peer_tx.send(Message::Disconnect);
                            }
                        }

                        sm.remove_client(&addr);
                    }

                    logger.info(&format!("client {} left session", addr))?;
                    socket.write_all(b"OK: left session\n")?;
                },
                "REGISTER_UDP" => {
                    if parts.len() < 2 {
                        socket.write_all(b"ERROR: usage: REGISTER_UDP <port>\n")?;
                        continue;
                    }

                    let udp_port = match parts[1].parse::<u16>() {
                        Ok(port) => port,
                        Err(_) => {
                            socket.write_all(b"ERROR: invalid UDP port\n")?;
                            continue;
                        }
                    };
                    let udp_addr = SocketAddr::new(addr.ip(), udp_port);

                    let mut sm = session_manager.lock().unwrap();
                    if let Some(session) = sm.get_session_for_client_mut(&addr) {
                        if session.client_1.as_ref().map(|(a, _)| a) == Some(&addr) {
                            session.client_1_udp = Some(udp_addr);
                        } else if session.client_2.as_ref().map(|(a, _)| a) == Some(&addr) {
                            session.client_2_udp = Some(udp_addr);
                        }

                        sm.udp_to_tcp.insert(udp_addr, addr);
                        socket.write_all(b"OK: register UDP port\n")?;
                    } else {
                        socket.write_all(b"ERROR: no session found\n")?;
                    }
                }
                _ => {
                    socket.write_all(b"ERROR: unknown command\n")?;
                }
            }
        }

        // clean up after closed connection
        {
            let mut sm  = session_manager.lock().unwrap();

            if let Some(session) = sm.get_session_for_client(&addr) {
                if let Some(peer_tx) = session.get_peer_tx(&addr) {
                    let _ = peer_tx.send(Message::Disconnect);
                }
            }

            sm.remove_client(&addr);
        }

        Ok(())
    }
}