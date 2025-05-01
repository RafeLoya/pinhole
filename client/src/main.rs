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
use rand::{rng, Rng};
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
    // let mut config = VideoConfig::new(
    //     640,
    //     480,
    //     120,
    //     40,
    //     127.50,
    //     1.5,
    //     0.0
    // );
    //
    // let mut camera = Camera::new(config.camera_width, config.camera_height)?;
    //
    // let mut image_frame = ImageFrame::new(config.camera_width, config.camera_height, 3)?;
    // let mut ascii_frame = AsciiFrame::new(config.ascii_width, config.ascii_height, ' ')?;
    //
    // let converter = AsciiConverter::new(
    //     AsciiConverter::DEFAULT_ASCII_INTENSITY.chars().collect(),
    //     AsciiConverter::DEFAULT_ASCII_HORIZONTAL.chars().collect(),
    //     AsciiConverter::DEFAULT_ASCII_VERTICAL.chars().collect(),
    //     AsciiConverter::DEFAULT_ASCII_FORWARD.chars().collect(),
    //     AsciiConverter::DEFAULT_ASCII_BACK.chars().collect(),
    //     config.camera_width,
    //     config.camera_height,
    //     config.edge_threshold,
    //     config.contrast,
    //     config.brightness
    // )?;
    //
    // let mut renderer = AsciiRenderer::new()?;
    //
    // loop {
    //     if let Err(e) = camera.capture_frame(&mut image_frame) {
    //         eprintln!("failed while capturing frame: {}", e);
    //         break;
    //     }
    //
    //     if let Err(e) = converter.convert(&image_frame, &mut ascii_frame) {
    //         eprintln!("failed while converting frame: {}", e);
    //         break;
    //     }
    //
    //     if let Err(e) = renderer.render(&ascii_frame) {
    //         eprintln!("failed while rendering frame: {}", e);
    //         break;
    //     }
    //
    //     thread::sleep(Duration::from_millis(10));
    // }

    // tracing_subscriber::fmt::init();
    // 
    // let mut client = Client::new()?;
    // 
    // let server_addr = "[::1]:4433".parse::<SocketAddr>()?;
    // client.connect(server_addr, "localhost").await?;
    // 
    // let response = client.send_message(b"hello from client!").await?;
    // println!("response: {:?}", std::str::from_utf8(&response)?);
    // 
    // client.close();
    // client.wait_idle().await;
    // 
    // Ok(())
    
    // TODO: mock client program?
    
    let args = Args::parse();
    tracing_subscriber::fmt::init();
    
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
