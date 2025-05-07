mod sessions;
mod sfu;

use crate::sfu::SFU;
use clap::{ArgAction, Parser};
use std::error::Error;

/// Simple TCP/UDP server with configurable logging
///
/// If you want to test locally, can simply use:
///
/// ```bash
/// cargo run --bin server
/// ```
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// TCP server bind address
    #[arg(short, long, default_value = "0.0.0.0:8080")]
    tcp_addr: String,

    /// UDP server bind address
    #[arg(short, long, default_value = "0.0.0.0:4433")]
    udp_addr: String,

    /// Log file path
    #[arg(short, long, default_value = "debug.log")]
    log_file: String,

    /// Enable verbose output
    #[arg(short, long, action = ArgAction::SetTrue)]
    verbose: bool,
}

/// Entry point for ASCII video SFU server (codename "Pinhole")
///
/// Launches TCP and UDP listeners, where
/// - TCP is used for control messages, managing session state and other logic
/// (e.g. JOIN, LEAVE, etc.)
/// - UDP is used for forwarding ASCII frames between peers
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Parse command line arguments
    let args = Args::parse();

    let server = SFU::new(args.tcp_addr, args.udp_addr, args.log_file, args.verbose);

    server.run().await?;

    Ok(())
}
