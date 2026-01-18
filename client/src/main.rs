extern crate alloc;

mod ascii_converter;
mod ascii_renderer;
mod camera;
mod client;
mod terminal;
mod config;
mod edge_detector;
mod ffmpeg;
mod image_frame;
mod mock_frame_generator;

use crate::client::Client;
use crate::config::PinholeConfig;
use crate::mock_frame_generator::PatternType;
use clap::{Parser, ValueEnum};
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use rand::Rng;
use std::error::Error;
use std::io::stdout;
use std::path::PathBuf;
use tokio::signal;
use tokio_util::sync::CancellationToken;
use terminal::TerminalInfo;

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

impl From<TestPattern> for PatternType {
    fn from(pattern: TestPattern) -> Self {
        match pattern {
            TestPattern::Checkerboard => PatternType::Checkerboard,
            TestPattern::MovingLine => PatternType::MovingLine,
        }
    }
}

/// If wanting to test locally with your webcam, enter the following:
///
/// ```bash
/// # solo mode (local preview, no server connection)
/// cargo run --release --bin pinhole -- --solo
/// ```
///
/// To connect to a session with a live server, enter the following:
///
/// ```bash
/// # Network mode
/// cargo run --release --bin pinhole -- -t <TCP_PORT> -u <UDP_PORT> -s <SESSION_ID> -p <PATTERN_TYPE>
/// ```
///
/// where:
/// - `TCP_PORT` and `UDP_PORT` is port of your choosing on 127.0.0.1
/// - `SESSION_ID` can be any string (for now)
/// - `PATTERN_TYPE` can be either "`checkerboard`" or "`moving-line`"
#[derive(Parser, Debug)]
#[command(version, about, long_about = None, disable_help_flag = true)]
struct Args {
    /// Print help information
    #[arg(long)]
    help: bool,

    /// Configuration file path
    #[arg(short = 'c', long, default_value = "pinhole.toml")]
    config: PathBuf,

    /// Solo mode - local preview without server connection
    #[arg(long)]
    solo: bool,

    /// TCP server bind address (overrides config)
    #[arg(short = 't', long)]
    tcp_addr: Option<String>,

    /// UDP server bind address (overrides config)
    #[arg(short = 'u', long)]
    udp_addr: Option<String>,

    /// Session ID to join (random if not given, overrides config)
    #[arg(short = 's', long)]
    session_id: Option<String>,

    /// Test pattern (if not using a camera)
    #[arg(short = 'p', long)]
    test_pattern: Option<TestPattern>,

    /// Dimension preset (small, medium, large, xlarge)
    #[arg(long)]
    preset: Option<String>,

    /// Render window width (overrides config and preset)
    #[arg(short = 'w', long)]
    width: Option<usize>,

    /// Render window height (overrides config and preset)
    #[arg(short = 'h', long)]
    height: Option<usize>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    // handle manual help flag
    if args.help {
        use clap::CommandFactory;
        Args::command().print_help()?;
        return Ok(());
    }

    // === SHUTDOWN HANDLER =======================================================================
    // create cancellation token for graceful shutdown coordination
    let cancel_token = CancellationToken::new();
    let cancel_token_clone = cancel_token.clone();

    // CTRL+C signal handler
    tokio::spawn(async move {
        if signal::ctrl_c().await.is_ok() {
            cancel_token_clone.cancel();
        }
    });

    // === CONFIGURATION ==========================================================================
    // load configuration from file or use defaults
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
    
    // TODO! make use of terminal info for config
    let _term_info = TerminalInfo::detect(config.terminal.clone())?;

    // === CLI ARGUMENTS ==========================================================================
    // CLI arguments override config file settings
    if let Some(tcp_addr) = args.tcp_addr {
        config.network.tcp_addr = tcp_addr;
    }

    if let Some(udp_addr) = args.udp_addr {
        config.network.udp_addr = udp_addr;
    }

    if let Some(session_id) = args.session_id {
        config.network.session_id = session_id;
    }

    let pattern_type = args.test_pattern.map(|p| PatternType::from(p));
    if pattern_type.is_some() {
        println!("using test pattern: {:?}", args.test_pattern);
    }

    // apply dimension overrides (preset first, then explicit width/height)
    if let Some(preset_str) = &args.preset {
        if let Some(preset) = config::DimensionPreset::from_str(preset_str) {
            let (w, h) = preset.dimensions();
            config.ascii.width = w;
            config.ascii.height = h;
            println!("Using dimension preset '{}': {}x{}", preset_str, w, h);

            // warn if not UDP safe
            if !preset.is_udp_safe() {
                eprintln!(
                    "WARNING: Frame size ~{} bytes exceeds safe UDP limit (1400 bytes)",
                    preset.frame_size()
                );
                eprintln!("    Expect packet loss and rendering issues over UDP.");
                eprintln!("    Consider using 'small' or 'medium' presets for reliable streaming.");
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

    // warn about custom dimensions if they're too large
    if (args.width.is_some() || args.height.is_some()) && args.preset.is_none() {
        let frame_size = 17 + (config.ascii.width * config.ascii.height);
        if frame_size > 1400 {
            eprintln!(
                "WARNING: Frame size ~{} bytes ({}x{}) exceeds safe UDP limit (1400 bytes)",
                frame_size, config.ascii.width, config.ascii.height
            );
            eprintln!("         Expect packet loss and rendering issues over UDP.");
            eprintln!("         Keep dimensions under ~37x37 for reliable streaming.");
        }
    }

    // === SOLO (PREVIEW) MODE ====================================================================
    // local preview without network connection
    if args.solo {
        println!("Running in solo mode (local preview only)");
        println!("Press 'q' to quit, 'b' to toggle border, 'd' to toggle debug");
        println!("Starting in 1 second...");
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        // enter TUI mode (raw mode + alternate screen)
        let _terminal_guard = TerminalGuard::new()?;

        let client = Client::new(
            String::new(), // no TCP addr needed
            String::new(), // no UDP addr needed
            String::new(), // no session ID needed
            pattern_type,
            config,
            cancel_token.clone(),
        );

        tokio::select! {
            result = client.run_solo() => {
                if let Err(e) = result {
                    // drop guard first to restore terminal before printing error
                    drop(_terminal_guard);
                    eprintln!("Error in solo mode: {}", e);
                }
            }
            _ = cancel_token.cancelled() => {
                // terminal guard will restore on drop
            }
        }
    } else {
        // === NETWORK MODE =======================================================================
        // connect to server and peer
        // generate random session ID if not provided
        let session_id = if config.network.session_id.is_empty() {
            let rand_id: u32 = rand::rng().random();
            format!("session-{}", rand_id)
        } else {
            config.network.session_id.clone()
        };

        println!("Connecting to session: {}", session_id);
        println!("Press 'q' to quit, 'b' to toggle border, 'd' to toggle debug");
        println!("Starting in 1 second...");
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        // enter TUI mode (raw mode + alternate screen)
        let _terminal_guard = TerminalGuard::new()?;

        let client = Client::new(
            config.network.tcp_addr.clone(),
            config.network.udp_addr.clone(),
            session_id.clone(),
            pattern_type,
            config,
            cancel_token.clone(),
        );

        tokio::select! {
            result = client.run() => {
                if let Err(e) = result {
                    // drop guard first to restore terminal before printing error
                    drop(_terminal_guard);
                    eprintln!("Error in network mode: {}", e);
                }
            }
            _ = cancel_token.cancelled() => {
                // terminal guard will restore on drop
            }
        }
    }

    println!("pinhole gracefully shut down");
    Ok(())
}
