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
use crate::ascii_renderer::AsciiRenderer;

// Fixed frame dimensions
const ASCII_WIDTH: u16 = 120;
const ASCII_HEIGHT: u16 = 40;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = VideoConfig::new(
        640,
        480,
        ASCII_WIDTH as i32 as usize,
        ASCII_HEIGHT as i32 as usize,
        127.50,
        1.5,
        0.0
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

    let socket = UdpSocket::bind("0.0.0.0:0")?;
    println!("Local socket bound to: {}", socket.local_addr()?);

    let server_addr = "35.226.114.166:4433";

    socket.send_to(b"init_ascii_stream", server_addr)?;
    println!("Sent initialization packet to {}", server_addr);

    let frame_data_size = (ASCII_WIDTH * ASCII_HEIGHT) as usize;
    let packet_size = 4 + frame_data_size;

    let socket_sender = socket.try_clone()?;
    let socket_receiver = socket; // original for receiving

    // Spawn sender task
    let sender_task = tokio::spawn(async move {
        let mut frame_number = 0;
        loop {
            if let Err(e) = camera.capture_frame(&mut image_frame) {
                eprintln!("Capture error: {}", e);
                break;
            }

            if let Err(e) = converter.convert(&image_frame, &mut ascii_frame) {
                eprintln!("Convert error: {}", e);
                break;
            }

            let mut packet = Vec::with_capacity(packet_size);
            packet.extend_from_slice(&frame_number.to_be_bytes());

            let ascii_data = ascii_frame.chars().into_vec().collect::<Vec<u8>>();

            if ascii_data.len() != frame_data_size {
                eprintln!("Warning: ASCII data size mismatch. Expected: {}, Got: {}", frame_data_size, ascii_data.len());
            }

            packet.extend_from_slice(&ascii_data);

            if let Err(e) = socket_sender.send_to(&packet, server_addr) {
                eprintln!("Failed to send frame {}: {}", frame_number, e);
            } else if frame_number % 30 == 0 {
                println!("Sent frame {}", frame_number);
            }

            frame_number += 1;

            // 100 FPS
            thread::sleep(Duration::from_millis(10));
        }
    });

    // Spawn receiver task
    let receiver_task = tokio::spawn(async move {
        let mut renderer = AsciiRenderer::new().unwrap();
        let mut buf = vec![0u8; 4096];

        loop {
            match socket_receiver.recv_from(&mut buf) {
                Ok((size, _src)) => {
                    if size < 4 {
                        continue; // invalid packet
                    }

                    let frame_number = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]);
                    let ascii_data = &buf[4..size];

                    if ascii_data.len() != (ASCII_WIDTH as usize * ASCII_HEIGHT as usize) {
                        eprintln!("Received corrupted frame {}", frame_number);
                        continue;
                    }

                    let ascii_chars = ascii_data.iter().map(|&b| b as char).collect::<Vec<char>>();
                    let mut incoming_frame = AsciiFrame::new(ASCII_WIDTH as usize, ASCII_HEIGHT as usize, ' ').unwrap();
                    incoming_frame.chars_mut().copy_from_slice(&ascii_chars);


                    if let Err(e) = renderer.render(&incoming_frame) {
                        eprintln!("Render error: {}", e);
                    }
                }
                Err(e) => {
                    eprintln!("Receive error: {}", e);
                    break;
                }
            }
        }
    });

    // Wait for both tasks to finish
    let _ = tokio::try_join!(sender_task, receiver_task);

    Ok(())
}
