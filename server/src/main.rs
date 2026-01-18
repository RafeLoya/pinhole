mod room_api;
mod room_registry;
mod sessions;
mod sfu;

use crate::room_api::run_api_server;
use crate::room_registry::RoomRegistry;
use crate::sfu::SFU;
use clap::{ArgAction, Parser};
use std::error::Error;
use std::sync::Arc;
use std::time::Duration;

/// SFU server for terminal-based video calls.
///
/// If you want to test locally, can simply use:
///
/// ```bash
/// cargo run --release --bin pinhole-server
/// ```
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// TCP server bind address (control channel)
    #[arg(short, long, default_value = "0.0.0.0:8080")]
    tcp_addr: String,

    /// UDP server bind address (frame forwarding)
    #[arg(short, long, default_value = "0.0.0.0:4433")]
    udp_addr: String,

    /// HTTP server bind address (room API)
    #[arg(long, default_value = "0.0.0.0:8000")]
    http_addr: String,

    /// Room code TTL in seconds
    #[arg(long, default_value = "3600")]
    room_ttl: u64,

    /// Log file path
    #[arg(short, long, default_value = "debug.log")]
    log_file: String,

    /// Enable verbose output
    #[arg(short, long, action = ArgAction::SetTrue)]
    verbose: bool,
}

/// Entry point for ASCII video SFU server (codename "Pinhole")
///
/// Launches:
/// - TCP listener for control messages (JOIN, LEAVE, etc.)
/// - UDP listener for forwarding ASCII frames between peers
/// - HTTP server for room code registration and lookup
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    // create room registry with TTL
    let room_ttl = Duration::from_secs(args.room_ttl);
    let registry = Arc::new(RoomRegistry::new(room_ttl));

    // spawn background cleanup task (runs every 5 minutes)
    registry.spawn_cleanup_task(Duration::from_secs(300));

    // spawn HTTP API server
    let http_addr: std::net::SocketAddr = args.http_addr.parse()?;
    let http_registry = Arc::clone(&registry);
    tokio::spawn(async move {
        if let Err(e) = run_api_server(http_addr, http_registry).await {
            eprintln!("[HTTP] server error: {}", e);
        }
    });

    // run SFU (TCP/UDP)
    let server = SFU::new(args.tcp_addr, args.udp_addr, args.log_file, args.verbose);
    server.run().await?;

    Ok(())
}
