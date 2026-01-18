/// Maximum number of intensity characters allowed
pub const MAX_INTENSITY_CHARS: usize = 16;
/// Maximum number of edge characters allowed per edge type
pub const MAX_EDGE_CHARS: usize = 8;

/// Character-agnostic representation of a single pixel in an ASCII frame.
///
/// Stores the category (intensity or edge type) and an index into that category's
/// character set. This allows clients to use different character mappings while
/// transmitting the same semantic data.
///
/// Encoded as a single byte with bit layout: CCC_IIIII
/// - CCC (3 bits): Category (0-4)
/// - IIIII (5 bits): Index within category (0-31)
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct FramePixel(u8);

// category bits (top 3 bits)
const CATEGORY_INTENSITY: u8 = 0;
const CATEGORY_HORIZONTAL: u8 = 1;
const CATEGORY_VERTICAL: u8 = 2;
const CATEGORY_FORWARD: u8 = 3;
const CATEGORY_BACK: u8 = 4;

/// bit mask for extracting index (bottom 5 bits)
const INDEX_MASK: u8 = 0b0001_1111;

impl FramePixel {
    /// Create an intensity pixel with the given index
    ///
    /// # Panics
    /// Panics in debug mode if index >= MAX_INTENSITY_CHARS
    pub fn intensity(idx: u8) -> Self {
        debug_assert!(
            (idx as usize) < MAX_INTENSITY_CHARS,
            "intensity index {} exceeds max {}",
            idx,
            MAX_INTENSITY_CHARS
        );
        Self((CATEGORY_INTENSITY << 5) | idx)
    }

    /// Create a horizontal edge pixel with the given index
    ///
    /// # Panics
    /// Panics in debug mode if index >= MAX_EDGE_CHARS
    pub fn horizontal_edge(idx: u8) -> Self {
        debug_assert!(
            (idx as usize) < MAX_EDGE_CHARS,
            "horizontal edge index {} exceeds max {}",
            idx,
            MAX_EDGE_CHARS
        );
        Self((CATEGORY_HORIZONTAL << 5) | idx)
    }

    /// Create a vertical edge pixel with the given index
    ///
    /// # Panics
    /// Panics in debug mode if index >= MAX_EDGE_CHARS
    pub fn vertical_edge(idx: u8) -> Self {
        debug_assert!(
            (idx as usize) < MAX_EDGE_CHARS,
            "vertical edge index {} exceeds max {}",
            idx,
            MAX_EDGE_CHARS
        );
        Self((CATEGORY_VERTICAL << 5) | idx)
    }

    /// Create a forward diagonal edge pixel with the given index
    ///
    /// # Panics
    /// Panics in debug mode if index >= MAX_EDGE_CHARS
    pub fn forward_diagonal(idx: u8) -> Self {
        debug_assert!(
            (idx as usize) < MAX_EDGE_CHARS,
            "forward diagonal index {} exceeds max {}",
            idx,
            MAX_EDGE_CHARS
        );
        Self((CATEGORY_FORWARD << 5) | idx)
    }

    /// Create a back diagonal edge pixel with the given index
    ///
    /// # Panics
    /// Panics in debug mode if index >= MAX_EDGE_CHARS
    pub fn back_diagonal(idx: u8) -> Self {
        debug_assert!(
            (idx as usize) < MAX_EDGE_CHARS,
            "back diagonal index {} exceeds max {}",
            idx,
            MAX_EDGE_CHARS
        );
        Self((CATEGORY_BACK << 5) | idx)
    }

    /// Get the category of this pixel (0-4)
    pub fn category(&self) -> u8 {
        self.0 >> 5
    }

    /// Get the index within the category (0-31)
    pub fn index(&self) -> u8 {
        self.0 & INDEX_MASK
    }

    /// Get the raw byte value
    pub fn as_byte(&self) -> u8 {
        self.0
    }

    /// Create from a raw byte value
    pub fn from_byte(byte: u8) -> Self {
        Self(byte)
    }

    /// Check if this is an intensity pixel
    pub fn is_intensity(&self) -> bool {
        self.category() == CATEGORY_INTENSITY
    }

    /// Check if this is a horizontal edge pixel
    pub fn is_horizontal_edge(&self) -> bool {
        self.category() == CATEGORY_HORIZONTAL
    }

    /// Check if this is a vertical edge pixel
    pub fn is_vertical_edge(&self) -> bool {
        self.category() == CATEGORY_VERTICAL
    }

    /// Check if this is a forward diagonal edge pixel
    pub fn is_forward_diagonal(&self) -> bool {
        self.category() == CATEGORY_FORWARD
    }

    /// Check if this is a back diagonal edge pixel
    pub fn is_back_diagonal(&self) -> bool {
        self.category() == CATEGORY_BACK
    }
}

impl Default for FramePixel {
    fn default() -> Self {
        // default to intensity 0 (typically space character)
        Self::intensity(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_intensity_pixel() {
        let pixel = FramePixel::intensity(5);
        assert_eq!(pixel.category(), CATEGORY_INTENSITY);
        assert_eq!(pixel.index(), 5);
        assert!(pixel.is_intensity());
    }

    #[test]
    fn test_horizontal_edge() {
        let pixel = FramePixel::horizontal_edge(2);
        assert_eq!(pixel.category(), CATEGORY_HORIZONTAL);
        assert_eq!(pixel.index(), 2);
        assert!(pixel.is_horizontal_edge());
    }

    #[test]
    fn test_vertical_edge() {
        let pixel = FramePixel::vertical_edge(1);
        assert_eq!(pixel.category(), CATEGORY_VERTICAL);
        assert_eq!(pixel.index(), 1);
        assert!(pixel.is_vertical_edge());
    }

    #[test]
    fn test_forward_diagonal() {
        let pixel = FramePixel::forward_diagonal(0);
        assert_eq!(pixel.category(), CATEGORY_FORWARD);
        assert_eq!(pixel.index(), 0);
        assert!(pixel.is_forward_diagonal());
    }

    #[test]
    fn test_back_diagonal() {
        let pixel = FramePixel::back_diagonal(2);
        assert_eq!(pixel.category(), CATEGORY_BACK);
        assert_eq!(pixel.index(), 2);
        assert!(pixel.is_back_diagonal());
    }

    #[test]
    fn test_byte_roundtrip() {
        let original = FramePixel::intensity(9);
        let byte = original.as_byte();
        let restored = FramePixel::from_byte(byte);
        assert_eq!(original, restored);
    }

    #[test]
    fn test_default() {
        let pixel = FramePixel::default();
        assert!(pixel.is_intensity());
        assert_eq!(pixel.index(), 0);
    }
}
