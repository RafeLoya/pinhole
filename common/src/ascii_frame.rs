use std::error::Error;

const DEFAULT_W: usize = 120;
const DEFAULT_H: usize = 40;

/// ASCII representation of an `ImageFrame` after contrast, brightness,
/// and luminance transformations
#[derive(Clone)]
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

    pub fn from_bytes(w: usize, h: usize, bytes: &[u8]) -> Result<Self, Box<dyn Error>> {
        if w == 0 || h == 0 {
            return Err("dimensions must be greater than zero".into());
        }

        if bytes.len() < w * h {
            return Err(format!(
                "not enough data: expected {} chars but got {}",
                w * h,
                bytes.len()
            )
            .into());
        }

        let mut frame = Self {
            w,
            h,
            chars: vec![' '; w * h],
        };

        // TODO: is this faster? iterating vs. iter than memcpy?
        // for i in 0..w * h {
        //     frame.chars[i] = bytes[i] as char;
        // }
        let ascii: Vec<char> = bytes.iter().map(|&b| b as char).collect();
        frame.chars.copy_from_slice(&ascii);

        Ok(frame)
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

    pub fn set_chars(&mut self, data: &[char]) -> bool {
        if data.len() > self.chars.len() {
            return false;
        }

        self.chars[0..data.len()].copy_from_slice(data);
        true
    }

    pub fn set_chars_from_bytes(&mut self, bytes: &[u8]) -> bool {
        if bytes.len() > self.chars.len() {
            return false;
        }

        let ascii: Vec<char> = bytes.iter().map(|&b| b as char).collect();
        self.chars[0..bytes.len()].copy_from_slice(&ascii);

        true
    }

    pub fn set_chars_from_vec(&mut self, data: Vec<char>) -> bool {
        if data.len() > self.chars.len() {
            return false;
        }

        self.chars[0..data.len()].copy_from_slice(&data);
        true
    }

    pub fn chars(&self) -> &[char] {
        &self.chars
    }

    pub fn chars_mut(&mut self) -> &mut [char] {
        &mut self.chars
    }

    pub fn bytes(&self) -> Vec<u8> {
        let mut bytes: Vec<u8> = Vec::with_capacity(self.chars.len());

        for &c in &self.chars {
            bytes.push(c as u8);
        }

        bytes
    }
}
