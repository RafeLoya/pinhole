use crate::edge_detector::EdgeDetector;
use crate::image_frame::ImageFrame;
use common::ascii_frame::AsciiFrame;
use std::error::Error;

/// The coefficients below are derived from Rec. ITU-R BT.601-7.
/// In the specification, these luminance coefficients represent
/// how much they influence / contribute to the human eye's
/// perception of brightness.

pub const R_LUMINANCE: f32 = 0.2989;
pub const G_LUMINANCE: f32 = 0.5870;
pub const B_LUMINANCE: f32 = 0.1140;

/// Intermediary translator to transform an `ImageFrame` into an `AsciiFrame`
pub struct AsciiConverter {
    /// Identifies edges in given `ImageFrame`s
    edge_detector: EdgeDetector,
    /// Intensity for `AsciiFrame` "pixels"
    ascii_intensity: Vec<char>,
    /// Characters for horizontal edges in `AsciiFrame` representation
    ///
    /// NOTE: 'horizontal' is in reference to the gradient direction, not
    /// their angle in the original / ASCII image, so these will actually
    /// be vertical characters in the final output (i.e. "|│┃")
    ascii_horizontal: Vec<char>,
    /// Characters for vertical edges in `AsciiFrame` representation
    ///
    /// NOTE: 'vertical' here means is in reference to the gradient direction,
    /// not their angle in the original / ASCII image, so these will actually
    /// be horizontal characters in the final output (i.e. "-━═")
    ascii_vertical: Vec<char>,
    /// Characters for forward edges in `AsciiFrame` representation
    ascii_forward: Vec<char>,
    /// Characters for back edges in `AsciiFrame` representation
    ascii_back: Vec<char>,
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
    pub const DEFAULT_ASCII_VERTICAL: &'static str = "-━═";
    pub const DEFAULT_ASCII_HORIZONTAL: &'static str = "|│┃";
    pub const DEFAULT_ASCII_FORWARD: &'static str = "/╱⟋";
    pub const DEFAULT_ASCII_BACK: &'static str = "\\╲⟍";
    pub const DEFAULT_CONTRAST: f32 = 1.5;
    pub const DEFAULT_BRIGHTNESS: f32 = 0.0;

    pub fn new(
        ascii_intensity: Vec<char>,
        ascii_horizontal: Vec<char>,
        ascii_vertical: Vec<char>,
        ascii_forward: Vec<char>,
        ascii_back: Vec<char>,
        w: usize,
        h: usize,
        edge_threshold: f32,
        contrast: f32,
        brightness: f32,
    ) -> Result<Self, Box<dyn Error>> {
        let edge_detector = EdgeDetector::new(w, h, edge_threshold);

        edge_detector.start(w, h)?;

        Ok(Self {
            edge_detector,
            ascii_intensity,
            ascii_horizontal,
            ascii_vertical,
            ascii_forward,
            ascii_back,
            edge_threshold,
            contrast,
            brightness,
        })
    }

    pub fn default() -> Result<Self, Box<dyn Error>> {
        Self::new(
            Self::DEFAULT_ASCII_INTENSITY.chars().collect(),
            Self::DEFAULT_ASCII_HORIZONTAL.chars().collect(),
            Self::DEFAULT_ASCII_VERTICAL.chars().collect(),
            Self::DEFAULT_ASCII_FORWARD.chars().collect(),
            Self::DEFAULT_ASCII_BACK.chars().collect(),
            640,
            480,
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
    /// dimensions to the target `AsciiFrame`'s dimensions
    pub fn convert(
        &self,
        i_frame: &ImageFrame,
        a_frame: &mut AsciiFrame,
    ) -> Result<(), Box<dyn Error>> {
        // submit the original image to the edge detector
        self.edge_detector.submit_frame(i_frame)?;

        // scaling factors to map the ASCII frame's dimension
        // to the original image's dimension
        let scale_x = i_frame.w as f32 / a_frame.w as f32;
        let scale_y = i_frame.h as f32 / a_frame.h as f32;

        // retrieve processed edge info
        let edge_info = self.edge_detector.get_edge_info()?;

        for y in 0..a_frame.h {
            for x in 0..a_frame.w {
                let i_x = (x as f32 * scale_x) as usize;
                let i_y = (y as f32 * scale_y) as usize;
                let e_i = i_y.min(edge_info.h - 1) * edge_info.w + i_x.min(edge_info.w - 1);

                // if an edge's magnitude is greater than the threshold,
                // assign edge character instead of regular character
                if e_i < edge_info.magnitude.len() && edge_info.magnitude[e_i] > self.edge_threshold
                {
                    let c = self.angle_to_edge(edge_info.angle[e_i], edge_info.magnitude[e_i]);
                    a_frame.set_char(x, y, c);
                } else {
                    // No significant edge, retrieve RGB values from
                    // scaled pixel destination in image frame and
                    // map by intensity
                    if let Some(rgb) = i_frame.get_pixel(i_x, i_y) {
                        // modify RGB w/ given brightness & contrast values
                        let rgb_adj = self.adjust_pixel(rgb);
                        let intensity = ImageFrame::calculate_intensity_u8(rgb_adj);

                        let char_i =
                            (intensity as f32 / 255.0 * self.ascii_intensity.len() as f32) as usize;
                        // bounds check (e.g. floating point rounding error)
                        let char_i = char_i.min(self.ascii_intensity.len() - 1);

                        a_frame.set_char(x, y, self.ascii_intensity[char_i]);
                    }
                }
            }
        }

        Ok(())
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
    /// angle character based on magnitude and angle degree
    fn angle_to_edge(&self, angle: f32, magnitude: f32) -> char {
        // normalizing to 0-180
        let angle_d = ((angle.to_degrees() % 180.0) + 180.0) % 180.0;

        let char_i = ((magnitude / 255.0) * (self.ascii_horizontal.len() as f32))
            .min((self.ascii_horizontal.len() - 1) as f32) as usize;

        if (angle_d >= 0.0 && angle_d < 22.5) || (angle_d >= 157.5 && angle_d < 180.0) {
            self.ascii_horizontal[char_i.min(self.ascii_horizontal.len() - 1)]
        } else if (angle_d >= 22.5) && (angle_d < 67.5) {
            self.ascii_forward[char_i.min(self.ascii_forward.len() - 1)]
        } else if (angle_d >= 67.5) && (angle_d < 112.5) {
            self.ascii_vertical[char_i.min(self.ascii_vertical.len() - 1)]
        } else {
            self.ascii_back[char_i.min(self.ascii_back.len() - 1)]
        }
    }
}
