//! Functions & structs to parse CLI arguments & apply overrides

use std::path::PathBuf;
use clap::{Parser, Subcommand, ValueEnum};
use crate::config::{DimensionPreset, PinholeConfig};
use crate::mock_frame_generator::PatternType;


/// Essentially `PatternType`, separate to keep `mock_frame_generator.rs`
/// reusable without requiring `clap`
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, ValueEnum)]
pub(crate) enum TestPattern {
    /// Checkerboard pattern
    Checkerboard,
    /// Horizontal line moving from top to bottom
    MovingLine,
}

/// Webcam, Screen, and (to be implemented) File
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, ValueEnum)]
pub(crate) enum SourceType {
    /// Webcam capture
    Webcam,
    /// Screen capture
    Screen,
}

impl From<TestPattern> for PatternType {
    fn from(pattern: TestPattern) -> Self {
        match pattern {
            TestPattern::Checkerboard => PatternType::Checkerboard,
            TestPattern::MovingLine => PatternType::MovingLine,
        }
    }
}

/// Terminal-based video calling client.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub(crate) struct Args {
    /// Configuration file path
    #[arg(short = 'c', long, default_value = "pinhole.toml", global = true)]
    pub(crate) config: PathBuf,

    /// Test pattern (if not using a camera)
    #[arg(short = 'p', long, global = true)]
    pub(crate) test_pattern: Option<TestPattern>,

    /// Dimension preset (small, medium, large, xlarge)
    #[arg(long, global = true)]
    pub(crate) preset: Option<String>,

    /// Render window width (overrides config and preset)
    #[arg(short = 'W', long, global = true)]
    pub(crate) width: Option<usize>,

    /// Render window height (overrides config and preset)
    #[arg(short = 'H', long, global = true)]
    pub(crate) height: Option<usize>,

    /// Video source type (overrides config)
    #[arg(long, global = true)]
    pub(crate) source: Option<SourceType>,

    /// Disable edge detection (improves performance at high resolutions)
    #[arg(long, global = true)]
    pub(crate) no_edges: bool,

    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Subcommand, Debug)]
pub(crate) enum Command {
    /// Host a session and generate a room code for others to join.
    Host {
        /// Room API server URL
        #[arg(long, default_value = "http://localhost:8000")]
        api_url: String,

        /// TCP server address
        #[arg(short = 't', long, default_value = "127.0.0.1:8080")]
        tcp_addr: String,

        /// UDP server address
        #[arg(short = 'u', long, default_value = "127.0.0.1:4433")]
        udp_addr: String,
    },

    /// Join a session using a room code.
    Join {
        /// The room code to join (e.g., swift-river-42)
        room_code: String,

        /// Room API server URL
        #[arg(long, default_value = "http://localhost:8000")]
        api_url: String,

        /// TCP server address
        #[arg(short = 't', long, default_value = "127.0.0.1:8080")]
        tcp_addr: String,

        /// UDP server address
        #[arg(short = 'u', long, default_value = "127.0.0.1:4433")]
        udp_addr: String,
    },

    /// Local preview without server connection.
    Solo,

    /// Direct connection with manual session ID (legacy mode).
    Connect {
        /// TCP server address
        #[arg(short = 't', long)]
        tcp_addr: Option<String>,

        /// UDP server address
        #[arg(short = 'u', long)]
        udp_addr: Option<String>,

        /// Session ID to join
        #[arg(short = 's', long)]
        session_id: Option<String>,
    },
}

/// Applies dimension overrides from CLI arguments.
pub(crate) fn apply_dimension_overrides(config: &mut PinholeConfig, args: &Args) {
    if let Some(preset_str) = &args.preset {
        if let Some(preset) = DimensionPreset::from_str(preset_str) {
            let (w, h) = preset.dimensions();
            config.ascii.width = w;
            config.ascii.height = h;
            println!("Using dimension preset '{}': {}x{}", preset_str, w, h);

            if !preset.is_udp_safe() {
                eprintln!(
                    "WARNING: Frame size ~{} bytes exceeds safe UDP limit (1400 bytes)",
                    preset.frame_size()
                );
            }
        } else {
            eprintln!("Warning: Unknown preset '{}', ignoring", preset_str);
        }
    }

    if let Some(width) = args.width {
        config.ascii.width = width;
        println!("ASCII width override: {}", width);
    }

    if let Some(height) = args.height {
        config.ascii.height = height;
        println!("ASCII height override: {}", height);
    }

    // warn about custom dimensions
    if (args.width.is_some() || args.height.is_some()) && args.preset.is_none() {
        let frame_size = 17 + (config.ascii.width * config.ascii.height);
        if frame_size > 1400 {
            eprintln!(
                "WARNING: Frame size ~{} bytes exceeds safe UDP limit (1400 bytes)",
                frame_size
            );
        }
    }
}

/// Applies source type override from CLI arguments.
pub(crate) fn apply_source_override(config: &mut PinholeConfig, args: &Args) {
    if let Some(source) = &args.source {
        let source_str = match source {
            SourceType::Webcam => "webcam",
            SourceType::Screen => "screen",
        };
        config.video.source.r#type = source_str.to_string();
        println!("Video source override: {}", source_str);
    }
}

/// Applies image processing overrides from CLI arguments.
pub(crate) fn apply_image_processing_overrides(config: &mut PinholeConfig, args: &Args) {
    if args.no_edges {
        config.image_processing.edge_detection = false;
        println!("Edge detection disabled");
    }
}