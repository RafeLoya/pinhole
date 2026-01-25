use crate::edge_detector::EdgeDetector;
use crate::image_frame::ImageFrame;
use common::text_frame::TextFrame;
use common::frame_pixel::FramePixel;
use std::error::Error;

// The coefficients below are derived from Rec. ITU-R BT.601-7.
// In the specification, these luminance coefficients represent
// how much they influence / contribute to the human eye's
// perception of brightness.

pub const R_LUMINANCE: f32 = 0.2989;
pub const G_LUMINANCE: f32 = 0.5870;
pub const B_LUMINANCE: f32 = 0.1140;

/// Intermediary translator to transform an `ImageFrame` into a `TextFrame`
pub struct AsciiConverter {
    /// Identifies edges in given `ImageFrame`s (None if edge detection disabled)
    edge_detector: Option<EdgeDetector>,
    /// Number of intensity levels (length of intensity character set)
    intensity_levels: usize,
    /// Number of edge characters per edge type
    edge_char_count: usize,
    /// Minimum gradient magnitude for edge detection
    edge_threshold: f32,
    /// Adjustment factor for contrast.
    /// Values < 1.0 reduce contrast, values > 1.0 increase contrast
    contrast: f32,
    /// Adjustment factor for brightness.
    /// values > 0 increase brightness, values < 0 brightness
    brightness: f32,
}

impl AsciiConverter {
    pub const DEFAULT_ASCII_INTENSITY: &'static str = " .:coPO?@■";
    pub const DEFAULT_ASCII_HORIZONTAL_LINES: &'static str = "-━═";
    pub const DEFAULT_ASCII_VERTICAL_LINES: &'static str = "|│┃";
    pub const DEFAULT_ASCII_FORWARD_DIAGONAL: &'static str = "/╱⟋";
    pub const DEFAULT_ASCII_BACK_DIAGONAL: &'static str = "\\╲⟍";
    pub const DEFAULT_CONTRAST: f32 = 1.5;
    pub const DEFAULT_BRIGHTNESS: f32 = 0.0;

    pub fn new(
        intensity_levels: usize,
        edge_char_count: usize,
        w: usize,
        h: usize,
        edge_detection_enabled: bool,
        edge_threshold: f32,
        contrast: f32,
        brightness: f32,
    ) -> Result<Self, Box<dyn Error>> {
        let edge_detector = if edge_detection_enabled {
            let detector = EdgeDetector::new(w, h, edge_threshold);
            detector.start(w, h)?;
            Some(detector)
        } else {
            None
        };

        Ok(Self {
            edge_detector,
            intensity_levels,
            edge_char_count,
            edge_threshold,
            contrast,
            brightness,
        })
    }

    pub fn default() -> Result<Self, Box<dyn Error>> {
        Self::new(
            Self::DEFAULT_ASCII_INTENSITY.chars().count(),
            Self::DEFAULT_ASCII_HORIZONTAL_LINES.chars().count(),
            640,
            480,
            true, // edge detection enabled by default
            EdgeDetector::DEFAULT_EDGE_THRESHOLD,
            Self::DEFAULT_CONTRAST,
            Self::DEFAULT_BRIGHTNESS,
        )
    }

    /// Convert an `ImageFrame` to an ASCII art representation with edges
    /// - Strong edges (based on `edge_threshold`) are represented with
    ///   separate characters to reflect the angle of an edge
    /// - All other regions are represented with intensity-based (grayscale)
    ///   ASCII characters
    ///
    /// The function also handles scaling from the original `ImageFrame`'s
    /// dimensions to the target `TextFrame`'s dimensions
    pub fn convert(
        &self,
        i_frame: &ImageFrame,
        a_frame: &mut TextFrame,
    ) -> Result<(), Box<dyn Error>> {
        // scaling factors to map the ASCII frame's dimension
        // to the original image's dimension
        let scale_x = i_frame.w as f32 / a_frame.w as f32;
        let scale_y = i_frame.h as f32 / a_frame.h as f32;

        if let Some(ref edge_detector) = self.edge_detector {
            // edge detection enabled
            edge_detector.submit_frame(i_frame)?;
            let edge_info = edge_detector.get_edge_info();

            for y in 0..a_frame.h {
                for x in 0..a_frame.w {
                    let i_x = (x as f32 * scale_x) as usize;
                    let i_y = (y as f32 * scale_y) as usize;
                    let e_i = i_y.min(edge_info.h - 1) * edge_info.w + i_x.min(edge_info.w - 1);

                    // if an edge's magnitude is greater than the threshold,
                    // assign edge pixel instead of intensity pixel
                    if e_i < edge_info.magnitude.len()
                        && edge_info.magnitude[e_i] > self.edge_threshold
                    {
                        let pixel =
                            self.angle_to_edge_pixel(edge_info.angle[e_i], edge_info.magnitude[e_i]);
                        a_frame.set_pixel(x, y, pixel);
                    } else {
                        self.set_intensity_pixel(i_frame, a_frame, x, y, i_x, i_y);
                    }
                }
            }
        } else {
            // edge detection disabled, intensity-only mode
            for y in 0..a_frame.h {
                for x in 0..a_frame.w {
                    let i_x = (x as f32 * scale_x) as usize;
                    let i_y = (y as f32 * scale_y) as usize;
                    self.set_intensity_pixel(i_frame, a_frame, x, y, i_x, i_y);
                }
            }
        }

        Ok(())
    }

    /// Set intensity-based pixel at the given ASCII frame coordinates.
    #[inline]
    fn set_intensity_pixel(
        &self,
        i_frame: &ImageFrame,
        a_frame: &mut TextFrame,
        x: usize,
        y: usize,
        i_x: usize,
        i_y: usize,
    ) {
        if let Some(rgb) = i_frame.get_pixel(i_x, i_y) {
            let rgb_adj = self.adjust_pixel(rgb);
            let intensity = ImageFrame::calculate_intensity_u8(rgb_adj);

            let idx = (intensity as f32 / 255.0 * self.intensity_levels as f32) as usize;
            let idx = idx.min(self.intensity_levels - 1);

            a_frame.set_pixel(x, y, FramePixel::intensity(idx as u8));
        }
    }

    /// Alter the color channels of an RGB pixel according to the specified
    /// `contrast` and `brightness` values.
    fn adjust_pixel(&self, (r, g, b): (u8, u8, u8)) -> (u8, u8, u8) {
        // closure to independently modify RGB channels
        let apply = |value: u8| -> u8 {
            // normalize color value (0-255) between 0.0 and 1.0
            let mut v = value as f32 / 255.0;
            v = (v - 0.5) * self.contrast + 0.5;
            v += self.brightness;
            // floor of 0.0 and ceiling of 1.0 (prevent overflow)
            v = v.max(0.0).min(1.0);
            (v * 255.0) as u8
        };

        (apply(r), apply(g), apply(b))
    }

    /// Normalizes an angle to 0-180 degrees, then maps the angle to an
    /// edge pixel based on magnitude and angle degree
    fn angle_to_edge_pixel(&self, angle: f32, magnitude: f32) -> FramePixel {
        // normalizing to 0-180
        let angle_d = ((angle.to_degrees() % 180.0) + 180.0) % 180.0;

        let idx = ((magnitude / 255.0) * (self.edge_char_count as f32))
            .min((self.edge_char_count - 1) as f32) as u8;

        if (angle_d >= 0.0 && angle_d < 22.5) || (angle_d >= 157.5 && angle_d < 180.0) {
            // gradient ~0 (horizontal gradient) -> vertical edge
            FramePixel::vertical_edge(idx)
        } else if (angle_d >= 22.5) && (angle_d < 67.5) {
            // gradient ~45 -> forward diagonal edge
            FramePixel::forward_diagonal(idx)
        } else if (angle_d >= 67.5) && (angle_d < 112.5) {
            // gradient ~90 (vertical gradient) -> horizontal edge
            FramePixel::horizontal_edge(idx)
        } else {
            // gradient ~135 -> back diagonal edge
            FramePixel::back_diagonal(idx)
        }
    }
}
