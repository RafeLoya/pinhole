mod server;
mod sessions;
mod sfu;

use crate::sfu::SFU;
use clap::{ArgAction, Parser};
use std::error::Error;
use tokio::io::AsyncReadExt;

/// Simple TCP/UDP server with configurable logging
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Parse command line arguments
    let args = Args::parse();

    let server = SFU::new(
        args.tcp_addr,
        args.udp_addr,
        args.log_file,
        args.verbose,
    );
    
    server.run().await?;
    
    Ok(())
}
