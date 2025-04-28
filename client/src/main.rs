extern crate alloc;

mod ffmpeg;
mod camera;
mod ascii_renderer;
mod image_frame;
mod ascii_converter;
mod edge_detector;
mod video_config;
mod client;

use std::net::{SocketAddr, UdpSocket};
use crate::ascii_converter::AsciiConverter;
use common::ascii_frame::AsciiFrame;
use crate::camera::Camera;
use crate::image_frame::ImageFrame;

use crate::video_config::VideoConfig;
use std::time::Duration;
use std::thread;
use std::io;

// Fixed frame dimensions
const ASCII_WIDTH: u16 = 120;
const ASCII_HEIGHT: u16 = 40;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = VideoConfig::new(
        640,                // Camera width
        480,                // Camera height
        ASCII_WIDTH as i32 as usize, // ASCII width
        ASCII_HEIGHT as i32 as usize, // ASCII height
        127.50,             // Edge threshold
        1.5,                // Contrast
        0.0                 // Brightness
    );

    let mut camera = Camera::new(config.camera_width, config.camera_height)?;

    let mut image_frame = ImageFrame::new(config.camera_width, config.camera_height, 3)?;
    let mut ascii_frame = AsciiFrame::new(ASCII_WIDTH as i32 as usize, ASCII_HEIGHT as i32 as usize, ' ')?;

    let converter = AsciiConverter::new(
        AsciiConverter::DEFAULT_ASCII_INTENSITY.chars().collect(),
        AsciiConverter::DEFAULT_ASCII_HORIZONTAL.chars().collect(),
        AsciiConverter::DEFAULT_ASCII_VERTICAL.chars().collect(),
        AsciiConverter::DEFAULT_ASCII_FORWARD.chars().collect(),
        AsciiConverter::DEFAULT_ASCII_BACK.chars().collect(),
        config.camera_width,
        config.camera_height,
        config.edge_threshold,
        config.contrast,
        config.brightness
    )?;

    tracing_subscriber::fmt::init();

    // Set up UDP socket
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    println!("Local socket bound to: {}", socket.local_addr()?);

    // Define the server address (using the same IP from Makefile)
    let server_addr = "35.226.114.166:4433";

    // Send initial packet to establish communication
    let init_message = b"init_ascii_stream";
    socket.send_to(init_message, server_addr)?;
    println!("Sent initialization packet to {}", server_addr);

    // Calculate the size of each ASCII frame
    let frame_data_size = (ASCII_WIDTH * ASCII_HEIGHT) as usize;

    // Calculate total packet size (4 bytes frame number + data)
    let packet_size = 4 + frame_data_size;
    println!("Sending frames of size {}x{} ({} bytes per frame)",
             ASCII_WIDTH, ASCII_HEIGHT, frame_data_size);

    // Main processing loop
    let mut frame_number = 0;
    loop {
        if let Err(e) = camera.capture_frame(&mut image_frame) {
            eprintln!("Failed while capturing frame: {}", e);
            break;
        }

        if let Err(e) = converter.convert(&image_frame, &mut ascii_frame) {
            eprintln!("Failed while converting frame: {}", e);
            break;
        }

        // Create packet with frame number header and ASCII data
        let mut packet = Vec::with_capacity(packet_size);

        // Add 4-byte frame number header
        packet.extend_from_slice(&frame_number.to_be_bytes());

        // Get the ASCII data
        // We don't need to include width/height since they're fixed and known to the server
        let ascii_data = ascii_frame.chars().into_vec().collect::<Vec<u8>>();

        // Verify the data size matches our expectations
        if ascii_data.len() != frame_data_size {
            eprintln!("Warning: ASCII data size mismatch. Expected: {}, Got: {}",
                      frame_data_size, ascii_data.len());
        }

        // Add ASCII frame data
        packet.extend_from_slice(&ascii_data);

        // Send the packet
        match socket.send_to(&packet, server_addr) {
            Ok(bytes_sent) => {
                if frame_number % 30 == 0 {
                    println!("Sent frame {} ({} bytes)", frame_number, bytes_sent);
                }
            },
            Err(e) => {
                eprintln!("Failed to send frame {}: {}", frame_number, e);
            }
        }

        frame_number += 1;

        // Control frame rate
        thread::sleep(Duration::from_millis(10));
    }

    // Send termination message
    let term_message = b"terminate_stream";
    let _ = socket.send_to(term_message, server_addr);
    println!("Sent termination packet");

    Ok(())
}