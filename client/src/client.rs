use std::error::Error;
use std::io::{Read, Write};
use std::net::{TcpStream, UdpSocket};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tokio::task;
use common::ascii_frame::AsciiFrame;
use crate::ascii_renderer::AsciiRenderer;

pub struct Client {
    /// TCP address for 'control' messages (i.e. updates to session state)
    server_tcp_addr: String,
    /// UDP address for receiving frame data from peer client
    server_udp_addr: String,
    /// ID for session, if client is currently in one
    session_id: String,
    /// is client currently in a session?
    is_connected: Arc<Mutex<bool>>,
    /// does client have another peer client in their session?
    has_peer: Arc<Mutex<bool>>,
}

impl Client {
    pub fn new(server_tcp_addr: String, server_udp_addr: String, session_id: String) -> Self {
        Self {
            server_tcp_addr,
            server_udp_addr,
            session_id,
            is_connected: Arc::new(Mutex::new(false)),
            has_peer: Arc::new(Mutex::new(false)),
        }
    }
    
    pub async fn run(&self) -> Result<(), Box<dyn Error>> {
        let mut tcp_stream = TcpStream::connect(&self.server_tcp_addr)?;
        println!("Connected to server at {}", &self.server_tcp_addr);

        let udp_socket = UdpSocket::bind("0.0.0.0:0")?;
        udp_socket.connect(&self.server_tcp_addr)?;
        println!("UDP socket ready to stream frames");

        let join_cmd = format!("JOIN {}\n", self.session_id);
        tcp_stream.write_all(join_cmd.as_bytes())?;

        let mut response = [0u8; 1024];
        let n =  tcp_stream.read(&mut response)?;
        let response = std::str::from_utf8(&response[..n])?;

        if !response.starts_with("OK") {
            return Err(format!("failed to join session: {}", response).into());
        }

        {
            let mut connected = self.is_connected.lock().unwrap();
            *connected = true;
        }

        println!("joined session: {}", self.session_id);

        // cloned references for tasks
        let (frame_tx, frame_rx) = mpsc::channel::<AsciiFrame>(32);
        let udp_socket = Arc::new(udp_socket);
        let tcp_stream = Arc::new(Mutex::new(tcp_stream));

        let tcp_task = {
            let is_connected = self.is_connected.clone();
            let has_peer = self.has_peer.clone();
            let tcp_stream = tcp_stream.clone();

            task::spawn(async move {
                if let Err(e) = Self::handle_tcp_control(tcp_stream, is_connected, has_peer).await {
                    eprintln!("TCP control error: {}", e)
                }
            })
        };
        
        let render_task = {
            let udp_socket = udp_socket.clone();
            let is_connected = self.is_connected.clone();
            let has_peer = self.has_peer.clone();
            
            task::spawn(async move {
                if let Err(e) = Self::receive_and_deserialize_frames(udp_socket, is_connected, has_peer).await {
                    eprintln!("render error: {}", e)
                }
            })
        };
        
        let capture_task = {
            let udp_socket = udp_socket.clone();
            let is_connected = self.is_connected.clone();
            let has_peer = self.has_peer.clone();
            
            task::spawn(async move {
                if let Err(e) = Self::capture_and_serialize_frames(udp_socket, frame_rx, is_connected, has_peer).await {
                    eprintln!("capture error: {}", e)
                }
            })
        };
        
        // WEBCAM LOGIC
        //=====================================================================
        
        while *self.is_connected.lock().unwrap() {
            if *self.has_peer.lock().unwrap() {
                
                let frame = AsciiFrame::from_bytes(3, 1, &[0u8, 0, 0])?;

                if let Err(e) = frame_tx.try_send(frame) {
                    eprintln!("failed to send ascii frame: {}", e);
                }
            }
            
            // originally 10ms
            tokio::time::sleep(tokio::time::Duration::from_millis(33)).await;
        }
        //=====================================================================
        
        // clean up
        let mut tcp = tcp_stream.lock().unwrap();
        let _ = tcp.write_all(b"LEAVE\n");
        
        let _ = tcp_task.await;
        let _ = render_task.await;
        let _ = capture_task.await;

        Ok(())
    }

    /// Allows client to receive control messages from the server,
    /// which indicates changes in state for the user's session
    async fn handle_tcp_control(
        tcp_stream: Arc<Mutex<TcpStream>>,
        is_connected: Arc<Mutex<bool>>,
        has_peer: Arc<Mutex<bool>>,
    ) -> Result<(), Box<dyn Error>> {

        let mut buffer = [0u8; 1024];

        {
            let mut tcp = tcp_stream.lock().unwrap();
            tcp.set_nonblocking(true)?;
        }

        while *is_connected.lock().unwrap() {
            let n = match {
                let mut tcp = tcp_stream.lock().unwrap();
                tcp.read(&mut buffer)
            } {
                Ok(n) => n,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    continue;
                }
                Err(e) => return Err(e.into()),
            };
            
            // connection has been terminated
            if n == 0 {
                *is_connected.lock().unwrap() = false;
                break;
            }
            
            // interpret session control message
            let msg = String::from_utf8_lossy(&buffer[..n]);
            if msg.starts_with("CONNECTED") {
                println!("peer connected to session");
                *has_peer.lock().unwrap() = true;
            } else if msg.starts_with("DISCONNECTED") {
                println!("peer disconnected from session");
                *has_peer.lock().unwrap() = false;
            }
        }

        Ok(())
    }

    /// Receive ASCII frame datagrams and assemble them into a usable `AsciiFrame`
    async fn receive_and_deserialize_frames(
        udp_socket: Arc<UdpSocket>,
        is_connected: Arc<Mutex<bool>>,
        has_peer: Arc<Mutex<bool>>,
    ) -> Result<(), Box<dyn Error>> {

        let mut renderer = AsciiRenderer::new()?;
        let mut buffer = vec![0u8; 65536];
        
        udp_socket.set_nonblocking(true)?;

        while *is_connected.lock().unwrap() {
            if !*has_peer.lock().unwrap() {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                continue;
            }

            let n = match udp_socket.recv(&mut buffer) {
                Ok(n) => n,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                    continue;
                },
                Err(e) => return Err(e.into()),
            };
            
            match renderer.process_datagram(&buffer[..n]) {
                Ok(frame) => {
                    // actual rendering
                    if let Err(e) = renderer.render(&frame) {
                        eprintln!("failed to render frame: {}", e);
                    }
                },
                Err(e) => {
                    eprintln!("failed to process frame: {}", e);
                    continue;
                }
            }
        }

        Ok(())
    }

    /// Serialize `AsciiFrame`s generated by the client to send to their peer
    async fn capture_and_serialize_frames(
        udp_socket: Arc<UdpSocket>,
        mut frame_rx: mpsc::Receiver<AsciiFrame>,
        is_connected: Arc<Mutex<bool>>,
        has_peer: Arc<Mutex<bool>>,
    ) -> Result<(), Box<dyn Error>> {

        while *is_connected.lock().unwrap() {
            // check for peer
            if !*has_peer.lock().unwrap() {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                continue;
            }

            // get next frame
            let frame = match tokio::time::timeout(
                std::time::Duration::from_millis(100),
                frame_rx.recv()
            ).await {
                Ok(Some(frame)) => frame,
                Ok(None) => break,  // channel closed
                Err(_) => continue, // timeout, retry
            };
            
            let data = AsciiRenderer::serialize_frame(&frame);
            
            match udp_socket.send(&data) {
                Ok(_) => {},
                Err(e) => {
                    eprintln!("failed to send frame: {}", e);
                    continue;
                }
            }
        }

        Ok(())
    }
}