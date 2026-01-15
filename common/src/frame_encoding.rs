use crate::frame_pixel::FramePixel;

/// Type of frame encoding
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FrameType {
    /// Full frame - all pixels included
    Full = 0x00,
    /// Diff frame - only changed pixels
    Diff = 0x01,
}

impl FrameType {
    pub fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            0x00 => Some(FrameType::Full),
            0x01 => Some(FrameType::Diff),
            _ => None,
        }
    }

    pub fn as_byte(self) -> u8 {
        self as u8
    }
}

/// A single pixel change in a diff frame
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct PixelChange {
    /// Index of the pixel (0 to w*h-1)
    pub index: u16,
    /// New pixel value
    pub pixel: FramePixel,
}

impl PixelChange {
    pub fn new(index: u16, pixel: FramePixel) -> Self {
        Self { index, pixel }
    }

    /// Encode this change as 3 bytes: [index_hi, index_lo, pixel]
    pub fn to_bytes(&self) -> [u8; 3] {
        let idx_bytes = self.index.to_be_bytes();
        [idx_bytes[0], idx_bytes[1], self.pixel.as_byte()]
    }

    /// Decode from 3 bytes
    pub fn from_bytes(bytes: &[u8; 3]) -> Self {
        let index = u16::from_be_bytes([bytes[0], bytes[1]]);
        let pixel = FramePixel::from_byte(bytes[2]);
        Self { index, pixel }
    }
}
