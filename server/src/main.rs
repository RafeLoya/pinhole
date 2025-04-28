use clap::{ArgAction, Parser};
use common::logger::Logger;
use std::error::Error;
use tokio::{
    io::AsyncReadExt,
    net::{TcpListener, UdpSocket},
    task,
};

/// Simple TCP/UDP server with configurable logging
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// TCP server bind address
    #[arg(short, long, default_value = "0.0.0.0:8080")]
    tcp_addr: String,

    /// UDP server bind address
    #[arg(short, long, default_value = "0.0.0.0:443")]
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

    // Initialize logger with custom file path
    let logger = Logger::with_file_name(&args.log_file)?;
    logger.info("Starting TCP/UDP server")?;

    if args.verbose {
        println!("Server starting with configuration:");
        println!("  TCP Address: {}", args.tcp_addr);
        println!("  UDP Address: {}", args.udp_addr);
        println!("  Log File: {}", args.log_file);
    } else {
        println!("Server up!");
    }

    // Start UDP listener on a separate task
    let udp_addr = args.udp_addr.clone();
    let log_file_path = args.log_file.clone();
    task::spawn(async move {
        // Create a logger for the UDP server
        let udp_logger = match Logger::with_file_name(&log_file_path) {
            Ok(l) => l,
            Err(e) => {
                println!("Error creating UDP logger: {}", e);
                return;
            }
        };

        if let Err(e) = start_udp_server(udp_logger, &udp_addr).await {
            println!("UDP server error: {}", e);
        }
    });

    // Start TCP listener
    start_tcp_server(logger, &args.tcp_addr, &args.log_file).await?;
    Ok(())
}

async fn start_tcp_server(
    logger: Logger,
    addr: &str,
    log_file_path: &str,
) -> Result<(), Box<dyn Error>> {
    // Bind TCP listener to specified address
    let listener = TcpListener::bind(addr).await?;
    logger.info(&format!("TCP server listening on: {}", addr))?;
    loop {
        // Accept incoming TCP connections
        let (mut socket, client_addr) = listener.accept().await?;
        logger.info(&format!("New TCP connection from: {}", client_addr))?;

        // Pass the log file path to the new task
        let log_path = log_file_path.to_string();

        // Handle connection in a separate task
        task::spawn(async move {
            // Create a new logger for this connection
            let conn_logger = match Logger::with_file_name(&log_path) {
                Ok(l) => l,
                Err(e) => {
                    println!("Error creating connection logger: {}", e);
                    return;
                }
            };

            let mut buffer = vec![0u8; 4096];
            loop {
                match socket.read(&mut buffer).await {
                    Ok(0) => {
                        // Connection closed
                        if let Err(e) =
                            conn_logger.info(&format!("TCP connection closed: {}", client_addr))
                        {
                            println!("Logging error: {}", e);
                        }
                        break;
                    }
                    Ok(n) => {
                        // Got data, log the packet contents
                        let packet_content =
                            format!("TCP packet from {}: {:?}", client_addr, &buffer[..n]);
                        if let Err(e) = conn_logger.info(&packet_content) {
                            println!("Logging error: {}", e);
                        }
                        // Also print to stdout
                        println!("{}", packet_content);
                    }
                    Err(e) => {
                        // Error reading from socket
                        if let Err(log_err) =
                            conn_logger.error(&format!("Error reading from TCP socket: {}", e))
                        {
                            println!("Logging error: {}", log_err);
                        }
                        break;
                    }
                }
            }
        });
    }
}

async fn start_udp_server(logger: Logger, addr: &str) -> Result<(), Box<dyn Error>> {
    // Bind UDP socket to specified address
    let socket = UdpSocket::bind(addr).await?;
    logger.info(&format!("UDP server listening on: {}", addr))?;
    let mut buffer = vec![0u8; 4096];
    loop {
        // Receive UDP packets
        match socket.recv_from(&mut buffer).await {
            Ok((len, addr)) => {
                // Got data, log the packet contents
                let packet_content = format!("UDP packet from {}: {:?}", addr, &buffer[..len]);
                logger.info(&packet_content)?;
                // Also print to stdout
                println!("{}", packet_content);
            }
            Err(e) => {
                // Error receiving from socket
                logger.error(&format!("Error receiving from UDP socket: {}", e))?;
            }
        }
    }
}
