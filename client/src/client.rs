use crate::ascii_converter::AsciiConverter;
use crate::ascii_renderer::AsciiRenderer;
use crate::camera::Camera;
use crate::config::PinholeConfig;
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
use tokio::time::{Instant, sleep};
use tokio_util::sync::CancellationToken;

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
    /// Written to by TCP-control
    conn_flag_tx: watch::Sender<bool>,
    /// Flag for session connection.
    /// Read by other tasks
    conn_flag_rx: watch::Receiver<bool>,
    /// Flag for if peer is on other end of session.
    /// Written to by TCP-control
    peer_flag_tx: watch::Sender<bool>,
    /// Flag for if peer is on other end of session.
    /// Read by sender & renderer
    peer_flag_rx: watch::Receiver<bool>,
    /// Optionally, pattern can be used instead of camera
    test_pattern: Option<PatternType>,
    /// Configuration
    config: PinholeConfig,
    /// Cancellation token for graceful shutdown
    cancel_token: CancellationToken,
}

impl Client {
    pub fn new(
        server_tcp_addr: String,
        server_udp_addr: String,
        session_id: String,
        test_pattern: Option<PatternType>,
        config: PinholeConfig,
        cancel_token: CancellationToken,
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
            config,
            cancel_token,
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

        // establish UDP socket
        let udp_socket = Arc::new(UdpSocket::bind("0.0.0.0:0").await?);
        udp_socket.connect(&self.server_udp_addr).await?;

        // === SESSION HANDSHAKE (JOIN + REGISTER_UDP) ============================================
        // sends JOIN request to server to either create a new session or
        // join a preexisting one
        tcp_wr
            .write_all(format!("JOIN {}\n", self.session_id).as_bytes())
            .await?;
        Self::expect_ok(&mut tcp_rd).await?;
        udp_socket.send(b"PING").await?;

        // update our session status to connected
        let _ = self.conn_flag_tx.send(true);

        // println!("joined session: {}", self.session_id);

        let (frame_tx, _) = broadcast::channel::<AsciiFrame>(self.config.performance.frame_buffer);

        // === TCP SESSION CONTROL ================================================================
        // reads control messages from server, updating local state about
        // session connection and / or peer presence.
        let ctrl_conn_tx = self.conn_flag_tx.clone();
        let ctrl_peer_tx = self.peer_flag_tx.clone();
        let ctrl_cancel = self.cancel_token.clone();
        task::spawn(async move {
            let mut buf = vec![0u8; 1024];

            loop {
                let n = tokio::select! {
                    result = tcp_rd.read(&mut buf) => match result {
                        // connection to SFU terminated
                        Ok(0) => {
                            let _ = ctrl_conn_tx.send(false);
                            break;
                        }
                        // message received
                        Ok(n) => n,
                        // read error
                        Err(e) => {
                            eprintln!("[CONTROL] TCP read error: {e}");
                            let _ = ctrl_conn_tx.send(false);
                            break;
                        }
                    },
                    _ = ctrl_cancel.cancelled() => {
                        let _ = ctrl_conn_tx.send(false);
                        break;
                    }
                };

                // actions for received message
                match &buf[..n] {
                    msg if msg.starts_with(b"CONNECTED") => {
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
        // receive incoming frames and render.
        let rend_conn_rx = self.conn_flag_rx.clone();
        let mut rend_peer_rx = self.peer_flag_rx.clone();
        let udp_rend = udp_socket.clone();
        let frame_interval = Duration::from_millis(1000 / self.config.performance.fps);
        let rend_cancel = self.cancel_token.clone();
        task::spawn(async move {
            let mut buf = vec![0u8; 65536];
            let mut renderer = AsciiRenderer::new().unwrap();
            let mut next_frame_time = Instant::now() + frame_interval;

            while *rend_conn_rx.borrow() && !rend_cancel.is_cancelled() {
                // blocks until peer is present
                tokio::select! {
                    _ = rend_peer_rx.wait_for(|peer| *peer) => {}
                    _ = rend_cancel.cancelled() => break
                }

                let frame_start = Instant::now();

                let mut next_frame = None;
                loop {
                    match udp_rend.try_recv(&mut buf) {
                        // received frame, move on to rendering it
                        Ok(n) => {
                            if let Ok(frame) = renderer.process_datagram(&buf[..n]) {
                                next_frame = Some(frame);
                            }
                        }
                        // expected, wait for frame to arrive
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            if next_frame.is_some() {
                                break;
                            } else {
                                // sleep for a tiny bit or exit on cancellation
                                tokio::select! {
                                    _ = sleep(Duration::from_millis(1)) => {}
                                    _ = rend_cancel.cancelled() => return
                                }
                            }
                        }
                        // actual receive error
                        Err(e) => {
                            eprintln!("[RENDER] UDP receive error: {e}");
                            if next_frame.is_some() {
                                break;
                            } else {
                                // sleep for a tiny bit or exit on cancellation
                                tokio::select! {
                                    _ = sleep(Duration::from_millis(1)) => {}
                                    _ = rend_cancel.cancelled() => return
                                }
                            }
                        }
                    }
                }
                let _ = renderer.render(&next_frame.unwrap());

                let now = Instant::now();
                let frame_time = (now - frame_start).as_millis();
                
                let one_frame_ago = now.checked_sub(frame_interval).unwrap_or(now);
                if next_frame_time < one_frame_ago {
                    // more than 1 frame behind - reset to prevent spiraling
                    next_frame_time = now + frame_interval;
                    let message = format!(
                        "[RENDER] Frame took {}ms (target: {}ms) - resetting timing",
                        frame_time,
                        frame_interval.as_millis()
                    );
                    let _ = renderer.write_status_message(&message);
                } else if next_frame_time > now {
                    // on schedule - sleep until next frame
                    tokio::select! {
                        _ = sleep(next_frame_time - now) => {}
                        _ = rend_cancel.cancelled() => return
                    }
                    next_frame_time += frame_interval;
                } else {
                    // slightly behind but catching up
                    next_frame_time += frame_interval;
                    if frame_time > frame_interval.as_millis() {
                        let message = format!(
                            "[RENDER] Frame took {}ms (target: {}ms)",
                            frame_time,
                            frame_interval.as_millis()
                        );
                        let _ = renderer.write_status_message(&message);
                    }
                }
            }
        });

        // === FRAME CAPTURE, ENCODING, AND SENDING ===============================================
        // receive AsciiFrame, then serialize and send to peer via UDP if present.
        let send_conn_rx = self.conn_flag_rx.clone();
        let mut send_peer_rx = self.peer_flag_rx.clone();
        let udp_send = udp_socket.clone();
        let mut ser_rx = frame_tx.subscribe();
        let send_cancel = self.cancel_token.clone();
        task::spawn(async move {
            while *send_conn_rx.borrow() && !send_cancel.is_cancelled() {
                // blocks until peer is present
                tokio::select! {
                    _ = send_peer_rx.wait_for(|peer| *peer) => {}
                    _ = send_cancel.cancelled() => break
                }

                tokio::select! {
                    result = ser_rx.recv() => match result {
                        Ok(frame) => {
                            let data = AsciiRenderer::serialize_frame(&frame);
                            let _ = udp_send.send(&data).await;
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            break;
                        }
                        _ => {}
                    },
                    _ = send_cancel.cancelled() => break
                }

                // TODO: look at notes "Current Caveats of AsciiFrame"
            }
        });

        // === FRAME GENERATION (WEBCAM OR TEST PATTERN) ==========================================
        // from either a mock frame generator or the camera,
        // create the ASCII frames to send to the peer.
        let cfg = VideoConfig::from_pinhole_config(&self.config);
        if let Some(pattern) = &self.test_pattern {
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
            }
        } else {
            let mut camera = Camera::from_config(&self.config)?;

            let mut image_frame = ImageFrame::new(cfg.camera_width, cfg.camera_height, 3)?;
            let mut ascii_frame = AsciiFrame::new(cfg.ascii_width, cfg.ascii_height, ' ')?;

            let converter = AsciiConverter::new(
                self.config.ascii.chars.intensity.chars().collect(),
                self.config.ascii.chars.horizontal_lines.chars().collect(),
                self.config.ascii.chars.vertical_lines.chars().collect(),
                self.config.ascii.chars.forward_diagonal.chars().collect(),
                self.config.ascii.chars.back_diagonal.chars().collect(),
                cfg.camera_width,
                cfg.camera_height,
                cfg.edge_threshold,
                cfg.contrast,
                cfg.brightness,
            )?;

            while *self.conn_flag_rx.borrow() && !self.cancel_token.is_cancelled() {
                if *self.peer_flag_rx.borrow() {
                    camera.capture_frame(&mut image_frame)?;
                    converter.convert(&image_frame, &mut ascii_frame)?;

                    let mut output = AsciiFrame::new(cfg.ascii_width, cfg.ascii_height, ' ')?;
                    output.set_chars(ascii_frame.chars());
                    let _ = frame_tx.send(output);
                }
            }
        }

        // connection stopped, signal to TCP CONTROL and leave
        let _ = tcp_wr.write_all(b"LEAVE\n").await;
        Ok(())
    }

    /// Run in solo mode - local preview without network connection
    /// This allows testing webcam / screen / file capture and ASCII rendering
    /// without needing a server or peer
    pub async fn run_solo(&self) -> Result<(), Box<dyn Error>> {
        let cfg = VideoConfig::from_pinhole_config(&self.config);

        if let Some(pattern) = &self.test_pattern {
            // mock pattern mode
            let pattern_val = match pattern {
                PatternType::Checkerboard => PatternType::Checkerboard,
                PatternType::MovingLine => PatternType::MovingLine,
            };

            let mut frame_gen =
                MockFrameGenerator::new(cfg.ascii_width, cfg.ascii_height, 30, pattern_val)?;

            let mut renderer = AsciiRenderer::new()?;

            println!("Generating test pattern...");

            // proper frame timing
            let frame_interval = Duration::from_millis(1000 / 30);
            let mut next_frame_time = Instant::now() + frame_interval;

            loop {
                // check for cancellation
                if self.cancel_token.is_cancelled() {
                    return Ok(());
                }

                let frame = frame_gen.generate_frame()?;
                renderer.render(&frame)?;

                // only sleep if we finished early
                let now = Instant::now();
                if next_frame_time > now {
                    tokio::select! {
                        _ = sleep(next_frame_time - now) => {}
                        _ = self.cancel_token.cancelled() => {
                            return Ok(());
                        }
                    }
                }
                next_frame_time += frame_interval;
            }
        } else {
            // camera / screen / file mode
            let mut camera = Camera::from_config(&self.config)?;

            // enable frame-dropping mode to prevent buffering lag
            camera.enable_frame_dropping()?;
            println!("Frame-dropping mode enabled (always renders latest frame)");

            let mut image_frame = ImageFrame::new(cfg.camera_width, cfg.camera_height, 3)?;
            let mut ascii_frame = AsciiFrame::new(cfg.ascii_width, cfg.ascii_height, ' ')?;

            let converter = AsciiConverter::new(
                self.config.ascii.chars.intensity.chars().collect(),
                self.config.ascii.chars.horizontal_lines.chars().collect(),
                self.config.ascii.chars.vertical_lines.chars().collect(),
                self.config.ascii.chars.forward_diagonal.chars().collect(),
                self.config.ascii.chars.back_diagonal.chars().collect(),
                cfg.camera_width,
                cfg.camera_height,
                cfg.edge_threshold,
                cfg.contrast,
                cfg.brightness,
            )?;

            let mut renderer = AsciiRenderer::new()?;

            println!("Capturing from {}...", self.config.video.source.r#type);

            let frame_interval = Duration::from_millis(1000 / self.config.performance.fps);
            let mut next_frame_time = Instant::now() + frame_interval;
            let mut duplicate_count = 0u64;

            loop {
                // check for cancellation
                if self.cancel_token.is_cancelled() {
                    return Ok(());
                }
                let frame_start = Instant::now();

                // get the latest frame (automatically drops old buffered frames)
                let is_new_frame = camera.capture_latest_frame(&mut image_frame)?;

                if !is_new_frame {
                    duplicate_count += 1;
                    if duplicate_count % 10 == 0 {
                        let message = format!(
                            "[SOLO] Warning: {} duplicate frames (FFmpeg slower than render loop)",
                            duplicate_count
                        );
                        let _ = renderer.write_status_message(&message);
                    }
                }

                converter.convert(&image_frame, &mut ascii_frame)?;
                renderer.render(&ascii_frame)?;

                // calculate actual frame processing time
                let now = Instant::now();
                let frame_time = (now - frame_start).as_millis();

                // accurate frame pacing with catch-up protection
                let one_frame_ago = now.checked_sub(frame_interval).unwrap_or(now);
                if next_frame_time < one_frame_ago {
                    // more than 1 frame behind - reset to prevent spiraling
                    next_frame_time = now + frame_interval;
                    let message = format!(
                        "[SOLO] Frame took {}ms (target: {}ms) - resetting timing",
                        frame_time,
                        frame_interval.as_millis()
                    );
                    let _ = renderer.write_status_message(&message);
                } else if next_frame_time > now {
                    // on schedule - sleep until next frame
                    tokio::select! {
                        _ = sleep(next_frame_time - now) => {}
                        _ = self.cancel_token.cancelled() => {
                            return Ok(());
                        }
                    }
                    next_frame_time += frame_interval;
                } else {
                    // slightly behind but catching up
                    next_frame_time += frame_interval;
                    if frame_time > frame_interval.as_millis() {
                        let message = format!(
                            "[SOLO] Frame took {}ms (target: {}ms)",
                            frame_time,
                            frame_interval.as_millis()
                        );
                        let _ = renderer.write_status_message(&message);
                    }
                }
            }
        }
    }

    /// Receive and respond to the initial handshake from the server
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