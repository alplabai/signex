//! 6 built-in themes for the Signex EDA application.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Color
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Parse "#RRGGBB" or "#RRGGBBAA" hex string.
    pub fn from_hex(hex: &str) -> Self {
        let hex = hex.trim_start_matches('#');
        let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
        let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
        let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
        let a = if hex.len() >= 8 {
            u8::from_str_radix(&hex[6..8], 16).unwrap_or(255)
        } else {
            255
        };
        Self { r, g, b, a }
    }
}

// ---------------------------------------------------------------------------
// Theme identity
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeId {
    CatppuccinMocha,
    VsCodeDark,
    AltiumDark,
    GitHubDark,
    SolarizedLight,
    Nord,
    /// User-defined custom theme (data stored externally in CustomThemeFile).
    Custom,
}

impl ThemeId {
    /// All 6 built-in themes (excludes Custom).
    pub const BUILTINS: &[ThemeId] = &[
        ThemeId::AltiumDark,
        ThemeId::VsCodeDark,
        ThemeId::CatppuccinMocha,
        ThemeId::GitHubDark,
        ThemeId::SolarizedLight,
        ThemeId::Nord,
    ];

    #[deprecated = "Use ThemeId::BUILTINS; ThemeId::ALL now includes Custom"]
    pub const ALL: &[ThemeId] = Self::BUILTINS;

    pub fn label(self) -> &'static str {
        match self {
            ThemeId::CatppuccinMocha => "Catppuccin Mocha",
            ThemeId::VsCodeDark => "VS Code Dark",
            ThemeId::AltiumDark => "Altium Dark",
            ThemeId::GitHubDark => "GitHub Dark",
            ThemeId::SolarizedLight => "Solarized Light",
            ThemeId::Nord => "Nord",
            ThemeId::Custom => "Custom",
        }
    }
}

// ---------------------------------------------------------------------------
// Custom theme file (JSON import/export)
// ---------------------------------------------------------------------------

/// A user-defined theme stored as JSON. Contains the full set of UI tokens
/// and canvas palette colours so it can be round-tripped to/from disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomThemeFile {
    /// Display name shown in the Preferences dialog.
    pub name: String,
    /// Panel chrome / UI widget colour tokens.
    pub tokens: ThemeTokens,
    /// Schematic canvas colour palette.
    pub canvas: CanvasColors,
}

// ---------------------------------------------------------------------------
// UI chrome tokens
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemeTokens {
    pub bg: Color,
    pub paper: Color,
    pub text: Color,
    pub text_secondary: Color,
    pub accent: Color,
    pub border: Color,
    pub panel_bg: Color,
    pub toolbar_bg: Color,
    pub statusbar_bg: Color,
    pub selection: Color,
    pub hover: Color,
    pub error: Color,
    pub warning: Color,
    pub success: Color,
}

// ---------------------------------------------------------------------------
// Canvas / schematic palette
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanvasColors {
    pub background: Color,
    pub paper: Color,
    pub wire: Color,
    pub junction: Color,
    pub body: Color,
    pub body_fill: Color,
    pub pin: Color,
    pub reference: Color,
    pub value: Color,
    pub net_label: Color,
    pub global_label: Color,
    pub hier_label: Color,
    pub no_connect: Color,
    pub power: Color,
    pub selection: Color,
    pub bus: Color,
    pub grid: Color,
    pub cursor: Color,
}

// ---------------------------------------------------------------------------
// Const helper
// ---------------------------------------------------------------------------

const fn c(r: u8, g: u8, b: u8) -> Color {
    Color { r, g, b, a: 0xFF }
}

// ===== Catppuccin Mocha =====

const CATPPUCCIN_MOCHA_TOKENS: ThemeTokens = ThemeTokens {
    bg: c(0x1E, 0x1E, 0x2E),
    paper: c(0x31, 0x32, 0x44),
    text: c(0xCD, 0xD6, 0xF4),
    text_secondary: c(0xA6, 0xAD, 0xC8),
    accent: c(0x89, 0xB4, 0xFA),
    border: c(0x45, 0x47, 0x5A),
    panel_bg: c(0x18, 0x18, 0x25),
    toolbar_bg: c(0x1E, 0x1E, 0x2E),
    statusbar_bg: c(0x18, 0x18, 0x25),
    selection: c(0x58, 0x5B, 0x70),
    hover: c(0x45, 0x47, 0x5A),
    error: c(0xF3, 0x8B, 0xA8),
    warning: c(0xFA, 0xB3, 0x87),
    success: c(0xA6, 0xE3, 0xA1),
};

const CATPPUCCIN_MOCHA_CANVAS: CanvasColors = CanvasColors {
    background: c(0x1E, 0x1E, 0x2E),
    paper: c(0x31, 0x32, 0x44),
    wire: c(0xA6, 0xE3, 0xA1),
    junction: c(0xA6, 0xE3, 0xA1),
    body: c(0x89, 0xB4, 0xFA),
    body_fill: c(0x31, 0x32, 0x44),
    pin: c(0xCD, 0xD6, 0xF4),
    reference: c(0x89, 0xB4, 0xFA),
    value: c(0xF5, 0xC2, 0xE7),
    net_label: c(0x94, 0xE2, 0xD5),
    global_label: c(0xF9, 0xE2, 0xAF),
    hier_label: c(0xFA, 0xB3, 0x87),
    no_connect: c(0xF3, 0x8B, 0xA8),
    power: c(0xF3, 0x8B, 0xA8),
    selection: c(0xF5, 0xE0, 0xDC),
    bus: c(0x89, 0xDC, 0xEB),
    grid: c(0x45, 0x47, 0x5A),
    cursor: c(0xF5, 0xE0, 0xDC),
};

// ===== VS Code Dark =====

const VSCODE_DARK_TOKENS: ThemeTokens = ThemeTokens {
    bg: c(0x1E, 0x1E, 0x1E),
    paper: c(0x25, 0x25, 0x26),
    text: c(0xD4, 0xD4, 0xD4),
    text_secondary: c(0x80, 0x80, 0x80),
    accent: c(0x00, 0x7A, 0xCC),
    border: c(0x3C, 0x3C, 0x3C),
    panel_bg: c(0x18, 0x18, 0x18),
    toolbar_bg: c(0x33, 0x33, 0x33),
    statusbar_bg: c(0x00, 0x7A, 0xCC),
    selection: c(0x26, 0x4F, 0x78),
    hover: c(0x2A, 0x2D, 0x2E),
    error: c(0xF4, 0x44, 0x47),
    warning: c(0xFF, 0x8C, 0x00),
    success: c(0x6A, 0x99, 0x55),
};

const VSCODE_DARK_CANVAS: CanvasColors = CanvasColors {
    background: c(0x1E, 0x1E, 0x1E),
    paper: c(0x25, 0x25, 0x26),
    wire: c(0x6A, 0x99, 0x55),
    junction: c(0x6A, 0x99, 0x55),
    body: c(0x56, 0x9C, 0xD6),
    body_fill: c(0x25, 0x25, 0x26),
    pin: c(0xD4, 0xD4, 0xD4),
    reference: c(0x56, 0x9C, 0xD6),
    value: c(0xCE, 0x91, 0x78),
    net_label: c(0x4E, 0xC9, 0xB0),
    global_label: c(0xDC, 0xDC, 0xAA),
    hier_label: c(0xFF, 0x8C, 0x00),
    no_connect: c(0xF4, 0x44, 0x47),
    power: c(0xF4, 0x44, 0x47),
    selection: c(0xFF, 0xFF, 0xFF),
    bus: c(0x9C, 0xDC, 0xFE),
    grid: c(0x3C, 0x3C, 0x3C),
    cursor: c(0xFF, 0xFF, 0xFF),
};

// ===== Altium Dark =====

const ALTIUM_DARK_TOKENS: ThemeTokens = ThemeTokens {
    bg: c(0x2D, 0x2D, 0x30),
    paper: c(0x1E, 0x1E, 0x1E),
    text: c(0xDC, 0xDC, 0xDC),
    text_secondary: c(0x9B, 0x9B, 0x9B),
    accent: c(0xE8, 0x91, 0x2D),
    border: c(0x3F, 0x3F, 0x46),
    panel_bg: c(0x25, 0x25, 0x28),
    toolbar_bg: c(0x2D, 0x2D, 0x30),
    statusbar_bg: c(0x25, 0x25, 0x28),
    selection: c(0x51, 0x51, 0x55),
    hover: c(0x3F, 0x3F, 0x46),
    error: c(0xF4, 0x4E, 0x4E),
    warning: c(0xE8, 0x91, 0x2D),
    success: c(0x57, 0xA6, 0x4A),
};

const ALTIUM_DARK_CANVAS: CanvasColors = CanvasColors {
    background: c(0x2D, 0x2D, 0x30),
    paper: c(0x1E, 0x1E, 0x1E),
    wire: c(0x57, 0xA6, 0x4A),
    junction: c(0x57, 0xA6, 0x4A),
    body: c(0xE8, 0x91, 0x2D),
    body_fill: c(0x1E, 0x1E, 0x1E),
    pin: c(0xDC, 0xDC, 0xDC),
    reference: c(0xE8, 0x91, 0x2D),
    value: c(0x9B, 0xDB, 0xF4),
    net_label: c(0x4E, 0xC9, 0xB0),
    global_label: c(0xDC, 0xDC, 0xAA),
    hier_label: c(0xE8, 0x91, 0x2D),
    no_connect: c(0xF4, 0x4E, 0x4E),
    power: c(0xF4, 0x4E, 0x4E),
    selection: c(0xFF, 0xFF, 0xFF),
    bus: c(0x56, 0x9C, 0xD6),
    grid: c(0x3F, 0x3F, 0x46),
    cursor: c(0xFF, 0xFF, 0xFF),
};

// ===== GitHub Dark =====

const GITHUB_DARK_TOKENS: ThemeTokens = ThemeTokens {
    bg: c(0x0D, 0x11, 0x17),
    paper: c(0x16, 0x1B, 0x22),
    text: c(0xE6, 0xED, 0xF3),
    text_secondary: c(0x8B, 0x94, 0x9E),
    accent: c(0x58, 0xA6, 0xFF),
    border: c(0x30, 0x36, 0x3D),
    panel_bg: c(0x01, 0x04, 0x09),
    toolbar_bg: c(0x16, 0x1B, 0x22),
    statusbar_bg: c(0x01, 0x04, 0x09),
    selection: c(0x38, 0x8B, 0xFD),
    hover: c(0x17, 0x1B, 0x22),
    error: c(0xFF, 0x7B, 0x72),
    warning: c(0xD2, 0x9A, 0x22),
    success: c(0x3F, 0xB9, 0x50),
};

const GITHUB_DARK_CANVAS: CanvasColors = CanvasColors {
    background: c(0x0D, 0x11, 0x17),
    paper: c(0x16, 0x1B, 0x22),
    wire: c(0x3F, 0xB9, 0x50),
    junction: c(0x3F, 0xB9, 0x50),
    body: c(0x58, 0xA6, 0xFF),
    body_fill: c(0x16, 0x1B, 0x22),
    pin: c(0xE6, 0xED, 0xF3),
    reference: c(0x58, 0xA6, 0xFF),
    value: c(0xD2, 0xA8, 0xFF),
    net_label: c(0x7E, 0xE7, 0x87),
    global_label: c(0xD2, 0x9A, 0x22),
    hier_label: c(0xFE, 0xA5, 0x5F),
    no_connect: c(0xFF, 0x7B, 0x72),
    power: c(0xFF, 0x7B, 0x72),
    selection: c(0xFF, 0xFF, 0xFF),
    bus: c(0x79, 0xC0, 0xFF),
    grid: c(0x30, 0x36, 0x3D),
    cursor: c(0xFF, 0xFF, 0xFF),
};

// ===== Solarized Light =====

const SOLARIZED_LIGHT_TOKENS: ThemeTokens = ThemeTokens {
    bg: c(0xFD, 0xF6, 0xE3),
    paper: c(0xEE, 0xE8, 0xD5),
    text: c(0x65, 0x7B, 0x83),
    text_secondary: c(0x93, 0xA1, 0xA1),
    accent: c(0x26, 0x8B, 0xD2),
    border: c(0x93, 0xA1, 0xA1),
    panel_bg: c(0xFD, 0xF6, 0xE3),
    toolbar_bg: c(0xEE, 0xE8, 0xD5),
    statusbar_bg: c(0xEE, 0xE8, 0xD5),
    selection: c(0x26, 0x8B, 0xD2),
    hover: c(0xEE, 0xE8, 0xD5),
    error: c(0xDC, 0x32, 0x2F),
    warning: c(0xCB, 0x4B, 0x16),
    success: c(0x85, 0x99, 0x00),
};

const SOLARIZED_LIGHT_CANVAS: CanvasColors = CanvasColors {
    background: c(0xFD, 0xF6, 0xE3),
    paper: c(0xEE, 0xE8, 0xD5),
    wire: c(0x85, 0x99, 0x00),
    junction: c(0x85, 0x99, 0x00),
    body: c(0x26, 0x8B, 0xD2),
    body_fill: c(0xEE, 0xE8, 0xD5),
    pin: c(0x65, 0x7B, 0x83),
    reference: c(0x26, 0x8B, 0xD2),
    value: c(0xD3, 0x36, 0x82),
    net_label: c(0x2A, 0xA1, 0x98),
    global_label: c(0xB5, 0x89, 0x00),
    hier_label: c(0xCB, 0x4B, 0x16),
    no_connect: c(0xDC, 0x32, 0x2F),
    power: c(0xDC, 0x32, 0x2F),
    selection: c(0x00, 0x2B, 0x36),
    bus: c(0x6C, 0x71, 0xC4),
    grid: c(0x93, 0xA1, 0xA1),
    cursor: c(0x00, 0x2B, 0x36),
};

// ===== Nord =====

const NORD_TOKENS: ThemeTokens = ThemeTokens {
    bg: c(0x2E, 0x34, 0x40),
    paper: c(0x3B, 0x42, 0x52),
    text: c(0xEC, 0xEF, 0xF4),
    text_secondary: c(0xD8, 0xDE, 0xE9),
    accent: c(0x88, 0xC0, 0xD0),
    border: c(0x4C, 0x56, 0x6A),
    panel_bg: c(0x2E, 0x34, 0x40),
    toolbar_bg: c(0x3B, 0x42, 0x52),
    statusbar_bg: c(0x2E, 0x34, 0x40),
    selection: c(0x43, 0x4C, 0x5E),
    hover: c(0x43, 0x4C, 0x5E),
    error: c(0xBF, 0x61, 0x6A),
    warning: c(0xEB, 0xCB, 0x8B),
    success: c(0xA3, 0xBE, 0x8C),
};

const NORD_CANVAS: CanvasColors = CanvasColors {
    background: c(0x2E, 0x34, 0x40),
    paper: c(0x3B, 0x42, 0x52),
    wire: c(0xA3, 0xBE, 0x8C),
    junction: c(0xA3, 0xBE, 0x8C),
    body: c(0x88, 0xC0, 0xD0),
    body_fill: c(0x3B, 0x42, 0x52),
    pin: c(0xEC, 0xEF, 0xF4),
    reference: c(0x88, 0xC0, 0xD0),
    value: c(0xB4, 0x8E, 0xAD),
    net_label: c(0x8F, 0xBC, 0xBB),
    global_label: c(0xEB, 0xCB, 0x8B),
    hier_label: c(0xD0, 0x87, 0x70),
    no_connect: c(0xBF, 0x61, 0x6A),
    power: c(0xBF, 0x61, 0x6A),
    selection: c(0xEC, 0xEF, 0xF4),
    bus: c(0x81, 0xA1, 0xC1),
    grid: c(0x4C, 0x56, 0x6A),
    cursor: c(0xEC, 0xEF, 0xF4),
};

// ---------------------------------------------------------------------------
// Public accessors
// ---------------------------------------------------------------------------

pub fn theme_tokens(id: ThemeId) -> ThemeTokens {
    match id {
        ThemeId::CatppuccinMocha => CATPPUCCIN_MOCHA_TOKENS,
        ThemeId::VsCodeDark => VSCODE_DARK_TOKENS,
        ThemeId::AltiumDark => ALTIUM_DARK_TOKENS,
        ThemeId::GitHubDark => GITHUB_DARK_TOKENS,
        ThemeId::SolarizedLight => SOLARIZED_LIGHT_TOKENS,
        ThemeId::Nord => NORD_TOKENS,
        ThemeId::Custom => ALTIUM_DARK_TOKENS, // caller must use CustomThemeFile directly
    }
}

pub fn canvas_colors(id: ThemeId) -> CanvasColors {
    match id {
        ThemeId::CatppuccinMocha => CATPPUCCIN_MOCHA_CANVAS,
        ThemeId::VsCodeDark => VSCODE_DARK_CANVAS,
        ThemeId::AltiumDark => ALTIUM_DARK_CANVAS,
        ThemeId::GitHubDark => GITHUB_DARK_CANVAS,
        ThemeId::SolarizedLight => SOLARIZED_LIGHT_CANVAS,
        ThemeId::Nord => NORD_CANVAS,
        ThemeId::Custom => ALTIUM_DARK_CANVAS, // caller must use CustomThemeFile directly
    }
}
