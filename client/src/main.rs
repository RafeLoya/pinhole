pub mod ffmpeg;

use std::time::Duration;
use std::{
    io::{self, Write, stdout},
    sync::Arc,
};

use ascii_converter::AsciiConverter;
use ascii_renderer::AsciiRenderer;
use camera::Camera;
use clap::{Parser, ValueEnum};
use common::ascii_frame::AsciiFrame;
use image_frame::ImageFrame;
use mock_frame_generator::{MockFrameGenerator, PatternType};
use tokio::time::sleep;
use video_config::VideoConfig;

mod ascii_converter;
mod ascii_renderer;
mod camera;
mod edge_detector;
mod image_frame;
mod mock_frame_generator;
mod video_config;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpStream, UdpSocket},
};
const FPS: u64 = 30;

const HELLO_BYTE: u8 = 0x69;
const INVALID_RESPONSE_BYTE: u8 = 0x01;
const CONNECTION_REQUEST_BYTE: u8 = 0x42;
const UDP_MESSAGE_BYTE: u8 = 0x34;

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
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let username = prompt_for_username()?;

    let username_to_connect;
    let mut stream;

    loop {
        let addr = args.tcp_addr.clone();
        stream = TcpStream::connect(addr.clone()).await?;

        send_username(&mut stream, &username).await?;
        let all_active_usernames = receive_user_list(&mut stream).await?;
        let other_usernames = all_active_usernames
            .iter()
            .filter(|&user| *user != username)
            .cloned()
            .collect::<Vec<_>>();

        println!("Connected to server at {} as {}", addr, username);

        if other_usernames.is_empty() {
            // Prompt the to try again?
            println!("No other users available. Please try again.");
            println!("Press Enter to try again or Ctrl+C to exit.");
            let mut input = String::new();
            io::stdin().read_line(&mut input).unwrap();
            continue;
        } else {
            username_to_connect = prompt_for_username_to_connect(other_usernames);
            break;
        }
    }

    println!("Connecting to user: {}", username_to_connect);

    let mut connection_request = vec![CONNECTION_REQUEST_BYTE, username_to_connect.len() as u8];
    connection_request.extend_from_slice(username_to_connect.as_bytes());
    stream.write_all(&connection_request).await?;

    handle_connection_response(&mut stream).await?;

    println!("Connection established with user: {}", username_to_connect);

    let udp_addr = "0.0.0.0:0";
    let udp_socket = Arc::new(UdpSocket::bind(udp_addr).await?);

    //send hello udp message as HELLO_BYTE, username.len() as u16 (two bytes), username

    let mut hello_message = vec![HELLO_BYTE];

    let username_bytes = username.as_bytes();
    let username_length = username_bytes.len() as u16;
    if username_length > u16::MAX as u16 {
        return Err("Username too long to send via double-byte length field".into());
    }
    hello_message.write_u16(username_length).await?;
    hello_message.extend_from_slice(username_bytes);
    udp_socket
        .send_to(&hello_message, args.udp_addr.clone())
        .await?;

    let udp_socket_clone = Arc::clone(&udp_socket);

    tokio::spawn(async move {
        let mut buf = vec![0u8; 4096];

        loop {
            match udp_socket.clone().recv_from(&mut buf).await {
                Ok((len, _)) if len > 1 => {
                    
                    let message = &buf[0..len];
                    let message = String::from_utf8_lossy(message);

                    // clear screen and print
                    AsciiRenderer::clear_screen().unwrap();

                    println!("{}", message);
                }
                Ok(_) => {
                    println!("Received empty or invalid UDP message");
                }
                Err(e) => {
                    eprintln!("UDP receive error: {}", e);
                }
            }
        }
    });

    let cfg = VideoConfig::default();
    let frame_interval = Duration::from_millis(1000 / FPS);

    if args.test_pattern.is_some() {
        let mut frame_gen = MockFrameGenerator::new(
            cfg.ascii_width,
            cfg.ascii_height,
            FPS as u32,
            args.test_pattern.unwrap().into(),
        )?;

        loop {
            let frame = frame_gen.generate_frame()?;

            let chars = frame.chars();
            let width = frame.w;
            let frame_string = chars
                .chunks(width)
                .map(|line| line.iter().collect::<String>())
                .collect::<Vec<_>>()
                .join("\n");

            // Send the ASCII frame over UDP
            let mut udp_message = vec![UDP_MESSAGE_BYTE];
            let frame_bytes = frame_string.as_bytes();
            let frame_bytes_len = frame_bytes.len();
            if frame_bytes_len > u16::MAX as usize {
                return Err("Frame too large to send via double-byte length field".into());
            }
            udp_message.write_u16(frame_bytes.iter().len() as u16).await?;
            udp_message.extend_from_slice(frame_bytes);
            udp_socket_clone
                .send_to(&udp_message, args.udp_addr.clone())
                .await?;
            
            sleep(frame_interval).await;
        }
    } else {
        let mut camera = Camera::new(cfg.camera_width, cfg.camera_height)?;
        let mut image_frame = ImageFrame::new(cfg.camera_width, cfg.camera_height, 3)?;
        let mut ascii_frame = AsciiFrame::new(cfg.ascii_width, cfg.ascii_height, ' ')?;

        let converter = AsciiConverter::new(
            AsciiConverter::DEFAULT_ASCII_INTENSITY.chars().collect(),
            AsciiConverter::DEFAULT_ASCII_HORIZONTAL.chars().collect(),
            AsciiConverter::DEFAULT_ASCII_VERTICAL.chars().collect(),
            AsciiConverter::DEFAULT_ASCII_FORWARD.chars().collect(),
            AsciiConverter::DEFAULT_ASCII_BACK.chars().collect(),
            cfg.camera_width,
            cfg.camera_height,
            cfg.edge_threshold,
            cfg.contrast,
            cfg.brightness,
        )?;

        loop {
            camera.capture_frame(&mut image_frame)?;
            converter.convert(&image_frame, &mut ascii_frame)?;

            let chars = ascii_frame.chars();
            let width = ascii_frame.w;
            let frame_string = chars
                .chunks(width)
                .map(|line| line.iter().collect::<String>())
                .collect::<Vec<_>>()
                .join("\n");

            // Use UTF-8 safe encoding from AsciiFrame
            let frame_bytes = frame_string.as_bytes();
            let frame_bytes_len = frame_bytes.len();
            if frame_bytes_len > u16::MAX as usize {
                return Err("Frame too large to send via double-byte length field".into());
            }

            let mut udp_message = vec![UDP_MESSAGE_BYTE];
            udp_message.write_u16(frame_bytes.iter().len() as u16).await?;
            udp_message.extend_from_slice(frame_bytes);
            
            udp_socket_clone
                .send_to(&udp_message, args.udp_addr.clone())
                .await?;
        }
    }
}

async fn handle_connection_response(
    stream: &mut TcpStream,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut buf = vec![0; 1024];
    let n = stream.read_buf(&mut buf).await?;
    if n == 0 {
        return Err("Connection closed by server".into());
    }

    let response_byte = buf[0];
    if response_byte == INVALID_RESPONSE_BYTE {
        return Err("Invalid connection request".into());
    }

    Ok(())
}

fn prompt_for_username_to_connect(other_usernames: Vec<String>) -> String {
    println!("Current available users:");
    for username in &other_usernames {
        println!(" - {}", username);
    }

    loop {
        print!("Enter a username to connect to: ");
        io::stdout().flush().unwrap(); // Make sure prompt shows before input

        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_err() {
            println!("Error reading input. Please try again.");
            continue;
        }

        let input = input.trim();

        if other_usernames.iter().any(|u| u == input) {
            return input.to_string();
        }

        println!("Invalid selection. Please try again.");
    }
}

fn prompt_for_username() -> Result<String, io::Error> {
    print!("Please enter your username: ");
    stdout().flush()?;

    let mut username = String::new();
    io::stdin().read_line(&mut username)?;
    let username = username.trim();

    if username.is_empty() || username.len() > 256 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Username must be between 1 and 256 characters.",
        ));
    }

    let invalid_chars = ['\n', '\r', '\0', '\t', ' '];
    if username.chars().any(|c| invalid_chars.contains(&c)) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Username contains invalid characters.",
        ));
    }

    Ok(username.to_string())
}

async fn send_username(stream: &mut TcpStream, username: &str) -> io::Result<()> {
    let username_bytes = username.as_bytes();
    let username_length = username_bytes.len() as u8;

    let mut buffer = Vec::with_capacity(2 + username_bytes.len());
    buffer.push(HELLO_BYTE);
    buffer.push(username_length);
    buffer.extend_from_slice(username_bytes);

    stream.write_all(&buffer).await?;

    Ok(())
}

async fn receive_user_list(stream: &mut TcpStream) -> io::Result<Vec<String>> {
    let mut buf = Vec::with_capacity(2048);

    loop {
        let n = stream.read_buf(&mut buf).await?;
        if n == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Server closed the connection",
            ));
        }

        let response_byte = buf.get(0).copied().unwrap_or(0);
        let usernames_length = buf.get(1).copied().unwrap_or(0);

        if response_byte == INVALID_RESPONSE_BYTE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Server rejected the username.",
            ));
        }

        if response_byte != HELLO_BYTE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Unexpected response byte: {}", response_byte),
            ));
        }

        if usernames_length == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Received invalid usernames length (0).",
            ));
        }

        let mut offset = 2;
        let mut usernames = Vec::new();

        while offset < buf.len() {
            if offset >= buf.len() {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "Unexpected end of buffer.",
                ));
            }

            let name_len = buf[offset] as usize;
            offset += 1;

            if offset + name_len > buf.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Invalid username length: {}", name_len),
                ));
            }

            let name_bytes = &buf[offset..offset + name_len];
            usernames.push(String::from_utf8_lossy(name_bytes).to_string());
            offset += name_len;
        }

        return Ok(usernames);
    }
}
