//! Built-in themes for the Signex EDA application.
//!
//! # The one central theme source
//!
//! This module is the **single source of truth** for every colour in Signex.
//! [`ThemeId`] selects a theme; [`ThemeTokens`] holds the UI-chrome palette
//! (backgrounds, text, accent, panels, status colours) and [`CanvasColors`]
//! holds the drawing palette (wire, junction, body, pad, silk, …). Nothing
//! else defines colours — every surface *consumes* these tokens:
//!
//! - `signex-widgets` (`theme_ext.rs`) bridges tokens into per-widget iced
//!   [`Style`](https://docs.rs/iced/0.14/iced/widget/container/struct.Style.html)
//!   values — the iced 0.14 Catalog route. iced's own `Theme`/`Palette` is
//!   therefore just *one consumer* of these tokens, not a second source.
//! - `signex-renderer` / the `signex-gfx` wgpu path read the same tokens for
//!   the GPU/canvas draw colours.
//! - `signex-output` reads them for SVG / PDF export.
//! - `signex-app` and `chrome-catalog` read them for the shell UI.
//!
//! This crate has **zero `iced` / `wgpu` dependency** on purpose (ADR: separate
//! types from rendering): because the tokens live in the domain layer, the
//! non-iced surfaces (GPU shader, export) can share them, so the wgpu PCB
//! renderer and the SVG/PDF export stay in lock-step with the iced UI. A
//! re-skin is one edit to [`ThemeId`]'s token tables; every surface follows.

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

    /// Parse exactly `#RRGGBB` or `#RRGGBBAA`; `None` for anything else.
    /// Unlike [`Color::from_hex`] it never indexes past a short string and
    /// never reads a bad digit as zero, so it is safe on file input.
    pub fn parse_hex(hex: &str) -> Option<Self> {
        let digits = hex.strip_prefix('#')?;
        if !matches!(digits.len(), 6 | 8) || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let channel = |at: usize| u8::from_str_radix(&digits[at..at + 2], 16).ok();
        Some(Self {
            r: channel(0)?,
            g: channel(2)?,
            b: channel(4)?,
            a: if digits.len() == 8 { channel(6)? } else { 255 },
        })
    }

    /// `#RRGGBBAA`, lower-case; the inverse of [`Color::parse_hex`].
    pub fn to_hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}{:02x}", self.r, self.g, self.b, self.a)
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
    Signex,
    /// Alp Lab brand theme — clones the Signex chrome palette and swaps
    /// the accent (and any visually-linked iconography) to cyan
    /// `#0891b2`. Used for co-branded or white-labelled builds.
    Alplab,
    GitHubDark,
    SolarizedLight,
    Nord,
    /// Graphite Light: warm paper-white chrome, copper accent. The default.
    GraphiteLight,
    /// Graphite Dark: the dark twin of [`ThemeId::GraphiteLight`].
    GraphiteDark,
    /// User-defined custom theme (data stored externally in CustomThemeFile).
    Custom,
}

impl ThemeId {
    /// Theme of a new install, i.e. when no theme preference is stored.
    pub const DEFAULT: ThemeId = ThemeId::GraphiteLight;

    /// All built-in themes (excludes Custom).
    pub const BUILTINS: &[ThemeId] = &[
        ThemeId::GraphiteLight,
        ThemeId::GraphiteDark,
        ThemeId::Signex,
        ThemeId::Alplab,
        ThemeId::VsCodeDark,
        ThemeId::CatppuccinMocha,
        ThemeId::GitHubDark,
        ThemeId::SolarizedLight,
        ThemeId::Nord,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ThemeId::CatppuccinMocha => "Catppuccin Mocha",
            ThemeId::VsCodeDark => "VS Code Dark",
            ThemeId::Signex => "Signex",
            ThemeId::Alplab => "Alp Lab",
            ThemeId::GitHubDark => "GitHub Dark",
            ThemeId::SolarizedLight => "Solarized Light",
            ThemeId::Nord => "Nord",
            ThemeId::GraphiteLight => "Graphite Light",
            ThemeId::GraphiteDark => "Graphite Dark",
            ThemeId::Custom => "Custom",
        }
    }

    /// The theme a light/dark toggle switches to: the Graphite twin of the
    /// opposite lightness. `Custom` has no known lightness and goes to
    /// Graphite Light.
    pub const fn light_dark_counterpart(self) -> ThemeId {
        match self {
            ThemeId::GraphiteLight | ThemeId::SolarizedLight => ThemeId::GraphiteDark,
            ThemeId::GraphiteDark
            | ThemeId::CatppuccinMocha
            | ThemeId::VsCodeDark
            | ThemeId::Signex
            | ThemeId::Alplab
            | ThemeId::GitHubDark
            | ThemeId::Nord
            | ThemeId::Custom => ThemeId::GraphiteLight,
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
    /// Tokens only the Graphite themes define; `None` everywhere else.
    #[serde(default, skip_serializing_if = "ExtendedTokens::is_empty")]
    pub extended: ExtendedTokens,
}

/// Optional chrome tokens beyond the 14 base ones. A theme leaves a field
/// `None` to keep the colour the app derives from the base tokens (see
/// `signex-app` `styles::tokens::resolve`), so old themes and old custom
/// theme files look the same and serialize without these keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ExtendedTokens {
    /// Floating surfaces: menus, dialogs, the Active Bar.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raised: Option<Color>,
    /// Text-input fill.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field: Option<Color>,
    /// Recessed fill: segmented-control track, table headers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sunken: Option<Color>,
    /// Hairline between rows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_soft: Option<Color>,
    /// Stronger rule: sheet frame, field outline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_strong: Option<Color>,
    /// Third text step.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ink3: Option<Color>,
    /// Fourth, faintest text step.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ink4: Option<Color>,
    /// Accent used as text colour.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accent_text: Option<Color>,
    /// Accent-tinted fill.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accent_soft: Option<Color>,
    /// Text on [`ExtendedTokens::accent_soft`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accent_soft_ink: Option<Color>,
    /// Primary button fill.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary: Option<Color>,
    /// Text on the primary button.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary_ink: Option<Color>,
    /// Informational colour.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub info: Option<Color>,
    /// Soft fills behind status text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ok_soft: Option<Color>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warn_soft: Option<Color>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub err_soft: Option<Color>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub info_soft: Option<Color>,
    /// Drop-shadow colour; the alpha channel is the shadow strength.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadow: Option<Color>,
    /// Modal scrim colour; the alpha channel is its strength.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scrim: Option<Color>,
}

impl ExtendedTokens {
    /// Every field `None`: the base tokens decide.
    pub const NONE: Self = Self {
        raised: None,
        field: None,
        sunken: None,
        line_soft: None,
        line_strong: None,
        ink3: None,
        ink4: None,
        accent_text: None,
        accent_soft: None,
        accent_soft_ink: None,
        primary: None,
        primary_ink: None,
        info: None,
        ok_soft: None,
        warn_soft: None,
        err_soft: None,
        info_soft: None,
        shadow: None,
        scrim: None,
    };

    /// True when no field is set (the serde skip test).
    pub fn is_empty(&self) -> bool {
        *self == Self::NONE
    }
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
    /// ERC violation marker, error severity.
    pub erc_error: Color,
    /// ERC violation marker, warning severity.
    pub erc_warning: Color,
    /// DRC violation marker, error severity.
    pub drc_error: Color,
    /// DRC violation marker, warning severity.
    pub drc_warning: Color,
    pub selection: Color,
    /// PCB ratsnest lines; `None` (a custom theme file from before this
    /// field) draws them in the selection colour, as they always were.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ratsnest: Option<Color>,
    pub bus: Color,
    pub grid: Color,
    pub cursor: Color,
}

impl CanvasColors {
    /// The colour of PCB ratsnest lines.
    #[must_use]
    pub fn ratsnest_color(&self) -> Color {
        self.ratsnest.unwrap_or(self.selection)
    }
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
    extended: ExtendedTokens::NONE,
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
    erc_error: c(0xF3, 0x8B, 0xA8),
    erc_warning: c(0xF3, 0x8B, 0xA8),
    drc_error: c(0xF3, 0x8B, 0xA8),
    drc_warning: c(0xF3, 0x8B, 0xA8),
    selection: c(0xFF, 0xEE, 0x00), // Bright yellow: apart from the pin, wire and label colours
    ratsnest: Some(c(0xF5, 0xE0, 0xDC)),
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
    extended: ExtendedTokens::NONE,
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
    erc_error: c(0xF4, 0x44, 0x47),
    erc_warning: c(0xF4, 0x44, 0x47),
    drc_error: c(0xF4, 0x44, 0x47),
    drc_warning: c(0xF4, 0x44, 0x47),
    selection: c(0xFF, 0xFF, 0x00), // Bright yellow: apart from the pin, wire and label colours
    ratsnest: Some(c(0xFF, 0xFF, 0xFF)),
    bus: c(0x9C, 0xDC, 0xFE),
    grid: c(0x3C, 0x3C, 0x3C),
    cursor: c(0xFF, 0xFF, 0xFF),
};

// ===== Signex =====

const SIGNEX_TOKENS: ThemeTokens = ThemeTokens {
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
    extended: ExtendedTokens::NONE,
};

const SIGNEX_CANVAS: CanvasColors = CanvasColors {
    // Classic schematic palette — near-white sheet, dark elements. The workspace
    // OUTSIDE the sheet follows the chrome (`SIGNEX_TOKENS.bg`) so the canvas
    // backdrop reads as part of the dark UI instead of a detached gray slab.
    background: c(0x2D, 0x2D, 0x30), // Chrome-aligned workspace outside sheet
    paper: c(0xFC, 0xFC, 0xFC),      // Near-white sheet (252,252,252)
    wire: c(0x00, 0x00, 0x80),       // Navy blue wires
    junction: c(0x00, 0x00, 0x80),   // Navy blue junction dots
    body: c(0x80, 0x60, 0x00),       // Brown component body border
    body_fill: c(0xFF, 0xFF, 0x80),  // Light yellow component fill
    pin: c(0x00, 0x00, 0x00),        // Black pin lines
    reference: c(0x80, 0x00, 0x00),  // Dark red designator (R1, C1…)
    value: c(0x00, 0x00, 0x80),      // Navy blue value/comment text
    net_label: c(0x80, 0x00, 0x00),  // Dark red / burgundy net labels
    global_label: c(0x80, 0x00, 0x00), // Altium maroon port outline/text
    hier_label: c(0x80, 0x00, 0x00), // Altium maroon hierarchical port outline/text
    no_connect: c(0x80, 0x00, 0x00), // Dark red X marks
    power: c(0x80, 0x00, 0x00),      // Dark red power ports (VCC/GND)
    erc_error: c(0x80, 0x00, 0x00),
    erc_warning: c(0x80, 0x00, 0x00),
    drc_error: c(0x80, 0x00, 0x00),
    drc_warning: c(0x80, 0x00, 0x00),
    selection: c(0x00, 0xA0, 0x00), // Altium-style green; reads on the white sheet and on dark chrome
    ratsnest: Some(c(0x00, 0x78, 0xD4)),
    bus: c(0x00, 0x00, 0x80),    // Navy blue bus (rendered thicker)
    grid: c(0xC0, 0xC0, 0xC0),   // Light gray grid dots/lines
    cursor: c(0x00, 0x00, 0x00), // Black cursor crosshair
};

// ===== Alp Lab =====
//
// Clone of the Signex chrome tokens with the accent swapped to the Alp
// Lab brand cyan. The icon tree in `assets/icons/alplab/` pre-tints the
// SVG fills to this same cyan; keeping the accent aligned so dropdown
// chevrons, focus rings and button highlights read as one palette.
// Canvas colours reuse SIGNEX_CANVAS for now — the schematic sheet
// keeps the near-white sheet regardless of chrome theme.

const ALPLAB_TOKENS: ThemeTokens = ThemeTokens {
    bg: c(0x2D, 0x2D, 0x30),
    paper: c(0x1E, 0x1E, 0x1E),
    text: c(0xDC, 0xDC, 0xDC),
    text_secondary: c(0x9B, 0x9B, 0x9B),
    accent: c(0x08, 0x91, 0xB2),
    border: c(0x3F, 0x3F, 0x46),
    panel_bg: c(0x25, 0x25, 0x28),
    toolbar_bg: c(0x2D, 0x2D, 0x30),
    statusbar_bg: c(0x25, 0x25, 0x28),
    selection: c(0x51, 0x51, 0x55),
    hover: c(0x3F, 0x3F, 0x46),
    error: c(0xF4, 0x4E, 0x4E),
    warning: c(0xE8, 0x91, 0x2D),
    success: c(0x57, 0xA6, 0x4A),
    extended: ExtendedTokens::NONE,
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
    extended: ExtendedTokens::NONE,
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
    erc_error: c(0xFF, 0x7B, 0x72),
    erc_warning: c(0xFF, 0x7B, 0x72),
    drc_error: c(0xFF, 0x7B, 0x72),
    drc_warning: c(0xFF, 0x7B, 0x72),
    selection: c(0xFF, 0xDD, 0x00), // Bright yellow: apart from the pin, wire and label colours
    ratsnest: Some(c(0xFF, 0xFF, 0xFF)),
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
    extended: ExtendedTokens::NONE,
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
    erc_error: c(0xDC, 0x32, 0x2F),
    erc_warning: c(0xDC, 0x32, 0x2F),
    drc_error: c(0xDC, 0x32, 0x2F),
    drc_warning: c(0xDC, 0x32, 0x2F),
    selection: c(0x00, 0x2B, 0x36),
    ratsnest: Some(c(0x00, 0x2B, 0x36)),
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
    extended: ExtendedTokens::NONE,
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
    erc_error: c(0xBF, 0x61, 0x6A),
    erc_warning: c(0xBF, 0x61, 0x6A),
    drc_error: c(0xBF, 0x61, 0x6A),
    drc_warning: c(0xBF, 0x61, 0x6A),
    selection: c(0xFF, 0xCC, 0x00), // Bright amber: apart from the pin, wire and label colours
    ratsnest: Some(c(0xEC, 0xEF, 0xF4)),
    bus: c(0x81, 0xA1, 0xC1),
    grid: c(0x4C, 0x56, 0x6A),
    cursor: c(0xEC, 0xEF, 0xF4),
};

// ===== Graphite =====
//
// Warm-neutral chrome with a copper accent. The canvas is one sheet: the
// workspace background equals the paper. Per-net-class wire colours are not
// part of this palette yet, so wires, bodies and pins share the symbol ink.

const fn s(r: u8, g: u8, b: u8) -> Option<Color> {
    Some(c(r, g, b))
}

const GRAPHITE_LIGHT_TOKENS: ThemeTokens = ThemeTokens {
    bg: c(0xF6, 0xF5, 0xF1),
    paper: c(0xFD, 0xFC, 0xFA),
    text: c(0x15, 0x17, 0x1A),
    text_secondary: c(0x5B, 0x5E, 0x66),
    accent: c(0xC2, 0x55, 0x2D),
    border: c(0xE4, 0xE2, 0xDC),
    panel_bg: c(0xFF, 0xFF, 0xFF),
    toolbar_bg: c(0xFB, 0xFA, 0xF7),
    statusbar_bg: c(0xFB, 0xFA, 0xF7),
    selection: c(0xEF, 0xEC, 0xE6),
    hover: c(0xF1, 0xEF, 0xEA),
    error: c(0xA8, 0x30, 0x1F),
    warning: c(0x8A, 0x5A, 0x10),
    success: c(0x24, 0x61, 0x3A),
    extended: ExtendedTokens {
        raised: s(0xFF, 0xFF, 0xFF),
        field: s(0xFF, 0xFF, 0xFF),
        sunken: s(0xF1, 0xEF, 0xEA),
        line_soft: s(0xEF, 0xED, 0xE8),
        line_strong: s(0xDC, 0xD9, 0xD1),
        ink3: s(0x5B, 0x5E, 0x66),
        ink4: s(0x6B, 0x6E, 0x75),
        accent_text: s(0xA8, 0x45, 0x2A),
        accent_soft: s(0xF3, 0xE6, 0xDF),
        accent_soft_ink: s(0x7E, 0x32, 0x1D),
        primary: s(0x15, 0x17, 0x1A),
        primary_ink: s(0xFF, 0xFF, 0xFF),
        info: s(0x1F, 0x45, 0xAE),
        ok_soft: s(0xEA, 0xF3, 0xEC),
        warn_soft: s(0xFD, 0xF7, 0xEA),
        err_soft: s(0xFB, 0xED, 0xEA),
        info_soft: s(0xEA, 0xF0, 0xFD),
        shadow: Some(Color::new(0x15, 0x17, 0x1A, 26)),
        scrim: Some(Color::new(0x15, 0x17, 0x1A, 87)),
    },
};

const GRAPHITE_LIGHT_CANVAS: CanvasColors = CanvasColors {
    background: c(0xFD, 0xFC, 0xFA),
    paper: c(0xFD, 0xFC, 0xFA),
    wire: c(0x2B, 0x2D, 0x33),
    junction: c(0x2B, 0x2D, 0x33),
    body: c(0x2B, 0x2D, 0x33),
    body_fill: c(0xFF, 0xFF, 0xFF),
    pin: c(0x2B, 0x2D, 0x33),
    reference: c(0x15, 0x17, 0x1A),
    value: c(0x5B, 0x5E, 0x66),
    net_label: c(0x2D, 0x5B, 0xD7),
    global_label: c(0x2D, 0x5B, 0xD7),
    hier_label: c(0x7B, 0x4B, 0xB8),
    no_connect: c(0xA8, 0x30, 0x1F),
    power: c(0xC0, 0x39, 0x2B),
    erc_error: c(0xA8, 0x30, 0x1F),
    erc_warning: c(0xC0, 0x39, 0x2B),
    drc_error: c(0xA8, 0x30, 0x1F),
    drc_warning: c(0xC0, 0x39, 0x2B),
    // Green, not the copper accent: it must stay apart from the power red.
    selection: c(0x1F, 0x9D, 0x55),
    ratsnest: s(0x9E, 0xA3, 0xAE),
    bus: c(0x5B, 0x5E, 0x66),
    grid: c(0xE2, 0xDF, 0xD8),
    cursor: c(0x15, 0x17, 0x1A),
};

const GRAPHITE_DARK_TOKENS: ThemeTokens = ThemeTokens {
    bg: c(0x11, 0x12, 0x14),
    paper: c(0x14, 0x15, 0x18),
    text: c(0xEC, 0xEA, 0xE5),
    text_secondary: c(0xA3, 0xA5, 0xAB),
    accent: c(0xC2, 0x55, 0x2D),
    border: c(0x2A, 0x2C, 0x31),
    panel_bg: c(0x17, 0x18, 0x1B),
    toolbar_bg: c(0x17, 0x18, 0x1B),
    statusbar_bg: c(0x17, 0x18, 0x1B),
    selection: c(0x2A, 0x2C, 0x32),
    hover: c(0x22, 0x24, 0x29),
    error: c(0xF2, 0xA0, 0x93),
    warning: c(0xE8, 0xC9, 0x7A),
    success: c(0x7F, 0xD1, 0xA0),
    extended: ExtendedTokens {
        raised: s(0x1D, 0x1F, 0x23),
        field: s(0x1D, 0x1F, 0x23),
        sunken: s(0x22, 0x24, 0x29),
        line_soft: s(0x23, 0x25, 0x29),
        line_strong: s(0x34, 0x36, 0x3C),
        ink3: s(0xA3, 0xA5, 0xAB),
        ink4: s(0x8B, 0x8D, 0x93),
        accent_text: s(0xE0, 0x86, 0x5C),
        accent_soft: s(0x3A, 0x24, 0x18),
        accent_soft_ink: s(0xF0, 0xB0, 0x8E),
        primary: s(0xEC, 0xEA, 0xE5),
        primary_ink: s(0x11, 0x12, 0x14),
        info: s(0xA9, 0xC2, 0xFF),
        ok_soft: s(0x15, 0x29, 0x1D),
        warn_soft: s(0x2A, 0x22, 0x10),
        err_soft: s(0x3A, 0x1D, 0x19),
        info_soft: s(0x16, 0x22, 0x3F),
        shadow: Some(Color::new(0, 0, 0, 115)),
        scrim: Some(Color::new(0, 0, 0, 140)),
    },
};

const GRAPHITE_DARK_CANVAS: CanvasColors = CanvasColors {
    background: c(0x14, 0x15, 0x18),
    paper: c(0x14, 0x15, 0x18),
    wire: c(0xC9, 0xCB, 0xD1),
    junction: c(0xC9, 0xCB, 0xD1),
    body: c(0xC9, 0xCB, 0xD1),
    body_fill: c(0x1B, 0x1D, 0x21),
    pin: c(0xC9, 0xCB, 0xD1),
    reference: c(0xEC, 0xEA, 0xE5),
    value: c(0xA3, 0xA5, 0xAB),
    net_label: c(0x6E, 0x9B, 0xFF),
    global_label: c(0x6E, 0x9B, 0xFF),
    hier_label: c(0xA7, 0x8B, 0xDB),
    no_connect: c(0xF2, 0xA0, 0x93),
    power: c(0xE0, 0x6C, 0x5C),
    erc_error: c(0xF2, 0xA0, 0x93),
    erc_warning: c(0xE0, 0x6C, 0x5C),
    drc_error: c(0xF2, 0xA0, 0x93),
    drc_warning: c(0xE0, 0x6C, 0x5C),
    selection: c(0x4A, 0xDE, 0x80),
    ratsnest: s(0x9E, 0xA3, 0xAE),
    bus: c(0xA3, 0xA5, 0xAB),
    grid: c(0x26, 0x28, 0x2D),
    cursor: c(0xEC, 0xEA, 0xE5),
};

// ---------------------------------------------------------------------------
// Public accessors
// ---------------------------------------------------------------------------

pub fn theme_tokens(id: ThemeId) -> ThemeTokens {
    match id {
        ThemeId::CatppuccinMocha => CATPPUCCIN_MOCHA_TOKENS,
        ThemeId::VsCodeDark => VSCODE_DARK_TOKENS,
        ThemeId::Signex => SIGNEX_TOKENS,
        ThemeId::Alplab => ALPLAB_TOKENS,
        ThemeId::GitHubDark => GITHUB_DARK_TOKENS,
        ThemeId::SolarizedLight => SOLARIZED_LIGHT_TOKENS,
        ThemeId::Nord => NORD_TOKENS,
        ThemeId::GraphiteLight => GRAPHITE_LIGHT_TOKENS,
        ThemeId::GraphiteDark => GRAPHITE_DARK_TOKENS,
        ThemeId::Custom => SIGNEX_TOKENS, // caller must use CustomThemeFile directly
    }
}

pub fn canvas_colors(id: ThemeId) -> CanvasColors {
    match id {
        ThemeId::CatppuccinMocha => CATPPUCCIN_MOCHA_CANVAS,
        ThemeId::VsCodeDark => VSCODE_DARK_CANVAS,
        ThemeId::Signex => SIGNEX_CANVAS,
        // Alp Lab reuses the Signex schematic palette; only
        // the chrome accent differs.
        ThemeId::Alplab => SIGNEX_CANVAS,
        ThemeId::GitHubDark => GITHUB_DARK_CANVAS,
        ThemeId::SolarizedLight => SOLARIZED_LIGHT_CANVAS,
        ThemeId::Nord => NORD_CANVAS,
        ThemeId::GraphiteLight => GRAPHITE_LIGHT_CANVAS,
        ThemeId::GraphiteDark => GRAPHITE_DARK_CANVAS,
        ThemeId::Custom => SIGNEX_CANVAS, // caller must use CustomThemeFile directly
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_signex_sheet_is_near_white() {
        for id in [ThemeId::Signex, ThemeId::Alplab] {
            assert_eq!(canvas_colors(id).paper, c(0xFC, 0xFC, 0xFC), "{id:?}");
        }
    }

    #[test]
    fn every_builtin_defines_marker_slots_matching_todays_look() {
        for &id in ThemeId::BUILTINS {
            let c = canvas_colors(id);
            assert_eq!(c.erc_error, c.no_connect, "{id:?}");
            assert_eq!(c.erc_warning, c.power, "{id:?}");
            assert_eq!(c.drc_error, c.erc_error, "{id:?}");
            assert_eq!(c.drc_warning, c.erc_warning, "{id:?}");
            assert_eq!(c.erc_error.a, 0xFF, "{id:?}");
        }
    }

    /// The selection outline must stand out from every item colour it can
    /// sit on or beside: wires, buses, junctions, bodies, pins, every label
    /// kind, power and no-connect (Nord and the VS Code / GitHub darks once
    /// matched their pin colour, and two themes a label colour).
    #[test]
    fn selection_is_distinct_from_every_item_colour_in_every_builtin() {
        let apart = |a: Color, b: Color| {
            [a.r.abs_diff(b.r), a.g.abs_diff(b.g), a.b.abs_diff(b.b)]
                .into_iter()
                .map(u32::from)
                .sum::<u32>()
        };
        for &id in ThemeId::BUILTINS {
            let c = canvas_colors(id);
            for (name, item) in [
                ("wire", c.wire),
                ("bus", c.bus),
                ("junction", c.junction),
                ("body", c.body),
                ("pin", c.pin),
                ("net_label", c.net_label),
                ("global_label", c.global_label),
                ("hier_label", c.hier_label),
                ("power", c.power),
                ("no_connect", c.no_connect),
            ] {
                let distance = apart(c.selection, item);
                assert!(
                    distance >= 96,
                    "{id:?}: selection is {distance} from {name}"
                );
            }
        }
    }

    #[test]
    fn the_ratsnest_keeps_the_colour_selection_used_to_have() {
        let old = |id| match id {
            ThemeId::CatppuccinMocha => (0xF5, 0xE0, 0xDC),
            ThemeId::VsCodeDark | ThemeId::GitHubDark => (0xFF, 0xFF, 0xFF),
            ThemeId::Signex | ThemeId::Alplab | ThemeId::Custom => (0x00, 0x78, 0xD4),
            ThemeId::SolarizedLight => (0x00, 0x2B, 0x36),
            ThemeId::Nord => (0xEC, 0xEF, 0xF4),
            ThemeId::GraphiteLight | ThemeId::GraphiteDark => (0x9E, 0xA3, 0xAE),
        };
        for &id in ThemeId::BUILTINS {
            let r = canvas_colors(id).ratsnest_color();
            assert_eq!((r.r, r.g, r.b), old(id), "{id:?}");
        }
    }

    #[test]
    fn an_old_custom_theme_file_draws_its_ratsnest_in_its_selection_colour() {
        let mut canvas = canvas_colors(ThemeId::Signex);
        canvas.selection = Color::new(1, 2, 3, 255);
        let mut json = serde_json::to_value(canvas).unwrap();
        json.as_object_mut().unwrap().remove("ratsnest");
        let old: CanvasColors = serde_json::from_value(json).unwrap();
        assert_eq!(old.ratsnest, None);
        assert_eq!(old.ratsnest_color(), Color::new(1, 2, 3, 255));
    }

    #[test]
    fn the_signex_and_alplab_selection_is_green() {
        for id in [ThemeId::Signex, ThemeId::Alplab] {
            let s = canvas_colors(id).selection;
            assert!(
                s.g > s.r.saturating_add(0x60) && s.g > s.b.saturating_add(0x60),
                "{id:?}"
            );
        }
    }

    #[test]
    fn custom_theme_round_trips_marker_slots() {
        let mut canvas = canvas_colors(ThemeId::Signex);
        canvas.erc_error = Color::new(1, 2, 3, 255);
        canvas.drc_warning = Color::new(9, 8, 7, 255);
        let file = CustomThemeFile {
            name: "t".into(),
            tokens: theme_tokens(ThemeId::Signex),
            canvas,
        };
        let json = serde_json::to_string(&file).unwrap();
        for key in ["erc_error", "erc_warning", "drc_error", "drc_warning"] {
            assert!(json.contains(key), "{key} missing from export");
        }
        let back: CustomThemeFile = serde_json::from_str(&json).unwrap();
        assert_eq!(back.canvas, canvas);
    }

    #[test]
    fn graphite_is_the_default_and_both_twins_are_builtin() {
        assert_eq!(ThemeId::DEFAULT, ThemeId::GraphiteLight);
        for id in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
            assert!(ThemeId::BUILTINS.contains(&id), "{id:?}");
        }
        assert_eq!(ThemeId::GraphiteLight.label(), "Graphite Light");
        assert_eq!(ThemeId::GraphiteDark.label(), "Graphite Dark");
    }

    #[test]
    fn the_light_dark_counterpart_flips_lightness() {
        let flip = ThemeId::light_dark_counterpart;
        assert_eq!(flip(ThemeId::GraphiteLight), ThemeId::GraphiteDark);
        assert_eq!(flip(ThemeId::GraphiteDark), ThemeId::GraphiteLight);
        assert_eq!(flip(ThemeId::Signex), ThemeId::GraphiteLight);
        assert_eq!(flip(ThemeId::SolarizedLight), ThemeId::GraphiteDark);
    }

    #[test]
    fn the_graphite_canvas_is_one_sheet_and_only_graphite_sets_extended_tokens() {
        for id in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
            let canvas = canvas_colors(id);
            assert_eq!(canvas.background, canvas.paper, "{id:?}");
        }
        for &id in ThemeId::BUILTINS {
            let graphite = matches!(id, ThemeId::GraphiteLight | ThemeId::GraphiteDark);
            assert_eq!(theme_tokens(id).extended.is_empty(), !graphite, "{id:?}");
        }
    }

    #[test]
    fn every_graphite_extended_token_is_set() {
        for id in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
            let json = serde_json::to_value(theme_tokens(id).extended).unwrap();
            let set = json.as_object().unwrap().len();
            let all = serde_json::to_value(ExtendedTokens {
                raised: Some(c(0, 0, 0)),
                field: Some(c(0, 0, 0)),
                sunken: Some(c(0, 0, 0)),
                line_soft: Some(c(0, 0, 0)),
                line_strong: Some(c(0, 0, 0)),
                ink3: Some(c(0, 0, 0)),
                ink4: Some(c(0, 0, 0)),
                accent_text: Some(c(0, 0, 0)),
                accent_soft: Some(c(0, 0, 0)),
                accent_soft_ink: Some(c(0, 0, 0)),
                primary: Some(c(0, 0, 0)),
                primary_ink: Some(c(0, 0, 0)),
                info: Some(c(0, 0, 0)),
                ok_soft: Some(c(0, 0, 0)),
                warn_soft: Some(c(0, 0, 0)),
                err_soft: Some(c(0, 0, 0)),
                info_soft: Some(c(0, 0, 0)),
                shadow: Some(c(0, 0, 0)),
                scrim: Some(c(0, 0, 0)),
            })
            .unwrap()
            .as_object()
            .unwrap()
            .len();
            assert_eq!(set, all, "{id:?}");
        }
    }

    #[test]
    fn graphite_tokens_match_the_spec_hex() {
        let light = theme_tokens(ThemeId::GraphiteLight);
        assert_eq!(light.accent, Color::from_hex("#C2552D"));
        assert_eq!(light.panel_bg, Color::from_hex("#FFFFFF"));
        assert_eq!(light.extended.accent_soft, Some(Color::from_hex("#F3E6DF")));
        let light_canvas = canvas_colors(ThemeId::GraphiteLight);
        assert_eq!(light_canvas.grid, Color::from_hex("#E2DFD8"));
        let dark = theme_tokens(ThemeId::GraphiteDark);
        assert_eq!(dark.bg, Color::from_hex("#111214"));
        assert_eq!(dark.extended.primary_ink, Some(Color::from_hex("#111214")));
        let dark_canvas = canvas_colors(ThemeId::GraphiteDark);
        assert_eq!(dark_canvas.wire, Color::from_hex("#C9CBD1"));
    }

    #[test]
    fn a_custom_theme_file_without_extended_tokens_loads_and_saves_without_them() {
        let file = CustomThemeFile {
            name: "t".into(),
            tokens: theme_tokens(ThemeId::Signex),
            canvas: canvas_colors(ThemeId::Signex),
        };
        let json = serde_json::to_value(&file).unwrap();
        assert!(json["tokens"].get("extended").is_none());
        let back: CustomThemeFile = serde_json::from_value(json.clone()).unwrap();
        assert!(back.tokens.extended.is_empty());
        assert_eq!(serde_json::to_value(&back).unwrap(), json);
    }

    #[test]
    fn a_graphite_custom_theme_round_trips_its_extended_tokens() {
        let file = CustomThemeFile {
            name: "g".into(),
            tokens: theme_tokens(ThemeId::GraphiteDark),
            canvas: canvas_colors(ThemeId::GraphiteDark),
        };
        let back: CustomThemeFile =
            serde_json::from_str(&serde_json::to_string(&file).unwrap()).unwrap();
        assert_eq!(back.tokens, file.tokens);
    }
}
