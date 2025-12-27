extern crate alloc;

mod ascii_converter;
mod ascii_renderer;
mod camera;
mod client;
mod config;
mod edge_detector;
mod ffmpeg;
mod image_frame;
mod mock_frame_generator;
mod video_config;

use crate::client::Client;
use crate::config::PinholeConfig;
use crate::mock_frame_generator::PatternType;
use clap::{Parser, ValueEnum};
use rand::Rng;
use std::error::Error;
use std::path::PathBuf;

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
/// # Solo mode (local preview, no server connection)
/// cargo run --bin pinhole -- --solo
/// ```
///
/// To connect to a session with a live server, enter the following:
///
/// ```bash
/// # Network mode
/// cargo run --bin pinhole -- -t <TCP_PORT> -u <UDP_PORT> -s <SESSION_ID> -p <PATTERN_TYPE>
/// ```
///
/// where:
/// - `TCP_PORT` and `UDP_PORT` is port of your choosing on 127.0.0.1
/// - `SESSION_ID` can be any string (for now)
/// - `PATTERN_TYPE` can be either "`checkerboard`" or "`moving-line`"
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
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
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    // Load configuration from file or use defaults
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
    if let Some(_) = &pattern_type {
        println!("using test pattern: {:?}", args.test_pattern);
    }

    // Solo mode - local preview without network
    if args.solo {
        println!("Running in solo mode (local preview only)");
        println!("Press Ctrl+C to exit");

        let client = Client::new(
            String::new(), // No TCP addr needed
            String::new(), // No UDP addr needed
            String::new(), // No session ID needed
            pattern_type,
            config,
        );

        client.run_solo().await?;
    } else {
        // Network mode - connect to server and peer
        // Generate random session ID if not provided
        let session_id = if config.network.session_id.is_empty() {
            let rand_id: u32 = rand::rng().random();
            format!("session-{}", rand_id)
        } else {
            config.network.session_id.clone()
        };

        println!("connection to session: {}", session_id);

        let client = Client::new(
            config.network.tcp_addr.clone(),
            config.network.udp_addr.clone(),
            session_id.clone(),
            pattern_type,
            config,
        );

        client.run().await?;
    }

    Ok(())
}
