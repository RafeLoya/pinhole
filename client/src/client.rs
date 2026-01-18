use crate::text_converter::AsciiConverter;
use crate::text_renderer::{TextRenderer, FrameSerializer, PerformanceStats, TuiLayout};
use crate::camera::Camera;
use crate::config::PinholeConfig;
use crate::image_frame::ImageFrame;
use crate::mock_frame_generator::{MockFrameGenerator, PatternType};
use common::text_frame::TextFrame;
use common::MAX_UDP_PACKET_SIZE;
use crossterm::event::{Event, EventStream, KeyCode, KeyModifiers};
use futures::StreamExt;
use std::error::Error;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::OwnedReadHalf;
use tokio::net::{TcpStream, UdpSocket};
use tokio::sync::{broadcast, mpsc, watch};
use tokio::task;
use tokio::time::{sleep, Instant};
use tokio_util::sync::CancellationToken;

/// Commands from TUI input handling.
#[derive(Debug, Clone)]
pub enum TuiCommand {
    /// Toggle border visibility
    ToggleBorder,
    /// Toggle debug pane visibility
    ToggleDebug,
    /// Quit the application
    Quit,
}

/// Result of frame timing check
enum FrameTimingAction {
    /// Sleep for the given duration until next frame
    Sleep(Duration),
    /// Reset timing - we're more than 1 frame behind
    Reset,
    /// Continue without sleeping - slightly behind but catching up
    Continue,
}

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

    /// Get camera dimensions based on source type from config
    fn get_camera_dimensions(&self) -> (usize, usize) {
        match self.config.video.source.r#type.as_str() {
            "webcam" => (
                self.config.video.source.webcam.width,
                self.config.video.source.webcam.height,
            ),
            "screen" => (
                self.config.video.source.screen.width,
                self.config.video.source.screen.height,
            ),
            _ => (640, 480), // fallback for file/custom
        }
    }

    /// Check frame timing and determine what action to take
    /// Returns the action and optionally a status message to log
    fn check_frame_timing(
        now: Instant,
        next_frame_time: Instant,
        frame_time_ms: u128,
        frame_interval: Duration,
    ) -> (FrameTimingAction, Option<String>) {
        let one_frame_ago = now.checked_sub(frame_interval).unwrap_or(now);

        if next_frame_time < one_frame_ago {
            // more than 1 frame behind - reset to prevent spiraling
            let message = format!(
                "Frame took {}ms (target: {}ms) - resetting timing",
                frame_time_ms,
                frame_interval.as_millis()
            );
            (FrameTimingAction::Reset, Some(message))
        } else if next_frame_time > now {
            // on schedule - sleep until next frame
            (FrameTimingAction::Sleep(next_frame_time - now), None)
        } else {
            // slightly behind but catching up
            let message = if frame_time_ms > frame_interval.as_millis() {
                Some(format!(
                    "Frame took {}ms (target: {}ms)",
                    frame_time_ms,
                    frame_interval.as_millis()
                ))
            } else {
                None
            };
            (FrameTimingAction::Continue, message)
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

        let (frame_tx, _) = broadcast::channel::<TextFrame>(self.config.performance.frame_buffer);

        // === INPUT HANDLING =====================================================================
        // spawn task to read keyboard events and send commands
        let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel::<TuiCommand>();
        let input_cancel = self.cancel_token.clone();
        task::spawn(async move {
            let mut reader = EventStream::new();
            loop {
                tokio::select! {
                    maybe_event = reader.next() => {
                        match maybe_event {
                            Some(Ok(Event::Key(key_event))) => {
                                let cmd = match key_event.code {
                                    KeyCode::Char('b') => Some(TuiCommand::ToggleBorder),
                                    KeyCode::Char('d') => Some(TuiCommand::ToggleDebug),
                                    KeyCode::Char('q') => Some(TuiCommand::Quit),
                                    KeyCode::Char('c') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                                        Some(TuiCommand::Quit)
                                    }
                                    _ => None,
                                };
                                if let Some(cmd) = cmd {
                                    let _ = cmd_tx.send(cmd);
                                }
                            }
                            Some(Err(_)) | None => break,
                            _ => {}
                        }
                    }
                    _ = input_cancel.cancelled() => break,
                }
            }
        });

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
        // receive incoming frames and render as fast as they arrive.
        
        let rend_conn_rx = self.conn_flag_rx.clone();
        let mut rend_peer_rx = self.peer_flag_rx.clone();
        let udp_rend = udp_socket.clone();
        let rend_cancel = self.cancel_token.clone();
        let intensity_chars: Vec<char> = self.config.ascii.chars.intensity.chars().collect();
        let horizontal_chars: Vec<char> = self.config.ascii.chars.horizontal_lines.chars().collect();
        let vertical_chars: Vec<char> = self.config.ascii.chars.vertical_lines.chars().collect();
        let forward_chars: Vec<char> = self.config.ascii.chars.forward_diagonal.chars().collect();
        let back_chars: Vec<char> = self.config.ascii.chars.back_diagonal.chars().collect();
        let (status_tx, mut status_rx) = mpsc::unbounded_channel::<String>();
        let rend_status_tx = status_tx.clone();
        let rend_cancel_trigger = self.cancel_token.clone();

        task::spawn(async move {
            let mut buf = vec![0u8; 65536];
            let mut renderer = TextRenderer::new_with_chars(
                intensity_chars,
                horizontal_chars,
                vertical_chars,
                forward_chars,
                back_chars,
            ).unwrap();

            while *rend_conn_rx.borrow() && !rend_cancel.is_cancelled() {
                // blocks until peer is present
                tokio::select! {
                    _ = rend_peer_rx.wait_for(|peer| *peer) => {}
                    _ = rend_cancel.cancelled() => break
                }

                let mut next_frame = None;
                let mut recv_count = 0u64;
                loop {
                    match udp_rend.try_recv(&mut buf) {
                        // received frame, move on to rendering it
                        Ok(n) => {
                            recv_count += 1;
                            match renderer.process_datagram(&buf[..n]) {
                                Ok(frame) => {
                                    next_frame = Some(frame);
                                }
                                Err(e) => {
                                    let _ = rend_status_tx.send(format!("[RECV ERROR] {} bytes: {}", n, e));
                                }
                            }

                            // show receive stats every 100 frames
                            if recv_count % 100 == 0 {
                                let _ = rend_status_tx.send(format!("[RECV] {} frames | last: {} bytes", recv_count, n));
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

                // render immediately without FPS throttling
                let _ = renderer.render(&next_frame.unwrap());

                // check for and display status messages
                if let Ok(status_msg) = status_rx.try_recv() {
                    let _ = renderer.write_status_message(&status_msg);
                }

                // handle input commands
                while let Ok(cmd) = cmd_rx.try_recv() {
                    match cmd {
                        TuiCommand::ToggleBorder => renderer.toggle_border(),
                        TuiCommand::ToggleDebug => renderer.toggle_debug(),
                        TuiCommand::Quit => {
                            rend_cancel_trigger.cancel();
                            return;
                        }
                    }
                }
            }
        });

        // === FRAME CAPTURE, ENCODING, AND SENDING ===============================================
        // receive TextFrame, then serialize and send to peer via UDP if present.
        
        let send_conn_rx = self.conn_flag_rx.clone();
        let mut send_peer_rx = self.peer_flag_rx.clone();
        let udp_send = udp_socket.clone();
        let mut ser_rx = frame_tx.subscribe();
        let send_cancel = self.cancel_token.clone();
        let send_status_tx = status_tx.clone();
        
        task::spawn(async move {
            while *send_conn_rx.borrow() && !send_cancel.is_cancelled() {
                // blocks until peer is present
                tokio::select! {
                    _ = send_peer_rx.wait_for(|peer| *peer) => {}
                    _ = send_cancel.cancelled() => break
                }

                // create new serializer for this peer connection
                let mut frame_serializer = FrameSerializer::new();

                loop {
                    tokio::select! {
                        result = ser_rx.recv() => match result {
                            Ok(frame) => {
                                let data = frame_serializer.serialize(&frame);

                                // warn if frame exceeds safe UDP packet size
                                if data.len() > MAX_UDP_PACKET_SIZE && frame_serializer.total_frames % 100 == 1 {
                                    let _ = send_status_tx.send(format!(
                                        "[WARNING] Frame size {} bytes exceeds safe UDP limit ({} bytes) - packet loss likely. Use smaller dimensions.",
                                        data.len(),
                                        MAX_UDP_PACKET_SIZE
                                    ));
                                }

                                let _ = udp_send.send(&data).await;

                                // send compression stats every 100 frames
                                if frame_serializer.total_frames % 100 == 0 && frame_serializer.total_frames > 0 {
                                    let avg_bytes = frame_serializer.total_bytes / frame_serializer.total_frames;
                                    let compression = if frame_serializer.diff_frames > 0 {
                                        100.0 * (frame_serializer.diff_frames as f64) / (frame_serializer.total_frames as f64)
                                    } else {
                                        0.0
                                    };
                                    let status_msg = format!(
                                        "[SEND] {} frames | {} full / {} diff ({:.1}% diff) | avg {} bytes | last: {} bytes",
                                        frame_serializer.total_frames,
                                        frame_serializer.full_frames,
                                        frame_serializer.diff_frames,
                                        compression,
                                        avg_bytes,
                                        data.len()
                                    );
                                    let _ = send_status_tx.send(status_msg);
                                }
                            }
                            Err(broadcast::error::RecvError::Closed) => {
                                break;
                            }
                            _ => {}
                        },
                        _ = send_cancel.cancelled() => break
                    }
                }

                // TODO: look at notes "Current Caveats of TextFrame"
            }
        });

        // === FRAME GENERATION (WEBCAM OR TEST PATTERN) ==========================================
        // from either a mock frame generator or the camera,
        // create the ASCII frames to send to the peer.

        // get camera dimensions based on source type
        let (camera_width, camera_height) = self.get_camera_dimensions();
        if let Some(pattern) = &self.test_pattern {
            let mut frame_gen = MockFrameGenerator::new(
                self.config.ascii.width,
                self.config.ascii.height,
                self.config.performance.fps as u32,
                pattern.clone(),
            )?;

            while *self.conn_flag_rx.borrow() {
                if *self.peer_flag_rx.borrow() {
                    let frame = frame_gen.generate_frame()?;
                    let _ = frame_tx.send(frame);
                }
            }
        } else {
            let mut camera = Camera::from_config(&self.config)?;

            let mut image_frame = ImageFrame::new(camera_width, camera_height, 3)?;
            let mut ascii_frame =
                TextFrame::new(self.config.ascii.width, self.config.ascii.height, ' ')?;

            let converter = AsciiConverter::new(
                self.config.ascii.chars.intensity.chars().count(),
                self.config.ascii.chars.horizontal_lines.chars().count(),
                camera_width,
                camera_height,
                self.config.image_processing.edge_threshold,
                self.config.image_processing.contrast,
                self.config.image_processing.brightness,
            )?;

            while *self.conn_flag_rx.borrow() && !self.cancel_token.is_cancelled() {
                if *self.peer_flag_rx.borrow() {
                    camera.capture_frame(&mut image_frame)?;
                    converter.convert(&image_frame, &mut ascii_frame)?;
                    let _ = frame_tx.send(ascii_frame.clone());
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
        // get camera dimensions based on source type
        let (camera_width, camera_height) = self.get_camera_dimensions();

        // create channel for input commands
        let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel::<TuiCommand>();

        // spawn input handling task
        let input_cancel = self.cancel_token.clone();
        task::spawn(async move {
            let mut reader = EventStream::new();
            loop {
                tokio::select! {
                    maybe_event = reader.next() => {
                        match maybe_event {
                            Some(Ok(Event::Key(key_event))) => {
                                let cmd = match key_event.code {
                                    KeyCode::Char('b') => Some(TuiCommand::ToggleBorder),
                                    KeyCode::Char('d') => Some(TuiCommand::ToggleDebug),
                                    KeyCode::Char('q') => Some(TuiCommand::Quit),
                                    KeyCode::Char('c') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                                        Some(TuiCommand::Quit)
                                    }
                                    _ => None,
                                };
                                if let Some(cmd) = cmd {
                                    let _ = cmd_tx.send(cmd);
                                }
                            }
                            Some(Err(_)) | None => break,
                            _ => {}
                        }
                    }
                    _ = input_cancel.cancelled() => break,
                }
            }
        });

        // create TUI layout with initial dimensions
        let layout = TuiLayout::new(self.config.ascii.width, self.config.ascii.height);

        if let Some(pattern) = &self.test_pattern {
            // mock pattern mode
            let mut frame_gen = MockFrameGenerator::new(
                self.config.ascii.width,
                self.config.ascii.height,
                self.config.performance.fps as u32,
                pattern.clone(),
            )?;

            let mut renderer = TextRenderer::new_with_layout(
                self.config.ascii.chars.intensity.chars().collect(),
                self.config.ascii.chars.horizontal_lines.chars().collect(),
                self.config.ascii.chars.vertical_lines.chars().collect(),
                self.config.ascii.chars.forward_diagonal.chars().collect(),
                self.config.ascii.chars.back_diagonal.chars().collect(),
                layout,
            )?;

            // performance tracking
            let frame_interval = Duration::from_millis(1000 / self.config.performance.fps as u64);
            let mut next_frame_time = Instant::now() + frame_interval;
            let mut frame_count = 0u64;
            let mut fps_timer = Instant::now();
            let mut stats = PerformanceStats::default();

            loop {
                // check for cancellation
                if self.cancel_token.is_cancelled() {
                    return Ok(());
                }

                // handle input commands
                while let Ok(cmd) = cmd_rx.try_recv() {
                    match cmd {
                        TuiCommand::ToggleBorder => renderer.toggle_border(),
                        TuiCommand::ToggleDebug => renderer.toggle_debug(),
                        TuiCommand::Quit => {
                            self.cancel_token.cancel();
                            return Ok(());
                        }
                    }
                }

                let frame_start = Instant::now();
                let frame = frame_gen.generate_frame()?;
                renderer.render(&frame)?;

                // update stats
                frame_count += 1;
                let elapsed = fps_timer.elapsed().as_secs_f32();
                if elapsed >= 1.0 {
                    stats.fps = frame_count as f32 / elapsed;
                    frame_count = 0;
                    fps_timer = Instant::now();
                }
                stats.frame_time_ms = frame_start.elapsed().as_secs_f32() * 1000.0;

                // render debug pane if visible
                if renderer.is_debug_visible() {
                    let _ = renderer.render_debug_pane(&stats);
                }

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

            let mut image_frame = ImageFrame::new(camera_width, camera_height, 3)?;
            let mut ascii_frame =
                TextFrame::new(self.config.ascii.width, self.config.ascii.height, ' ')?;

            let converter = AsciiConverter::new(
                self.config.ascii.chars.intensity.chars().count(),
                self.config.ascii.chars.horizontal_lines.chars().count(),
                camera_width,
                camera_height,
                self.config.image_processing.edge_threshold,
                self.config.image_processing.contrast,
                self.config.image_processing.brightness,
            )?;

            let mut renderer = TextRenderer::new_with_layout(
                self.config.ascii.chars.intensity.chars().collect(),
                self.config.ascii.chars.horizontal_lines.chars().collect(),
                self.config.ascii.chars.vertical_lines.chars().collect(),
                self.config.ascii.chars.forward_diagonal.chars().collect(),
                self.config.ascii.chars.back_diagonal.chars().collect(),
                layout,
            )?;

            // performance tracking
            let frame_interval = Duration::from_millis(1000 / self.config.performance.fps);
            let mut next_frame_time = Instant::now() + frame_interval;
            let mut duplicate_count = 0u64;
            let mut frame_count = 0u64;
            let mut fps_timer = Instant::now();
            let mut stats = PerformanceStats::default();

            loop {
                // check for cancellation
                if self.cancel_token.is_cancelled() {
                    return Ok(());
                }

                // handle input commands
                while let Ok(cmd) = cmd_rx.try_recv() {
                    match cmd {
                        TuiCommand::ToggleBorder => renderer.toggle_border(),
                        TuiCommand::ToggleDebug => renderer.toggle_debug(),
                        TuiCommand::Quit => {
                            self.cancel_token.cancel();
                            return Ok(());
                        }
                    }
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

                // update stats
                frame_count += 1;
                let elapsed = fps_timer.elapsed().as_secs_f32();
                if elapsed >= 1.0 {
                    stats.fps = frame_count as f32 / elapsed;
                    frame_count = 0;
                    fps_timer = Instant::now();
                }
                stats.frame_time_ms = frame_start.elapsed().as_secs_f32() * 1000.0;

                // render debug pane if visible
                if renderer.is_debug_visible() {
                    let _ = renderer.render_debug_pane(&stats);
                }

                // calculate actual frame processing time
                let now = Instant::now();
                let frame_time = (now - frame_start).as_millis();

                let (action, message) = Self::check_frame_timing(now, next_frame_time, frame_time, frame_interval);

                if let Some(msg) = message {
                    let _ = renderer.write_status_message(&format!("[SOLO] {}", msg));
                }

                match action {
                    FrameTimingAction::Reset => {
                        next_frame_time = now + frame_interval;
                    }
                    FrameTimingAction::Sleep(duration) => {
                        tokio::select! {
                            _ = sleep(duration) => {}
                            _ = self.cancel_token.cancelled() => {
                                return Ok(());
                            }
                        }
                        next_frame_time += frame_interval;
                    }
                    FrameTimingAction::Continue => {
                        next_frame_time += frame_interval;
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