use common::frame_pixel::{MAX_EDGE_CHARS, MAX_INTENSITY_CHARS};
use config as config_crate; // external config crate for TOML loading
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::path::Path;
use crate::terminal::TerminalOverrides;

/// Predefined dimension presets
#[derive(Debug, Clone, Copy)]
pub enum DimensionPreset {
    Small,   // 80x24 - 1937 bytes (UDP safe)
    Medium,  // 120x40 - 4817 bytes (UDP safe)
    Large,   // 160x50 - 8017 bytes (may fragment)
    XLarge,  // 200x60 - 12017 bytes (requires TCP or chunking)
}

impl DimensionPreset {
    pub fn dimensions(&self) -> (usize, usize) {
        match self {
            DimensionPreset::Small => (80, 24),
            DimensionPreset::Medium => (120, 40),
            DimensionPreset::Large => (160, 50),
            DimensionPreset::XLarge => (200, 60),
        }
    }

    /// Returns approximate full frame size in bytes
    pub fn frame_size(&self) -> usize {
        let (w, h) = self.dimensions();
        17 + (w * h) // 17 byte header + width * height pixels
    }

    /// Returns true if this preset is safe for UDP without fragmentation
    pub fn is_udp_safe(&self) -> bool {
        self.frame_size() <= 1400
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "small" | "s" => Some(DimensionPreset::Small),
            "medium" | "m" => Some(DimensionPreset::Medium),
            "large" | "l" => Some(DimensionPreset::Large),
            "xlarge" | "xl" => Some(DimensionPreset::XLarge),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PinholeConfig {
    #[serde(default)]
    pub video: VideoSettings,
    #[serde(default)]
    pub ascii: AsciiSettings,
    #[serde(default)]
    pub image_processing: ImageProcessingSettings,
    #[serde(default)]
    pub performance: PerformanceSettings,
    #[serde(default)]
    pub network: NetworkSettings,
    #[serde(default)]
    pub terminal: TerminalOverrides
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoSettings {
    #[serde(default)]
    pub source: VideoSource,
    #[serde(default)]
    pub ffmpeg: FfmpegSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoSource {
    #[serde(default = "default_source_type")]
    pub r#type: String, // "webcam", "screen", "file", "custom"
    #[serde(default)]
    pub webcam: WebcamSettings,
    #[serde(default)]
    pub screen: ScreenSettings,
    #[serde(default)]
    pub file: FileSettings,
    #[serde(default)]
    pub custom: CustomSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebcamSettings {
    #[serde(default = "default_webcam_device")]
    pub device: String,
    #[serde(default = "default_width")]
    pub width: usize,
    #[serde(default = "default_height")]
    pub height: usize,
    #[serde(default = "default_framerate")]
    pub framerate: u32,
    #[serde(default = "default_pixel_format")]
    pub pixel_format: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenSettings {
    #[serde(default = "default_screen_device")]
    pub device: String,
    #[serde(default = "default_screen_width")]
    pub width: usize,
    #[serde(default = "default_screen_height")]
    pub height: usize,
    #[serde(default = "default_framerate")]
    pub framerate: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSettings {
    #[serde(default)]
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomSettings {
    #[serde(default)]
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FfmpegSettings {
    #[serde(default = "default_probesize")]
    pub probesize: u32,
    #[serde(default = "default_analyzeduration")]
    pub analyzeduration: u32,
    #[serde(default = "default_fflags")]
    pub fflags: String,
    #[serde(default = "default_flags")]
    pub flags: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsciiSettings {
    #[serde(default = "default_ascii_width")]
    pub width: usize,
    #[serde(default = "default_ascii_height")]
    pub height: usize,
    #[serde(default)]
    pub chars: AsciiChars,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsciiChars {
    #[serde(default = "default_intensity_chars")]
    pub intensity: String,
    #[serde(default = "default_horizontal_lines_chars")]
    pub horizontal_lines: String,
    #[serde(default = "default_vertical_lines_chars")]
    pub vertical_lines: String,
    #[serde(default = "default_forward_diagonal_chars")]
    pub forward_diagonal: String,
    #[serde(default = "default_back_diagonal_chars")]
    pub back_diagonal: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageProcessingSettings {
    #[serde(default = "default_edge_threshold")]
    pub edge_threshold: f32,
    #[serde(default = "default_contrast")]
    pub contrast: f32,
    #[serde(default = "default_brightness")]
    pub brightness: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceSettings {
    #[serde(default = "default_frame_buffer")]
    pub frame_buffer: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkSettings {
    #[serde(default = "default_tcp_addr")]
    pub tcp_addr: String,
    #[serde(default = "default_udp_addr")]
    pub udp_addr: String,
    #[serde(default)]
    pub session_id: String,
}

// Default functions
fn default_source_type() -> String {
    "webcam".to_string()
}

fn default_webcam_device() -> String {
    if cfg!(target_os = "macos") {
        "0:none".to_string()
    } else if cfg!(target_os = "linux") {
        "/dev/video0".to_string()
    } else if cfg!(target_os = "windows") {
        "video=Integrated Camera".to_string()
    } else {
        "0".to_string()
    }
}

fn default_screen_device() -> String {
    if cfg!(target_os = "macos") {
        "1:none".to_string()
    } else if cfg!(target_os = "linux") {
        ":0.0".to_string()
    } else if cfg!(target_os = "windows") {
        "desktop".to_string()
    } else {
        "0".to_string()
    }
}

fn default_width() -> usize {
    640
}

fn default_height() -> usize {
    480
}

fn default_screen_width() -> usize {
    1920
}

fn default_screen_height() -> usize {
    1080
}

fn default_framerate() -> u32 {
    30
}

fn default_pixel_format() -> String {
    "rgb24".to_string()
}

fn default_probesize() -> u32 {
    32
}

fn default_analyzeduration() -> u32 {
    0
}

fn default_fflags() -> String {
    "nobuffer".to_string()
}

fn default_flags() -> String {
    "low_delay".to_string()
}

fn default_ascii_width() -> usize {
    120
}

fn default_ascii_height() -> usize {
    40
}

fn default_intensity_chars() -> String {
    " .:coPO?@■".to_string()
}

fn default_horizontal_lines_chars() -> String {
    "-━═".to_string()
}

fn default_vertical_lines_chars() -> String {
    "|│┃".to_string()
}

fn default_forward_diagonal_chars() -> String {
    "/╱⟋".to_string()
}

fn default_back_diagonal_chars() -> String {
    "\\╲⟍".to_string()
}

fn default_edge_threshold() -> f32 {
    127.5
}

fn default_contrast() -> f32 {
    1.5
}

fn default_brightness() -> f32 {
    0.0
}

fn default_frame_buffer() -> usize {
    30
}

fn default_tcp_addr() -> String {
    "127.0.0.1:8080".to_string()
}

fn default_udp_addr() -> String {
    "127.0.0.1:4433".to_string()
}

impl Default for PinholeConfig {
    fn default() -> Self {
        Self {
            video: VideoSettings::default(),
            ascii: AsciiSettings::default(),
            image_processing: ImageProcessingSettings::default(),
            performance: PerformanceSettings::default(),
            network: NetworkSettings::default(),
            terminal: TerminalOverrides::default(),
        }
    }
}

impl Default for VideoSettings {
    fn default() -> Self {
        Self {
            source: VideoSource::default(),
            ffmpeg: FfmpegSettings::default(),
        }
    }
}

impl Default for VideoSource {
    fn default() -> Self {
        Self {
            r#type: default_source_type(),
            webcam: WebcamSettings::default(),
            screen: ScreenSettings::default(),
            file: FileSettings::default(),
            custom: CustomSettings::default(),
        }
    }
}

impl Default for WebcamSettings {
    fn default() -> Self {
        Self {
            device: default_webcam_device(),
            width: default_width(),
            height: default_height(),
            framerate: default_framerate(),
            pixel_format: default_pixel_format(),
        }
    }
}

impl Default for ScreenSettings {
    fn default() -> Self {
        Self {
            device: default_screen_device(),
            width: default_screen_width(),
            height: default_screen_height(),
            framerate: default_framerate(),
        }
    }
}

impl Default for FileSettings {
    fn default() -> Self {
        Self {
            path: String::new(),
        }
    }
}

impl Default for CustomSettings {
    fn default() -> Self {
        Self { args: Vec::new() }
    }
}

impl Default for FfmpegSettings {
    fn default() -> Self {
        Self {
            probesize: default_probesize(),
            analyzeduration: default_analyzeduration(),
            fflags: default_fflags(),
            flags: default_flags(),
        }
    }
}

impl Default for AsciiSettings {
    fn default() -> Self {
        Self {
            width: default_ascii_width(),
            height: default_ascii_height(),
            chars: AsciiChars::default(),
        }
    }
}

impl Default for AsciiChars {
    fn default() -> Self {
        Self {
            intensity: default_intensity_chars(),
            horizontal_lines: default_horizontal_lines_chars(),
            vertical_lines: default_vertical_lines_chars(),
            forward_diagonal: default_forward_diagonal_chars(),
            back_diagonal: default_back_diagonal_chars(),
        }
    }
}

impl AsciiChars {
    /// Validate that character counts are within allowed limits
    pub fn validate(&self) -> Result<(), String> {
        let intensity_count = self.intensity.chars().count();
        if intensity_count == 0 {
            return Err("intensity characters cannot be empty".into());
        }
        if intensity_count > MAX_INTENSITY_CHARS {
            return Err(format!(
                "intensity characters ({}) exceed maximum allowed ({})",
                intensity_count, MAX_INTENSITY_CHARS
            ));
        }

        let horizontal_count = self.horizontal_lines.chars().count();
        if horizontal_count == 0 {
            return Err("horizontal line characters cannot be empty".into());
        }
        if horizontal_count > MAX_EDGE_CHARS {
            return Err(format!(
                "horizontal line characters ({}) exceed maximum allowed ({})",
                horizontal_count, MAX_EDGE_CHARS
            ));
        }

        let vertical_count = self.vertical_lines.chars().count();
        if vertical_count == 0 {
            return Err("vertical line characters cannot be empty".into());
        }
        if vertical_count > MAX_EDGE_CHARS {
            return Err(format!(
                "vertical line characters ({}) exceed maximum allowed ({})",
                vertical_count, MAX_EDGE_CHARS
            ));
        }

        let forward_count = self.forward_diagonal.chars().count();
        if forward_count == 0 {
            return Err("forward diagonal characters cannot be empty".into());
        }
        if forward_count > MAX_EDGE_CHARS {
            return Err(format!(
                "forward diagonal characters ({}) exceed maximum allowed ({})",
                forward_count, MAX_EDGE_CHARS
            ));
        }

        let back_count = self.back_diagonal.chars().count();
        if back_count == 0 {
            return Err("back diagonal characters cannot be empty".into());
        }
        if back_count > MAX_EDGE_CHARS {
            return Err(format!(
                "back diagonal characters ({}) exceed maximum allowed ({})",
                back_count, MAX_EDGE_CHARS
            ));
        }

        Ok(())
    }
}

impl Default for ImageProcessingSettings {
    fn default() -> Self {
        Self {
            edge_threshold: default_edge_threshold(),
            contrast: default_contrast(),
            brightness: default_brightness(),
        }
    }
}

impl Default for PerformanceSettings {
    fn default() -> Self {
        Self {
            frame_buffer: default_frame_buffer(),
        }
    }
}

impl Default for NetworkSettings {
    fn default() -> Self {
        Self {
            tcp_addr: default_tcp_addr(),
            udp_addr: default_udp_addr(),
            session_id: String::new(),
        }
    }
}

impl PinholeConfig {
    /// Load configuration from a TOML file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn Error>> {
        let settings = config_crate::Config::builder()
            .add_source(config_crate::File::from(path.as_ref()))
            .build()?;

        let config: Self = settings.try_deserialize()?;

        // validate ASCII character counts
        config.ascii.chars.validate()?;

        Ok(config)
    }

    /// Load configuration from a TOML file with optional fallback to defaults
    pub fn from_file_or_default<P: AsRef<Path>>(path: P) -> Self {
        Self::from_file(path).unwrap_or_default()
    }

    /// Create a default configuration
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the framerate from the active video source.
    /// For file/custom sources, returns a default of 30 fps.
    pub fn get_source_framerate(&self) -> u32 {
        match self.video.source.r#type.as_str() {
            "webcam" => self.video.source.webcam.framerate,
            "screen" => self.video.source.screen.framerate,
            // files have inherent framerate, custom is user-defined
            _ => 30,
        }
    }
}
