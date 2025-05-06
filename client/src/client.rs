use std::error::Error;
use std::io::{Read, Write};
use std::net::{TcpStream, UdpSocket};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task;
use common::ascii_frame::AsciiFrame;
use crate::ascii_converter::AsciiConverter;
use crate::ascii_renderer::AsciiRenderer;
use crate::camera::Camera;
use crate::image_frame::ImageFrame;
use crate::mock_frame_generator::{MockFrameGenerator, PatternType};
use crate::video_config::VideoConfig;

/// Terminal-based client that connects to a server for ASCII video streaming.
/// Session control is handled over TCP, frame forwarding is handled over UDP.
/// Can either use a camera or generate a test patten
pub struct Client {
    /// TCP address for 'control' messages (e.g. JOIN, LEAVE)
    server_tcp_addr: String,
    /// Sending / receiving ASCII video frames
    server_udp_addr: String,
    /// Session ID client attempts to join
    session_id: String,
    /// Is client currently in a session?
    is_connected: Arc<Mutex<bool>>,
    /// Does client have another peer client in their session?
    has_peer: Arc<Mutex<bool>>,
    /// Optionally, pattern can be used instead of camera
    test_pattern: Option<PatternType>
}

impl Client {
    pub fn new(server_tcp_addr: String, server_udp_addr: String, session_id: String, test_pattern: Option<PatternType>) -> Self {
        Self {
            server_tcp_addr,
            server_udp_addr,
            session_id,
            is_connected: Arc::new(Mutex::new(false)),
            has_peer: Arc::new(Mutex::new(false)),
            test_pattern,
        }
    }

    /// Start client's runtime logic:
    /// - Connect to server
    /// - Join session
    /// - Registers its UDP port
    /// - Spawns background tasks for:
    ///     - TCP control handling
    ///     - UDP receiving / rendering
    ///     - Frame generation / sending
    pub async fn run(&self) -> Result<(), Box<dyn Error>> {
        let mut tcp_stream = TcpStream::connect(&self.server_tcp_addr)?;
        println!("Connected to server at {}", &self.server_tcp_addr);

        let udp_socket = UdpSocket::bind("0.0.0.0:0")?;
        let local_udp_addr = udp_socket.local_addr()?;
        udp_socket.connect(&self.server_udp_addr)?;
        println!("UDP socket bound to {} and connected to {}", local_udp_addr, self.server_udp_addr);

        let join_cmd = format!("JOIN {}\n", self.session_id);
        tcp_stream.write_all(join_cmd.as_bytes())?;

        // wait for OK
        let mut response = [0u8; 1024];
        let n =  tcp_stream.read(&mut response)?;
        let response = std::str::from_utf8(&response[..n])?;

        if !response.starts_with("OK") {
            return Err(format!("failed to join session: {}", response).into());
        }

        // register UDP port
        let register_cmd = format!("REGISTER_UDP {}\n", local_udp_addr.port());
        tcp_stream.write_all(register_cmd.as_bytes())?;

        // wait for OK
        let mut udp_register_response = [0u8; 1024];
        let n = tcp_stream.read(&mut udp_register_response)?;
        let udp_register_response = std::str::from_utf8(&udp_register_response[..n])?;

        if !udp_register_response.starts_with("OK") {
            return Err(format!("failed to register UDP port: {}", response).into());
        }

        println!("Registering UDP port {} with server", local_udp_addr.port());

        {
            let mut connected = self.is_connected.lock().unwrap();
            *connected = true;
        }

        println!("joined session: {}", self.session_id);

        {
            let mut connected = self.is_connected.lock().unwrap();
            *connected = true;
            let mut has_peer = self.has_peer.lock().unwrap();
            *has_peer = true;
            println!("WARNING: manually forcing 'has_peer=true'")
        }

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

        println!("connection status: connected={}\nhas_peer={}",
                 *self.is_connected.lock().unwrap(),
                 *self.has_peer.lock().unwrap());
        if let Some(pattern) = &self.test_pattern {
            let pattern_val = match pattern {
                PatternType::Checkerboard => PatternType::Checkerboard,
                &PatternType::MovingLine => PatternType::MovingLine,
            };

            let mut frame_gen = MockFrameGenerator::new(
                config.ascii_width, config.ascii_height, 30, pattern_val
            )?;

            while *self.is_connected.lock().unwrap() {
                if *self.has_peer.lock().unwrap() {
                    match frame_gen.generate_frame() {
                        Ok(frame) => {
                            if let Err(e) = frame_tx.try_send(frame) {
                                eprintln!("failed to send ascii frame: {}", e);
                            }
                        }
                        Err(e) => {
                            eprintln!("failed to generate frame: {}", e);
                            continue;
                        }
                    }
                }

                tokio::time::sleep(Duration::from_millis(33)).await;
            }
        } else {
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
                tokio::time::sleep(Duration::from_millis(33)).await;
                //std::thread::sleep(std::time::Duration::from_millis(33));
            }
            //=====================================================================
        }

        // clean up
        let mut tcp = tcp_stream.lock().unwrap();
        let _ = tcp.write_all(b"LEAVE\n");

        let _ = tcp_task.await;
        let _ = render_task.await;
        let _ = capture_task.await;

        Ok(())
    }

    /// Reads control messages from server, updating local state about
    /// session connection and / or peer presence.
    /// 
    /// Exits if connection is dropped.
    async fn handle_tcp_control(
        tcp_stream: Arc<Mutex<TcpStream>>,
        is_connected: Arc<Mutex<bool>>,
        has_peer: Arc<Mutex<bool>>,
    ) -> Result<(), Box<dyn Error>> {

        let mut buffer = [0u8; 1024];

        {
            let tcp = tcp_stream.lock().unwrap();
            tcp.set_nonblocking(true)?;
        }

        while *is_connected.lock().unwrap() {
            let n = match {
                let mut tcp = tcp_stream.lock().unwrap();
                tcp.read(&mut buffer)
            } {
                Ok(n) => n,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    tokio::time::sleep(Duration::from_millis(100)).await;
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
            println!("TCP control message received: '{}'", msg.trim());

            //if msg.trim().starts_with("CONNECTED") {
            if msg.trim().contains("CONNECTED") {
                println!("peer connected to session");
                *has_peer.lock().unwrap() = true;
                println!("has_peer is now: {}", *has_peer.lock().unwrap());
                //} else if msg.trim().starts_with("DISCONNECTED") {
            } else if msg.trim().contains("DISCONNECTED") {
                println!("peer disconnected from session");
                *has_peer.lock().unwrap() = false;
            } else {
                println!("unknown control message: {}", msg.trim());
            }
        }

        Ok(())
    }

    /// Receive ASCII frame datagrams and assemble them into a usable `AsciiFrame`
    /// to be rendered w/ `AsciiRenderer`
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
                tokio::time::sleep(Duration::from_millis(100)).await;
                continue;
            }

            let n = match udp_socket.recv(&mut buffer) {
                Ok(n) => {
                    //println!("received datagram of size: {}", n);
                    n
                },
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                    continue;
                },
                Err(e) => return Err(e.into()),
            };

            match renderer.process_datagram(&buffer[..n]) {
                Ok(frame) => {
                    // actual rendering
                    //println!("processed frame, rendering...");
                    if let Err(e) = renderer.render(&frame) {
                        eprintln!("failed to render frame: {}", e);
                    } else {
                        //println!("rendered frame");
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
    /// via UDP if peer is present
    async fn capture_and_serialize_frames(
        udp_socket: Arc<UdpSocket>,
        mut frame_rx: mpsc::Receiver<AsciiFrame>,
        is_connected: Arc<Mutex<bool>>,
        has_peer: Arc<Mutex<bool>>,
    ) -> Result<(), Box<dyn Error>> {

        while *is_connected.lock().unwrap() {
            // check for peer
            if !*has_peer.lock().unwrap() {
                tokio::time::sleep(Duration::from_millis(100)).await;
                continue;
            }

            // get next frame
            let frame = match tokio::time::timeout(
                Duration::from_millis(100),
                frame_rx.recv()
            ).await {
                Ok(Some(frame)) => frame,
                Ok(None) => break,  // channel closed
                Err(_) => continue, // timeout, retry
            };

            let data = AsciiRenderer::serialize_frame(&frame);
            //println!("sending frame of size: {}", data.len());

            match udp_socket.send(&data) {
                Ok(_) => {
                    //println!("sending frame of size: {}", data.len());
                },
                Err(e) => {
                    eprintln!("failed to send frame: {}", e);
                    continue;
                }
            }
        }

        Ok(())
    }
}