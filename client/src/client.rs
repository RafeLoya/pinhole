// use std::error::Error;
// use std::net::SocketAddr;
// use std::sync::Arc;
// use quinn::{ClientConfig, Connection, Endpoint};
// use rustls::client::danger;
// use rustls::crypto::{verify_tls12_signature, verify_tls13_signature, CryptoProvider};
// use rustls::{DigitallySignedStruct, SignatureScheme};
// use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
// use tracing::info;
// use quinn::SendStream;
// use quinn::RecvStream;
// #[derive(Debug)]
// struct SkipServerVerification(Arc<CryptoProvider>);
//
// impl SkipServerVerification {
//     fn new() -> Arc<Self> {
//         Arc::new(Self(Arc::new(rustls::crypto::ring::default_provider())))
//     }
// }
//
// impl danger::ServerCertVerifier for SkipServerVerification {
//     fn verify_server_cert(
//         &self,
//         _end_entity: &CertificateDer<'_>,
//         _intermediates: &[CertificateDer<'_>],
//         _server_name: &ServerName<'_>,
//         _ocsp: &[u8],
//         _now: UnixTime,
//     ) -> Result<danger::ServerCertVerified, rustls::Error> {
//         Ok(danger::ServerCertVerified::assertion())
//     }
//     fn verify_tls12_signature(
//         &self,
//         message: &[u8],
//         cert: &CertificateDer<'_>,
//         dss: &DigitallySignedStruct,
//     ) -> Result<danger::HandshakeSignatureValid, rustls::Error> {
//         verify_tls12_signature(
//             message,
//             cert,
//             dss,
//             &self.0.signature_verification_algorithms,
//         )
//     }
//
//     fn verify_tls13_signature(
//         &self,
//         message: &[u8],
//         cert: &CertificateDer<'_>,
//         dss: &DigitallySignedStruct,
//     ) -> Result<danger::HandshakeSignatureValid, rustls::Error> {
//         verify_tls13_signature(
//             message,
//             cert,
//             dss,
//             &self.0.signature_verification_algorithms,
//         )
//     }
//
//     fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
//         self.0.signature_verification_algorithms.supported_schemes()
//     }
// }
//
// pub struct Client {
//     endpoint: Endpoint,
//     connection: Option<Connection>,
// }
//
// impl Client {
//     pub fn new() -> Result<Self, Box<dyn Error>> {
//         let client_config = Self::configure_client()?;
//
//         let bind_addr = "[::]:0".parse::<SocketAddr>()?;
//         let mut endpoint = Endpoint::client(bind_addr)?;
//         endpoint.set_default_client_config(client_config);
//
//         Ok(Self {
//             endpoint,
//             connection: None,
//         })
//     }
//
//     pub async fn connect(&mut self, server_addr: SocketAddr, server_name: &str) -> Result<(), Box<dyn Error>> {
//         let connecting = self.endpoint
//             .connect(server_addr, server_name)?;
//         let connection = connecting.await?;
//
//         info!("connected to server: {}", connection.remote_address());
//
//         self.connection = Some(connection);
//
//         Ok(())
//     }
//
//     pub async fn send_message(&self, message: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
//         if let Some(conn) = self.connection.as_ref() {
//             let (mut send, mut recv) = conn.open_bi().await?;
//
//             send.write_all(&message).await?;
//             send.finish()?;
//
//             let response = recv.read_to_end(1024).await?;
//             info!("received message: {:?}", response);
//
//             Ok(response)
//         } else {
//             Err("not connected to any server".into())
//         }
//     }
//
//     pub fn close(&self) {
//         if let Some(conn) = &self.connection {
//             conn.close(0u32.into(), b"done");
//         }
//     }
//
//     pub async fn wait_idle(&self) {
//         self.endpoint.wait_idle().await;
//     }
//
//     fn configure_client() -> Result<ClientConfig, Box<dyn Error>> {
//         rustls::crypto::ring::default_provider()
//             .install_default()
//             .expect("failed to install rustls crypto provider");
//
//         let mut crypto = rustls::ClientConfig::builder()
//             .dangerous() // wot is this?
//             .with_custom_certificate_verifier(SkipServerVerification::new())
//             .with_no_client_auth();
//
//         crypto.alpn_protocols = vec![b"h3".to_vec()];
//
//         let quinn_crypto = quinn::crypto::rustls::QuicClientConfig::try_from(crypto)?;
//
//         let client_config =  ClientConfig::new(Arc::new(quinn_crypto));
//
//         Ok(client_config)
//     }
// }

use std::error::Error;
use std::io::{Read, Write};
use std::net::{TcpStream, UdpSocket};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tokio::task;
use common::ascii_frame::AsciiFrame;
use crate::ascii_converter::AsciiConverter;
use crate::ascii_renderer::AsciiRenderer;
use crate::camera::Camera;
use crate::image_frame::ImageFrame;
use crate::video_config::VideoConfig;

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
        let config = VideoConfig::new(
            640,
            480,
            120,
            40,
            127.50,
            1.5, 
            0.0
        );
        
        let mut camera = Camera::new(config.camera_width, config.camera_height)?;
        
        let mut image_frame = ImageFrame::new(config.camera_width, config.camera_height, 3)?;
        let mut ascii_frame = AsciiFrame::new(config.ascii_width, config.ascii_height, ' ')?;
        
        let converter = AsciiConverter::new(
            AsciiConverter::DEFAULT_ASCII_INTENSITY.chars().collect(),
            AsciiConverter::DEFAULT_ASCII_HORIZONTAL.chars().collect(),
            AsciiConverter::DEFAULT_ASCII_VERTICAL.chars().collect(),
            AsciiConverter::DEFAULT_ASCII_FORWARD.chars().collect(),
            AsciiConverter::DEFAULT_ASCII_BACK.chars().collect(),
            config.camera_width,
            config.camera_height,
            config.edge_threshold,
            config.contrast,
            config.brightness
        )?;
        
        while *self.is_connected.lock().unwrap() {
            if *self.has_peer.lock().unwrap() {
                if let Err(e) = camera.capture_frame(&mut image_frame) {
                    eprintln!("failed to capture frame: {}", e);
                    continue;
                }
                
                if let Err(e) = converter.convert(&image_frame, &mut ascii_frame) {
                    eprintln!("failed to convert frame: {}", e);
                    continue;
                }
                
                let mut frame_to_send = AsciiFrame::new(config.ascii_width, config.ascii_height, ' ')?;
                frame_to_send.set_chars_from_bytes(&ascii_frame.bytes());
                if let Err(e) = frame_tx.try_send(frame_to_send) {
                    eprintln!("failed to send ascii frame: {}", e);
                }
            }
            
            // originally 10ms
            tokio::time::sleep(tokio::time::Duration::from_millis(33)).await;
            //std::thread::sleep(std::time::Duration::from_millis(33));
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