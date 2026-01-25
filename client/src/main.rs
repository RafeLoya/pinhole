extern crate alloc;

mod text_converter;
mod text_renderer;
mod camera;
mod client;
mod terminal;
mod config;
mod edge_detector;
mod ffmpeg;
mod image_frame;
mod mock_frame_generator;
mod room_client;

use crate::client::Client;
use crate::config::{DimensionPreset, PinholeConfig};
use crate::mock_frame_generator::PatternType;
use crate::room_client::RoomClient;
use crate::terminal::TerminalInfo;
use clap::{Parser, Subcommand, ValueEnum};
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::error::Error;
use std::io::stdout;
use std::path::PathBuf;
use tokio::signal;
use tokio_util::sync::CancellationToken;

/// Guard that restores terminal state on drop.
struct TerminalGuard;

impl TerminalGuard {
    fn new() -> Result<Self, Box<dyn Error>> {
        enable_raw_mode()?;
        execute!(stdout(), EnterAlternateScreen)?;
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = execute!(stdout(), LeaveAlternateScreen);
        let _ = disable_raw_mode();
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, ValueEnum)]
enum TestPattern {
    /// Checkerboard pattern
    Checkerboard,
    /// Horizontal line moving from top to bottom
    MovingLine,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, ValueEnum)]
enum SourceType {
    /// Webcam capture
    Webcam,
    /// Screen capture
    Screen,
}

impl From<TestPattern> for PatternType {
    fn from(pattern: TestPattern) -> Self {
        match pattern {
            TestPattern::Checkerboard => PatternType::Checkerboard,
            TestPattern::MovingLine => PatternType::MovingLine,
        }
    }
}

/// Terminal-based video calling client.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Configuration file path
    #[arg(short = 'c', long, default_value = "pinhole.toml", global = true)]
    config: PathBuf,

    /// Test pattern (if not using a camera)
    #[arg(short = 'p', long, global = true)]
    test_pattern: Option<TestPattern>,

    /// Dimension preset (small, medium, large, xlarge)
    #[arg(long, global = true)]
    preset: Option<String>,

    /// Render window width (overrides config and preset)
    #[arg(short = 'W', long, global = true)]
    width: Option<usize>,

    /// Render window height (overrides config and preset)
    #[arg(short = 'H', long, global = true)]
    height: Option<usize>,

    /// Video source type (overrides config)
    #[arg(short = 's', long, global = true)]
    source: Option<SourceType>,

    /// Disable edge detection (improves performance at high resolutions)
    #[arg(long, global = true)]
    no_edges: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Host a session and generate a room code for others to join.
    Host {
        /// Room API server URL
        #[arg(long, default_value = "http://localhost:8000")]
        api_url: String,

        /// TCP server address
        #[arg(short = 't', long, default_value = "127.0.0.1:8080")]
        tcp_addr: String,

        /// UDP server address
        #[arg(short = 'u', long, default_value = "127.0.0.1:4433")]
        udp_addr: String,
    },

    /// Join a session using a room code.
    Join {
        /// The room code to join (e.g., swift-river-42)
        room_code: String,

        /// Room API server URL
        #[arg(long, default_value = "http://localhost:8000")]
        api_url: String,

        /// TCP server address
        #[arg(short = 't', long, default_value = "127.0.0.1:8080")]
        tcp_addr: String,

        /// UDP server address
        #[arg(short = 'u', long, default_value = "127.0.0.1:4433")]
        udp_addr: String,
    },

    /// Local preview without server connection.
    Solo,

    /// Direct connection with manual session ID (legacy mode).
    Connect {
        /// TCP server address
        #[arg(short = 't', long)]
        tcp_addr: Option<String>,

        /// UDP server address
        #[arg(short = 'u', long)]
        udp_addr: Option<String>,

        /// Session ID to join
        #[arg(short = 's', long)]
        session_id: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    // === SHUTDOWN HANDLER ========================================================================
    let cancel_token = CancellationToken::new();
    let cancel_token_clone = cancel_token.clone();

    tokio::spawn(async move {
        if signal::ctrl_c().await.is_ok() {
            cancel_token_clone.cancel();
        }
    });

    // === CONFIGURATION ===========================================================================
    let mut config = if args.config.exists() {
        println!("Loading config from: {}", args.config.display());
        PinholeConfig::from_file(&args.config)?
    } else {
        println!(
            "Config file not found at {}, using defaults",
            args.config.display()
        );
        PinholeConfig::default()
    };

    // detect terminal capabilities
    let _term_info = TerminalInfo::detect(config.terminal.clone())?;

    apply_dimension_overrides(&mut config, &args);
    apply_source_override(&mut config, &args);
    apply_image_processing_overrides(&mut config, &args);

    let pattern_type = args.test_pattern.map(PatternType::from);
    if pattern_type.is_some() {
        println!("Using test pattern: {:?}", args.test_pattern);
    }

    // === COMMAND DISPATCH ========================================================================
    match args.command {
        Command::Host {
            api_url,
            tcp_addr,
            udp_addr,
        } => {
            run_host(config, pattern_type, cancel_token, &api_url, &tcp_addr, &udp_addr).await?;
        }

        Command::Join {
            room_code,
            api_url,
            tcp_addr,
            udp_addr,
        } => {
            run_join(
                config,
                pattern_type,
                cancel_token,
                &room_code,
                &api_url,
                &tcp_addr,
                &udp_addr,
            )
            .await?;
        }

        Command::Solo => {
            run_solo(config, pattern_type, cancel_token).await?;
        }

        // legacy session join, join command preferred
        Command::Connect {
            tcp_addr,
            udp_addr,
            session_id,
        } => {
            run_connect(config, pattern_type, cancel_token, tcp_addr, udp_addr, session_id).await?;
        }
    }

    println!("pinhole gracefully shut down");
    Ok(())
}

/// Applies dimension overrides from CLI arguments.
fn apply_dimension_overrides(config: &mut PinholeConfig, args: &Args) {
    if let Some(preset_str) = &args.preset {
        if let Some(preset) = DimensionPreset::from_str(preset_str) {
            let (w, h) = preset.dimensions();
            config.ascii.width = w;
            config.ascii.height = h;
            println!("Using dimension preset '{}': {}x{}", preset_str, w, h);

            if !preset.is_udp_safe() {
                eprintln!(
                    "WARNING: Frame size ~{} bytes exceeds safe UDP limit (1400 bytes)",
                    preset.frame_size()
                );
            }
        } else {
            eprintln!("Warning: Unknown preset '{}', ignoring", preset_str);
        }
    }

    if let Some(width) = args.width {
        config.ascii.width = width;
        println!("ASCII width override: {}", width);
    }

    if let Some(height) = args.height {
        config.ascii.height = height;
        println!("ASCII height override: {}", height);
    }

    // warn about custom dimensions
    if (args.width.is_some() || args.height.is_some()) && args.preset.is_none() {
        let frame_size = 17 + (config.ascii.width * config.ascii.height);
        if frame_size > 1400 {
            eprintln!(
                "WARNING: Frame size ~{} bytes exceeds safe UDP limit (1400 bytes)",
                frame_size
            );
        }
    }
}

/// Applies source type override from CLI arguments.
fn apply_source_override(config: &mut PinholeConfig, args: &Args) {
    if let Some(source) = &args.source {
        let source_str = match source {
            SourceType::Webcam => "webcam",
            SourceType::Screen => "screen",
        };
        config.video.source.r#type = source_str.to_string();
        println!("Video source override: {}", source_str);
    }
}

/// Applies image processing overrides from CLI arguments.
fn apply_image_processing_overrides(config: &mut PinholeConfig, args: &Args) {
    if args.no_edges {
        config.image_processing.edge_detection = false;
        println!("Edge detection disabled");
    }
}

/// Host a session: create room code and wait for peer.
async fn run_host(
    config: PinholeConfig,
    pattern_type: Option<PatternType>,
    cancel_token: CancellationToken,
    api_url: &str,
    tcp_addr: &str,
    udp_addr: &str,
) -> Result<(), Box<dyn Error>> {
    println!("Registering room with server...");

    let room_client = RoomClient::new(api_url);

    // check server health first
    if !room_client.health_check().await {
        return Err(format!("Cannot reach room API at {}", api_url).into());
    }

    // create room
    let room = room_client.create_room().await?;
    println!();
    println!("Room Code: {:^25}", room.room_code);
    println!();
    println!("Share this code with your peer to connect.");
    println!("Press 'q' to quit, 'b' to toggle border, 'd' to toggle debug");
    println!("Waiting for peer...");
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let _terminal_guard = TerminalGuard::new()?;

    let client = Client::new(
        tcp_addr.to_string(),
        udp_addr.to_string(),
        room.session_id,
        pattern_type,
        config,
        cancel_token.clone(),
    );

    // tokio::select! {
    //     result = client.run() => {
    //         if let Err(e) = result {
    //             drop(_terminal_guard);
    //             eprintln!("Error: {}", e);
    //         }
    //     }
    //     _ = cancel_token.cancelled() => {}
    // }
    cleanup(cancel_token, _terminal_guard, client).await;

    Ok(())
}

/// Join a session using a room code.
async fn run_join(
    config: PinholeConfig,
    pattern_type: Option<PatternType>,
    cancel_token: CancellationToken,
    room_code: &str,
    api_url: &str,
    tcp_addr: &str,
    udp_addr: &str,
) -> Result<(), Box<dyn Error>> {
    println!("Looking up room code: {}", room_code);

    let room_client = RoomClient::new(api_url);

    // lookup room
    let room = room_client.lookup_room(room_code).await?;
    println!("Found session: {}", room.session_id);
    println!("Press 'q' to quit, 'b' to toggle border, 'd' to toggle debug");
    println!("Connecting...");
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    let _terminal_guard = TerminalGuard::new()?;

    let client = Client::new(
        tcp_addr.to_string(),
        udp_addr.to_string(),
        room.session_id,
        pattern_type,
        config,
        cancel_token.clone(),
    );

    // tokio::select! {
    //     result = client.run() => {
    //         if let Err(e) = result {
    //             drop(_terminal_guard);
    //             eprintln!("Error: {}", e);
    //         }
    //     }
    //     _ = cancel_token.cancelled() => {}
    // }
    cleanup(cancel_token, _terminal_guard, client).await;

    Ok(())
}

async fn cleanup(cancel_token: CancellationToken, _terminal_guard: TerminalGuard, client: Client) {
    tokio::select! {
        result = client.run() => {
            if let Err(e) = result {
                drop(_terminal_guard);
                eprintln!("Error: {}", e);
            }
        }
        _ = cancel_token.cancelled() => {}
    }
}

/// Run in solo mode (local preview).
async fn run_solo(
    config: PinholeConfig,
    pattern_type: Option<PatternType>,
    cancel_token: CancellationToken,
) -> Result<(), Box<dyn Error>> {
    println!("Running in solo mode (local preview only)");
    println!("Press 'q' to quit, 'b' to toggle border, 'd' to toggle debug");
    println!("Starting in 1 second...");
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    let _terminal_guard = TerminalGuard::new()?;

    let client = Client::new(
        String::new(),
        String::new(),
        String::new(),
        pattern_type,
        config,
        cancel_token.clone(),
    );

    tokio::select! {
        result = client.run_solo() => {
            if let Err(e) = result {
                drop(_terminal_guard);
                eprintln!("Error in solo mode: {}", e);
            }
        }
        _ = cancel_token.cancelled() => {}
    }

    Ok(())
}

/// Direct connection with manual session ID (legacy mode).
async fn run_connect(
    mut config: PinholeConfig,
    pattern_type: Option<PatternType>,
    cancel_token: CancellationToken,
    tcp_addr: Option<String>,
    udp_addr: Option<String>,
    session_id: Option<String>,
) -> Result<(), Box<dyn Error>> {
    // apply overrides from CLI
    if let Some(addr) = tcp_addr {
        config.network.tcp_addr = addr;
    }
    if let Some(addr) = udp_addr {
        config.network.udp_addr = addr;
    }
    if let Some(id) = session_id {
        config.network.session_id = id;
    }

    // generate random session ID if not provided
    let session_id = if config.network.session_id.is_empty() {
        let rand_id: u32 = rand::random();
        format!("session-{}", rand_id)
    } else {
        config.network.session_id.clone()
    };

    println!("Connecting to session: {}", session_id);
    println!("Press 'q' to quit, 'b' to toggle border, 'd' to toggle debug");
    println!("Starting in 1 second...");
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    let _terminal_guard = TerminalGuard::new()?;

    let client = Client::new(
        config.network.tcp_addr.clone(),
        config.network.udp_addr.clone(),
        session_id,
        pattern_type,
        config,
        cancel_token.clone(),
    );

    // tokio::select! {
    //     result = client.run() => {
    //         if let Err(e) = result {
    //             drop(_terminal_guard);
    //             eprintln!("Error in network mode: {}", e);
    //         }
    //     }
    //     _ = cancel_token.cancelled() => {}
    // }
    cleanup(cancel_token, _terminal_guard, client).await;

    Ok(())
}
