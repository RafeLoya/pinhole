use crate::frame_pixel::FramePixel;
use std::error::Error;

/// ASCII representation of an `ImageFrame` after contrast, brightness,
/// and luminance transformations.
///
/// Stores character-agnostic pixel data that can be mapped to different
/// character sets on each client.
#[derive(Clone)]
pub struct AsciiFrame {
    /// The amount of columns in the frame
    pub w: usize,
    /// The amount of rows in the frame
    pub h: usize,
    /// processed image pixels, stored as character-agnostic indices
    pixels: Vec<FramePixel>,
}

impl AsciiFrame {
    /// Create a new frame with the given dimensions
    ///
    /// The deprecated `default_char` parameter is ignored and kept for
    /// backwards compatibility. All pixels are initialized to default (intensity 0).
    pub fn new(w: usize, h: usize, _default_char: char) -> Result<Self, Box<dyn Error>> {
        if w == 0 || h == 0 {
            return Err("dimensions must be greater than zero".into());
        }

        Ok(Self {
            w,
            h,
            pixels: vec![FramePixel::default(); w * h],
        })
    }

    /// Extract an `AsciiFrame` from an array of bytes (raw FramePixel data)
    pub fn from_bytes(w: usize, h: usize, bytes: &[u8]) -> Result<Self, Box<dyn Error>> {
        if w == 0 || h == 0 {
            return Err("dimensions must be greater than zero".into());
        }

        let total = w * h;

        if bytes.len() != total {
            return Err(format!(
                "expected {} bytes for {}x{} frame, got {}",
                total, w, h, bytes.len()
            ).into());
        }

        let pixels = bytes.iter().map(|&b| FramePixel::from_byte(b)).collect();

        Ok(Self { w, h, pixels })
    }

    /// Set individual pixel, with bounds check
    pub fn set_pixel(&mut self, x: usize, y: usize, pixel: FramePixel) -> bool {
        if x >= self.w || y >= self.h {
            return false;
        }

        let i = y * self.w + x;
        if i < self.pixels.len() {
            self.pixels[i] = pixel;
            true
        } else {
            false
        }
    }

    /// Legacy method: set individual characters (deprecated, kept for compatibility)
    ///
    /// This is a no-op in the new implementation. Use set_pixel instead.
    #[deprecated(note = "Use set_pixel instead")]
    pub fn set_char(&mut self, x: usize, y: usize, _c: char) -> bool {
        // for now, just set to default pixel
        self.set_pixel(x, y, FramePixel::default())
    }

    /// Set range of pixels, with bounds check
    pub fn set_pixels(&mut self, data: &[FramePixel]) -> bool {
        if data.len() > self.pixels.len() {
            return false;
        }

        self.pixels[0..data.len()].copy_from_slice(data);
        true
    }

    /// Legacy method: set range of characters (deprecated, kept for compatibility)
    #[deprecated(note = "Use set_pixels instead")]
    pub fn set_chars(&mut self, _data: &[char]) -> bool {
        // no-op in new implementation
        false
    }

    /// Return raw pixel data
    pub fn pixels(&self) -> &[FramePixel] {
        &self.pixels
    }

    /// Return raw, mutable pixel data
    pub fn pixels_mut(&mut self) -> &mut [FramePixel] {
        &mut self.pixels
    }

    /// Legacy method: return char data (deprecated, kept for compatibility)
    ///
    /// Returns empty slice in new implementation
    #[deprecated(note = "Use pixels instead")]
    pub fn chars(&self) -> &[char] {
        &[]
    }

    /// Legacy method: return mutable char data (deprecated, kept for compatibility)
    #[deprecated(note = "Use pixels_mut instead")]
    pub fn chars_mut(&mut self) -> &mut [char] {
        &mut []
    }

    /// Encode frame as raw bytes (one byte per pixel)
    pub fn bytes(&self) -> Vec<u8> {
        self.pixels.iter().map(|p| p.as_byte()).collect()
    }
}
