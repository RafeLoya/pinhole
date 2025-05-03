extern crate alloc;

mod ffmpeg;
mod camera;
mod ascii_renderer;
mod image_frame;
mod ascii_converter;
mod edge_detector;
mod video_config;
mod client;

use clap::Parser;
use rand::{Rng};
use crate::client::Client;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// TCP server bind address
    //#[arg(short, long, default_value = "0.0.0.0:8080")]
    #[arg(short, long, default_value = "127.0.0.1:8080")]
    tcp_addr: String,

    /// UDP server bind address
    // #[arg(short, long, default_value = "0.0.0.0:4433")]
    #[arg(short, long, default_value = "127.0.0.1:4433")]
    udp_addr: String,
    
    /// Session ID to join (random if not given)
    #[arg(short, long, default_value = "")]
    session_id: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    
    let args = Args::parse();
    
    let session_id = if args.session_id.is_empty() {
        let rand_id: u32 = rand::rng().random();
        format!("session-{}", rand_id)
    } else {
        args.session_id.clone()
    };
    
    println!("connection to session: {}", session_id);
    
    let client = Client::new(
        args.tcp_addr,
        args.udp_addr,
        args.session_id,
    );
    
    client.run().await?;
    
    Ok(())
}
