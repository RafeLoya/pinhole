use nokhwa::Camera;
use nokhwa::pixel_format::RgbFormat;
use nokhwa::utils::{CameraIndex, RequestedFormat, RequestedFormatType};
use std::io::{self, Write};
use std::thread;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Terminal dimensions
    let width = 120;
    let height = 40;

    // ASCII characters from darkest to lightest
    let ascii_chars = " .,-~:;>=+uoxe*%$&#@AGNM".chars().collect::<Vec<char>>();

    // Clear screen sequence
    let clear_screen = "\x1B[2J\x1B[1;1H";

    println!("Starting webcam to ASCII stream. Press Ctrl+C to exit.");

    // Select the first camera
    let index = CameraIndex::Index(0);
    // Request the highest resolution in RGB format
    let requested = RequestedFormat::new::<RgbFormat>(RequestedFormatType::AbsoluteHighestFrameRate);
    let mut camera = Camera::new(index, requested)?;

    // Open the camera stream
    camera.open_stream()?;

    loop {
        // Capture a frame
        let frame = camera.frame()?;

        // Decode into RGB image (ImageBuffer)
        let decoded = frame.decode_image::<RgbFormat>()?;

        // Get dimensions
        let frame_width = decoded.width();
        let frame_height = decoded.height();

        // Calculate scaling factors
        let scale_x = frame_width as f32 / width as f32;
        let scale_y = frame_height as f32 / height as f32;

        // Clear the screen
        print!("{}", clear_screen);

        for y in 0..height {
            for x in 0..width {
                let img_x = (x as f32 * scale_x) as u32;
                let img_y = (y as f32 * scale_y) as u32;

                if img_x < frame_width && img_y < frame_height {
                    let pixel = decoded.get_pixel(img_x, img_y);
                    let [r, g, b] = pixel.0;

                    let intensity = (0.2989 * r as f32 + 0.5870 * g as f32 + 0.1140 * b as f32) as u8;
                    let char_idx = (intensity as f32 / 255.0 * (ascii_chars.len() - 1) as f32) as usize;
                    let ascii_char = ascii_chars[char_idx];

                    print!("{}", ascii_char);
                } else {
                    print!(" ");
                }
            }
            println!();
        }

        io::stdout().flush()?;
        thread::sleep(Duration::from_millis(30)); // ~30 FPS
    }
}