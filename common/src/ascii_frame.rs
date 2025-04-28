use std::error::Error;

const DEFAULT_W: usize = 120;
const DEFAULT_H: usize = 40;

/// ASCII representation of an `ImageFrame` after contrast, brightness,
/// and luminance transformations
pub struct AsciiFrame {
    /// The amount of columns in the frame
    pub w: usize,
    /// The amount of rows in the frame
    pub h: usize,
    /// processed image pixels, interpreted as characters
    chars: Vec<char>,
}

impl AsciiFrame {
    pub fn new(w: usize, h: usize, default_char: char) -> Result<Self, Box<dyn Error>> {
        if w == 0 || h == 0 {
            return Err("dimensions must be greater than zero".into());
        }

        Ok(Self {
            w,
            h,
            chars: vec![default_char; w * h],
        })
    }

    pub fn set_char(&mut self, x: usize, y: usize, c: char) -> bool {
        if x >= self.w || y >= self.h {
            return false;
        }

        let i = y * self.w + x;
        if i < self.chars.len() {
            self.chars[i] = c;
            true
        } else {
            false
        }
    }

    pub fn chars(&self) -> &[char] {
        &self.chars
    }
}