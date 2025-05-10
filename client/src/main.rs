extern crate alloc;

mod ascii_converter;
mod ascii_renderer;
mod camera;
mod client;
mod edge_detector;
mod ffmpeg;
mod image_frame;
mod mock_frame_generator;
mod video_config;

use crate::client::Client;
use crate::mock_frame_generator::PatternType;
use clap::{Parser, ValueEnum};
use rand::Rng;
use std::error::Error;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, ValueEnum)]
enum TestPattern {
    /// Checkerboard pattern
    Checkerboard,
    /// Horizontal line moving from top to bottom
    MovingLine,
}

// TODO: this is really jank, probably not important tho if we will remove test patterns in future
impl From<TestPattern> for PatternType {
    fn from(pattern: TestPattern) -> Self {
        match pattern {
            TestPattern::Checkerboard => PatternType::Checkerboard,
            TestPattern::MovingLine => PatternType::MovingLine,
        }
    }
}

/// if wanting to test locally, the command would look something like this:
///
/// ```bash
/// cargo run --bin client -- -t <TCP_PORT> -u <UDP_PORT> -s <SESSION_ID> -p <PATTERN_TYPE>
/// ```
///
/// where:
/// - TCP_PORT and UDP_PORT is port of your choosing on 127.0.0.1
/// - SESSION_ID can be any string (for now)
/// - PATTERN_TYPE can be either "checkerboard" or "moving-line"
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// TCP server bind address
    #[arg(short = 't', long, default_value = "127.0.0.1:8080")]
    tcp_addr: String,

    /// UDP server bind address
    #[arg(short = 'u', long, default_value = "127.0.0.1:4433")]
    udp_addr: String,

    /// Session ID to join (random if not given)
    #[arg(short = 's', long, default_value = "")]
    session_id: String,

    /// Test pattern (if not using a camera)
    #[arg(short = 'p', long)]
    test_pattern: Option<TestPattern>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    tracing_subscriber::fmt::init();

    let session_id = if args.session_id.is_empty() {
        let rand_id: u32 = rand::rng().random();
        format!("session-{}", rand_id)
    } else {
        args.session_id.clone()
    };

    println!("connection to session: {}", session_id);

    let pattern_type = args.test_pattern.map(|p| PatternType::from(p));
    if let Some(_) = &pattern_type {
        println!("using test pattern: {:?}", args.test_pattern);
    }

    let client = Client::new(
        args.tcp_addr,
        args.udp_addr,
        session_id.clone(),
        pattern_type,
    );

    client.run().await?;

    Ok(())
}
