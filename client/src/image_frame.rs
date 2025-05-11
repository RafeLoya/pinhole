use crate::ascii_converter::{B_LUMINANCE, G_LUMINANCE, R_LUMINANCE};
use std::error::Error;

/// Initial frame received from webcam feed
pub struct ImageFrame {
    /// width of image
    pub w: usize,
    /// height of image
    pub h: usize,
    /// usually 3 (RGB)
    pub bytes_per_pixel: usize,
    /// frame data
    pub buffer: Vec<u8>,
}

/// Video frames given to this program from the FFmpeg child process
impl ImageFrame {
    pub fn new(w: usize, h: usize, bytes_per_pixel: usize) -> Result<Self, Box<dyn Error>> {
        if w == 0 || h == 0 || bytes_per_pixel == 0 {
            return Err("width, height, and bytes per pixel must be greater than zero".into());
        }

        Ok(Self {
            w,
            h,
            bytes_per_pixel,
            buffer: vec![0; w * h * bytes_per_pixel],
        })
    }

    /// Return raw image data
    pub fn buffer(&self) -> &[u8] {
        &self.buffer
    }

    /// Return raw, mutable image data
    pub fn buffer_mut(&mut self) -> &mut [u8] {
        &mut self.buffer
    }

    /// Get pixel RGB values, with bounds checking
    pub fn get_pixel(&self, x: usize, y: usize) -> Option<(u8, u8, u8)> {
        if x >= self.w || y >= self.h {
            return None;
        }

        let i = (y * self.w + x) * self.bytes_per_pixel;
        if i + 2 >= self.buffer.len() {
            return None;
        }

        Some((self.buffer[i], self.buffer[i + 1], self.buffer[i + 2]))
    }

    /// Calculate the grayscale intensity value (relative luminance)
    /// of a given pixel
    pub fn calculate_intensity((r, g, b): (u8, u8, u8)) -> f32 {
        R_LUMINANCE * r as f32 + G_LUMINANCE * g as f32 + B_LUMINANCE * b as f32
    }

    /// Calculate the grayscale intensity value (relative luminance)
    /// of a given pixel and cast as a `u8`
    pub fn calculate_intensity_u8((r, g, b): (u8, u8, u8)) -> u8 {
        ImageFrame::calculate_intensity((r, g, b)) as u8
    }
}
