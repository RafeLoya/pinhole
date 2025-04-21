use common::logger;

use clap::Parser;
use std::io::{self, Write};
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use nokhwa::{
    Camera,
    pixel_format::RgbFormat,
    utils::{CameraFormat, CameraIndex, FrameFormat, RequestedFormat, RequestedFormatType},
};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value_t = 30)]
    fps: u32,

    #[arg(short, long)]
    logfile: Option<PathBuf>,
}

fn get_log_path(cli_arg: &Option<PathBuf>) -> PathBuf {
    cli_arg
        .clone()
        .unwrap_or_else(|| PathBuf::from("client.log"))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse command line arguments
    let args = Args::parse();
    let log_path = get_log_path(&args.logfile);

    // Create custom logger with 'client.log'
    let logger = logger::Logger::with_file_name("client.log")?;

    // Clamp FPS value between 30 and 256
    let mut fps = args.fps;
    if fps < 30 {
        logger.warn(format!("FPS value ({}) is too low. Clamping to 30.", fps).as_str())?;
        fps = 30;
    } else if fps > 256 {
        logger.warn(format!("FPS value ({}) is too high. Clamping to 256.", fps).as_str())?;
        fps = 256;
    }

    // Log initial configuration
    logger.info(
        format!(
            "Initial Configuration; FPS({}), LogFile({:?})",
            fps, log_path
        )
        .as_str(),
    )?;

    // Increased terminal dimensions for higher resolution
    let width = 120;
    let height = 40;

    // Extended ASCII character set for more detail
    let ascii_chars = " .,:-=o+*#%&@8$".chars().collect::<Vec<char>>();

    // Clear screen sequence
    let clear_screen = "\x1B[2J\x1B[1;1H";

    println!("Starting webcam to ASCII stream. Press Ctrl+C to exit.");
    logger.info("Starting webcam to ASCII stream")?;

    // Select the first camera
    let index = CameraIndex::Index(0);

    // Request a standard format (640x480, 30 FPS, MJPEG) for cross-platform compatibility
    let requested = RequestedFormat::new::<RgbFormat>(RequestedFormatType::Exact(
        CameraFormat::new_from(640, 480, FrameFormat::MJPEG, fps),
    ));

    // Initialize camera with platform-specific backend
    let mut camera = Camera::new(index, requested)?;
    logger.info("Camera initialized successfully")?;

    // Open the camera stream
    camera.open_stream()?;
    logger.info("Camera stream opened successfully")?;

    // Contrast and brightness adjustments
    let contrast = 1.5;
    let brightness = -0.03;

    let frame_format = camera.frame_format();
    logger.info(format!("Using frame format: {:?}", frame_format).as_str())?;

    // Frame counter
    let mut frame_count: u64 = 0;

    loop {
        frame_count += 1;

        // Capture a frame
        let frame = camera.frame()?;
        logger.debug(format!("Captured frame #{}", frame_count).as_str())?;

        let buffer = frame.buffer();
        let resolution = frame.resolution();
        let frame_width = resolution.width() as usize;
        let frame_height = resolution.height() as usize;

        let scale_x = frame_width as f32 / width as f32;
        let scale_y = frame_height as f32 / height as f32; // Adjust for terminal character aspect ratio

        print!("{}", clear_screen);

        match frame_format {
            // Handle RGB format
            FrameFormat::RAWRGB => {
                logger.debug(format!("Processing RGB frame #{}", frame_count).as_str())?;
                for y in 0..height {
                    for x in 0..width {
                        let img_x = (x as f32 * scale_x) as usize;
                        let img_y = (y as f32 * scale_y) as usize;

                        if img_x < frame_width && img_y < frame_height {
                            let pixel_index = (img_y * frame_width + img_x) * 3;
                            if pixel_index + 2 >= buffer.len() {
                                print!(" ");
                                continue;
                            }

                            let r = buffer[pixel_index];
                            let g = buffer[pixel_index + 1];
                            let b = buffer[pixel_index + 2];

                            let apply_contrast_brightness = |value: u8| -> u8 {
                                let mut v = value as f32 / 255.0;
                                // Apply contrast
                                v = (v - 0.5) * contrast + 0.5;
                                // Apply brightness
                                v += brightness;
                                // Clamp to valid range
                                v = v.clamp(0.0, 1.0);
                                (v * 255.0) as u8
                            };

                            let r_adj = apply_contrast_brightness(r);
                            let g_adj = apply_contrast_brightness(g);
                            let b_adj = apply_contrast_brightness(b);

                            // Improved intensity calculation with adjusted weights
                            let intensity = (0.2989 * r_adj as f32
                                + 0.5870 * g_adj as f32
                                + 0.1140 * b_adj as f32)
                                as u8;
                            let char_idx = (intensity as f32 / 255.0
                                * (ascii_chars.len() - 1) as f32)
                                as usize;
                            let ascii_char = ascii_chars[char_idx];

                            print!("{}", ascii_char);
                        } else {
                            print!(" ");
                        }
                    }
                    println!();
                }
            }
            // Handle YUYV format
            FrameFormat::NV12 => {
                logger.debug(format!("Processing NV12 frame #{}", frame_count).as_str())?;
                for y in 0..height {
                    for x in 0..width {
                        let img_x = (x as f32 * scale_x) as usize;
                        let img_y = (y as f32 * scale_y) as usize;

                        if img_x < frame_width && img_y < frame_height {
                            let pixel_index = (img_y * frame_width + img_x) * 2;
                            if pixel_index + 1 >= buffer.len() {
                                print!(" ");
                                continue;
                            }

                            // YUYV format stores Y, U, Y, V pairs
                            let y1 = buffer[pixel_index];
                            let u = buffer[pixel_index + 1];
                            let v = buffer[pixel_index + 3];

                            // Convert YUYV to RGB
                            let y = y1 as f32;
                            let u = u as f32 - 128.0;
                            let v = v as f32 - 128.0;

                            let r = (y + 1.402 * v).clamp(0.0, 255.0);
                            let g = (y - 0.344136 * u - 0.714136 * v).clamp(0.0, 255.0);
                            let b = (y + 1.772 * u).clamp(0.0, 255.0);

                            let apply_contrast_brightness = |value: f32| -> f32 {
                                // Apply contrast
                                let mut v = value / 255.0;
                                v = (v - 0.5) * contrast + 0.5;
                                // Apply brightness
                                v += brightness;
                                v = v.clamp(0.0, 1.0);
                                v * 255.0
                            };

                            let r_adj = apply_contrast_brightness(r);
                            let g_adj = apply_contrast_brightness(g);
                            let b_adj = apply_contrast_brightness(b);

                            // Improved intensity calculation with adjusted weights
                            let intensity =
                                (0.2989 * r_adj + 0.5870 * g_adj + 0.1140 * b_adj) as u8;
                            let char_idx = (intensity as f32 / 255.0
                                * (ascii_chars.len() - 1) as f32)
                                as usize;
                            let ascii_char = ascii_chars[char_idx];

                            print!("{}", ascii_char);
                        } else {
                            print!(" ");
                        }
                    }
                    println!();
                }
            }
            x => {
                logger.error(format!("Unsupported pixel format: {}", x).as_str())?;
                println!("Unsupported pixel format. {}", x);
            }
        }

        io::stdout().flush()?;
        logger.debug(format!("Frame #{} rendered to terminal", frame_count).as_str())?;
        thread::sleep(Duration::from_millis(u64::from(1 / fps)));
    }
}
