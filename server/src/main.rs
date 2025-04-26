use std::error::Error;
use std::net::SocketAddr;
use crate::server::Server;

mod server;

/// TODO: Look into multiple streams to avoid head-of-line blocking: does QUIC / quinn automatically do this

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt::init();
    
    let addr = "[::1]:4433".parse::<SocketAddr>()?;
    let server = Server::new(addr)?;
    
    println!("Server listening on {}", server.local_addr()?);
    
    // blocking call
    server.run().await?;
    
    Ok(())
}
