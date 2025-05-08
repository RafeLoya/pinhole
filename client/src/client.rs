use crate::ascii_converter::AsciiConverter;
use crate::ascii_renderer::AsciiRenderer;
use crate::camera::Camera;
use crate::image_frame::ImageFrame;
use crate::mock_frame_generator::{MockFrameGenerator, PatternType};
use crate::video_config::VideoConfig;
use common::ascii_frame::AsciiFrame;
use std::error::Error;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::OwnedReadHalf;
use tokio::net::{TcpStream, UdpSocket};
use tokio::sync::{broadcast, watch};
use tokio::task;
use tokio::time::{interval, sleep, Instant, MissedTickBehavior};

/// Max amount of frames that can be buffered
const FRAME_BUFFER: usize = 15;
const FPS: u64 = 30;

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
    /// Flag for session connection.
    /// Written to by TCP-control, read by other tasks
    conn_flag_tx: watch::Sender<bool>,
    conn_flag_rx: watch::Receiver<bool>,
    /// Flag for if peer is on other end of session
    /// Written to by TCP-control, read by sender & renderer
    peer_flag_tx: watch::Sender<bool>,
    peer_flag_rx: watch::Receiver<bool>,
    /// Optionally, pattern can be used instead of camera
    test_pattern: Option<PatternType>,
}

impl Client {
    pub fn new(
        server_tcp_addr: String,
        server_udp_addr: String,
        session_id: String,
        test_pattern: Option<PatternType>,
    ) -> Self {
        let (conn_flag_tx, conn_flag_rx) = watch::channel(false);
        let (peer_flag_tx, peer_flag_rx) = watch::channel(false);

        Self {
            server_tcp_addr,
            server_udp_addr,
            session_id,
            conn_flag_tx,
            conn_flag_rx,
            peer_flag_tx,
            peer_flag_rx,
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
        // establish TCP socket
        let tcp_stream = TcpStream::connect(&self.server_tcp_addr).await?;
        let (mut tcp_rd, mut tcp_wr) = tcp_stream.into_split();
        
        //println!("Connected to server at {}", &self.server_tcp_addr);

        // establish UDP socket
        let udp_socket = Arc::new(UdpSocket::bind("0.0.0.0:0").await?);
        let local_udp_addr = udp_socket.local_addr()?;
        udp_socket.connect(&self.server_udp_addr).await?;
        
        // println!(
        //     "UDP socket bound to {} and connected to {}",
        //     local_udp_addr, self.server_udp_addr
        // );

        // === SESSION HANDSHAKE (JOIN + REGISTER_UDP) ============================================
        // Sends JOIN request to server to either create a new session or
        // join a preexisting one
        tcp_wr
            .write_all(format!("JOIN {}\n", self.session_id).as_bytes())
            .await?;
        Self::expect_ok(&mut tcp_rd).await?;
        // println!("JOIN received by server for session {}", self.session_id);
        tcp_wr
            .write_all(format!("REGISTER_UDP {}\n", local_udp_addr.port()).as_bytes())
            .await?;
        Self::expect_ok(&mut tcp_rd).await?;
        // println!(
        //     "Registered UDP port {} with server for session {}",
        //     local_udp_addr.port(),
        //     self.session_id
        // );

        // update our session status to connected
        let _ = self.conn_flag_tx.send(true);

        // println!("joined session: {}", self.session_id);

        // TODO: how large should buffer be?
        // EXPERIMENTAL: target FPS * worst consumer lag in seconds
        // 30 FPS * 3 Seconds = 90
        // TODO: watch channel? only keeping one frame but latency bounded at one frame
        // TODO: broadcast channel (ring buffer)? drops oldest frame automatically
        let (frame_tx, _) = broadcast::channel::<AsciiFrame>(FRAME_BUFFER);

        // === TCP SESSION CONTROL ================================================================
        // Reads control messages from server, updating local state about
        // session connection and / or peer presence.
        let ctrl_conn_tx = self.conn_flag_tx.clone();
        let ctrl_peer_tx = self.peer_flag_tx.clone();
        task::spawn(async move {
            let mut buf = vec![0u8; 1024];

            loop {
                let n = match tcp_rd.read(&mut buf).await {
                    Ok(0) => {
                        let _ = ctrl_conn_tx.send(false);
                        break;
                    }
                    Ok(n) => n,
                    Err(e) => {
                        eprintln!("TCP read error: {e}");
                        let _ = ctrl_conn_tx.send(false);
                        break;
                    }
                };

                match &buf[..n] {
                    msg if msg.starts_with(b"CONNECTED") => {
                        //println!("CONTROL: got CONNECTED");
                        let _ = ctrl_peer_tx.send(true);
                    }
                    msg if msg.starts_with(b"DISCONNECTED") => {
                        let _ = ctrl_peer_tx.send(false);
                    }
                    _ => {}
                }
            }
        });

        // === FRAME RENDERING ====================================================================
        let rend_conn_rx = self.conn_flag_rx.clone();
        let mut rend_peer_rx = self.peer_flag_rx.clone();
        let udp_rend = udp_socket.clone();
        let frame_interval = Duration::from_millis((1000 / FPS));
        task::spawn(async move {
            let mut buf = vec![0u8; 65536];
            let mut renderer = AsciiRenderer::new().unwrap();
            // let mut ticker = interval(frame_interval);
            // ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
            let mut next_frame = Instant::now() + frame_interval;

            while *rend_conn_rx.borrow() {
                // blocks until peer is present
                let _ = rend_peer_rx.wait_for(|peer| *peer).await;

                // ticker.tick().await;
                match udp_rend.recv(&mut buf).await {
                    Ok(n) => {
                        if let Ok(frame) = renderer.process_datagram(&buf[..n]) {
                            let _ = renderer.render(&frame);
                        }
                    }
                    Err(e) => {
                        eprintln!("UDP receive error: {e}");
                        //continue;
                    }
                }
                
                let now =  Instant::now();
                if next_frame > now {
                    sleep(next_frame - now).await;
                }
                eprintln!("time to sleep {:?}", next_frame - now);
                next_frame = Instant::now() + frame_interval;
            }
        });

        // === FRAME CAPTURE, ENCODING, AND SENDING ===============================================
        // Receive AsciiFrame, then serialize and send to peer via UDP if present
        let send_conn_rx = self.conn_flag_rx.clone();
        let mut send_peer_rx = self.peer_flag_rx.clone();
        let udp_send = udp_socket.clone();
        let mut ser_rx = frame_tx.subscribe();
        task::spawn(async move {
            while *send_conn_rx.borrow() {
                // blocks until peer is present
                let _ = send_peer_rx.wait_for(|peer| *peer).await;

                match ser_rx.recv().await {
                    Ok(frame) => {
                        let data = AsciiRenderer::serialize_frame(&frame);
                        let _ = udp_send.send(&data).await;
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        break;
                    }
                    _ => {}
                }
                
                // // TODO: look at notes "Current Caveats of AsciiFrame"
                // if let Some(frame) = frame_rx.recv().await {
                //     let data = AsciiRenderer::serialize_frame(&frame);
                //     let _ = udp_send.try_send(&data);
                //     //println!("CLIENT: sent {} bytes", data.len());
                // }
            }
        });

        // === FRAME GENERATION (WEBCAM OR TEST PATTERN) ==========================================
        let cfg = VideoConfig::default();
        // println!(
        //     "connection status: connected={}\nhas_peer={}",
        //     *self.conn_flag_rx.borrow(),
        //     *self.peer_flag_rx.borrow()
        // );
        if let Some(pattern) = &self.test_pattern {
            // TODO: this is jank, may not be important if we remove patterns in future
            let pattern_val = match pattern {
                PatternType::Checkerboard => PatternType::Checkerboard,
                &PatternType::MovingLine => PatternType::MovingLine,
            };

            let mut frame_gen =
                MockFrameGenerator::new(cfg.ascii_width, cfg.ascii_height, 30, pattern_val)?;

            while *self.conn_flag_rx.borrow() {
                if *self.peer_flag_rx.borrow() {
                    let frame = frame_gen.generate_frame()?;
                    let _ = frame_tx.send(frame);
                }
                //sleep(Duration::from_millis(33)).await;
            }
        } else {
            let mut camera = Camera::new(cfg.camera_width, cfg.camera_height)?;

            let mut image_frame = ImageFrame::new(cfg.camera_width, cfg.camera_height, 3)?;
            let mut ascii_frame = AsciiFrame::new(cfg.ascii_width, cfg.ascii_height, ' ')?;

            let converter = AsciiConverter::new(
                AsciiConverter::DEFAULT_ASCII_INTENSITY.chars().collect(),
                AsciiConverter::DEFAULT_ASCII_HORIZONTAL.chars().collect(),
                AsciiConverter::DEFAULT_ASCII_VERTICAL.chars().collect(),
                AsciiConverter::DEFAULT_ASCII_FORWARD.chars().collect(),
                AsciiConverter::DEFAULT_ASCII_BACK.chars().collect(),
                cfg.camera_width,
                cfg.camera_height,
                cfg.edge_threshold,
                cfg.contrast,
                cfg.brightness,
            )?;

            while *self.conn_flag_rx.borrow() {
                if *self.peer_flag_rx.borrow() {
                    camera.capture_frame(&mut image_frame)?;
                    converter.convert(&image_frame, &mut ascii_frame)?;

                    let mut output = AsciiFrame::new(cfg.ascii_width, cfg.ascii_height, ' ')?;
                    output.set_chars_from_bytes(&ascii_frame.bytes());
                    let _ = frame_tx.send(output);
                }
                //sleep(Duration::from_millis(33)).await;
            }
        }

        let _ = tcp_wr.write_all(b"LEAVE\n").await;
        Ok(())
    }

    async fn expect_ok(rd: &mut OwnedReadHalf) -> Result<(), Box<dyn Error>> {
        let mut line = Vec::with_capacity(64);
        loop {
            let mut byte = [0u8; 1];
            if rd.read(&mut byte).await? == 0 {
                return Err("unexpected EOF waiting for OK".into());
            }
            line.push(byte[0]);
            if byte[0] == b'\n' {
                break;
            }
        }
        let text = std::str::from_utf8(&line)?.trim_start();
        if text.starts_with("OK") {
            Ok(())
        } else {
            Err(format!("unexpected reply: {}", text).into())
        }
    }
}
