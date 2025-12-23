use crate::config::PinholeConfig;

/// Shared configuration values used by different systems
/// in the entire program
pub struct VideoConfig {
    pub camera_width: usize,
    pub camera_height: usize,
    pub ascii_width: usize,
    pub ascii_height: usize,
    pub edge_threshold: f32,
    pub contrast: f32,
    pub brightness: f32,
}

impl VideoConfig {
    pub fn default() -> Self {
        Self {
            camera_width: 640,
            camera_height: 480,
            ascii_width: 120,
            ascii_height: 40,
            edge_threshold: 127.50,
            contrast: 1.5,
            brightness: 0.0,
        }
    }

    /// Create VideoConfig from PinholeConfig
    pub fn from_pinhole_config(config: &PinholeConfig) -> Self {
        let (width, height) = match config.video.source.r#type.as_str() {
            "webcam" => (
                config.video.source.webcam.width,
                config.video.source.webcam.height,
            ),
            "screen" => (
                config.video.source.screen.width,
                config.video.source.screen.height,
            ),
            _ => (640, 480), // fallback for file/custom
        };

        Self {
            camera_width: width,
            camera_height: height,
            ascii_width: config.ascii.width,
            ascii_height: config.ascii.height,
            edge_threshold: config.image_processing.edge_threshold,
            contrast: config.image_processing.contrast,
            brightness: config.image_processing.brightness,
        }
    }

    pub fn new(
        camera_width: usize,
        camera_height: usize,
        ascii_width: usize,
        ascii_height: usize,
        edge_threshold: f32,
        contrast: f32,
        brightness: f32,
    ) -> Self {
        Self {
            camera_width,
            camera_height,
            ascii_width,
            ascii_height,
            edge_threshold,
            contrast,
            brightness,
        }
    }
}
