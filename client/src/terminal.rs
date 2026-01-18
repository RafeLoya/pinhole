use serde::{Deserialize, Serialize};
use termwiz::caps::{Capabilities, ColorLevel, ProbeHints};

/// User-defined terminal capabilities, overrides runtime-detected values
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TerminalOverrides {
    pub color_level: Option<ColorLevelOverride>,
    pub hyperlinks: Option<bool>,
    pub iterm2_img: Option<bool>,
    pub sixel: Option<bool>,
    pub bce: Option<bool>,
    pub mouse_reporting: Option<bool>,
    pub bracketed_paste: Option<bool>,
}

/// Enums mapping to `ColorLevel`
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ColorLevelOverride {
    Monochrome,
    Sixteen,
    #[serde(rename = "256")]
    TwoFiftySix,
    TrueColor
}

impl From<ColorLevelOverride> for ColorLevel {
    fn from(val: ColorLevelOverride) -> Self {
        match val {
            ColorLevelOverride::Monochrome => ColorLevel::MonoChrome,
            ColorLevelOverride::Sixteen => ColorLevel::Sixteen,
            ColorLevelOverride::TwoFiftySix => ColorLevel::TwoFiftySix,
            ColorLevelOverride::TrueColor => ColorLevel::TrueColor
        }
    }
}

/// Features & information available within a terminal emulator
///
/// Unless a valid override value is provided, this will be configured
/// during runtime.
pub struct TerminalInfo {
    /// Features, assessed during runtime
    caps: Capabilities,
    /// User-defined overrides
    overrides: TerminalOverrides,
}

impl TerminalInfo {
    /// Determine terminal emulator features during runtime
    /// & override with user-defined settings
    pub fn detect(overrides: TerminalOverrides) -> anyhow::Result<TerminalInfo> {
        let hints = ProbeHints::new_from_env();
        let caps = Capabilities::new_with_hints(hints)?;
        Ok(Self { caps, overrides })
    }

    pub fn color_level(&self) -> ColorLevel {
        self.overrides
            .color_level
            .map(ColorLevel::from)
            .unwrap_or_else(|| self.caps.color_level())
    }

    pub fn hyperlinks(&self) -> bool {
        self.overrides
            .hyperlinks
            .unwrap_or_else(|| self.caps.hyperlinks())
    }

    pub fn iterm2_image(&self) -> bool {
        self.overrides
            .iterm2_img
            .unwrap_or_else(|| self.caps.iterm2_image())
    }

    pub fn sixel(&self) -> bool {
        self.overrides
            .sixel
            .unwrap_or_else(|| self.caps.sixel())
    }

    pub fn bce(&self) -> bool {
        self.overrides
            .bce
            .unwrap_or_else(|| self.caps.bce())
    }

    pub fn mouse_reporting(&self) -> bool {
        self.overrides
            .mouse_reporting
            .unwrap_or_else(|| self.caps.mouse_reporting())
    }

    pub fn bracketed_paste(&self) -> bool {
        self.overrides
            .bracketed_paste
            .unwrap_or_else(|| self.caps.bracketed_paste())
    }
}