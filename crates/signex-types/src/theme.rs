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

/// The two Graphite themes are the only themes (decision 2026-11-07, see
/// `docs/DECISIONS.md`). An id saved by an older build is read through
/// [`ThemeId::from_stored`], so a stale preferences file still loads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeId {
    /// Graphite Light: warm paper-white chrome, copper accent. The default.
    GraphiteLight,
    /// Graphite Dark: the dark twin of [`ThemeId::GraphiteLight`].
    GraphiteDark,
}

/// Stored ids of the retired light themes.
const RETIRED_LIGHT_IDS: &[&str] = &["solarized_light"];

/// Stored ids of the retired dark themes, plus the retired `custom` id,
/// whose lightness is unknown.
const RETIRED_DARK_IDS: &[&str] = &[
    "signex",
    "alplab",
    "vs_code_dark",
    "catppuccin_mocha",
    "git_hub_dark",
    "nord",
    "custom",
];

impl ThemeId {
    /// Theme of a new install, i.e. when no theme preference is stored.
    pub const DEFAULT: ThemeId = ThemeId::GraphiteLight;

    /// Every theme, in the order the Preferences page shows them.
    pub const BUILTINS: &[ThemeId] = &[ThemeId::GraphiteLight, ThemeId::GraphiteDark];

    pub fn label(self) -> &'static str {
        match self {
            ThemeId::GraphiteLight => "Graphite Light",
            ThemeId::GraphiteDark => "Graphite Dark",
        }
    }

    /// True for the light theme.
    pub const fn is_light(self) -> bool {
        matches!(self, ThemeId::GraphiteLight)
    }

    /// The theme a light/dark toggle switches to.
    pub const fn light_dark_counterpart(self) -> ThemeId {
        match self {
            ThemeId::GraphiteLight => ThemeId::GraphiteDark,
            ThemeId::GraphiteDark => ThemeId::GraphiteLight,
        }
    }

    /// Read a stored theme id. A retired theme maps to the Graphite twin of
    /// its lightness; an unknown id (e.g. written by a newer build) reads as
    /// [`ThemeId::DEFAULT`]. Never fails.
    pub fn from_stored(stored: &str) -> ThemeId {
        match stored {
            "graphite_light" => ThemeId::GraphiteLight,
            "graphite_dark" => ThemeId::GraphiteDark,
            s if RETIRED_LIGHT_IDS.contains(&s) => ThemeId::GraphiteLight,
            s if RETIRED_DARK_IDS.contains(&s) => ThemeId::GraphiteDark,
            _ => ThemeId::DEFAULT,
        }
    }
}

/// Tolerant: any string is a theme id (see [`ThemeId::from_stored`]).
impl<'de> Deserialize<'de> for ThemeId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let stored = String::deserialize(deserializer)?;
        Ok(ThemeId::from_stored(&stored))
    }
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
    /// Chrome tokens beyond the 14 base ones; both Graphite themes set them all.
    #[serde(default, skip_serializing_if = "ExtendedTokens::is_empty")]
    pub extended: ExtendedTokens,
}

/// Optional chrome tokens beyond the 14 base ones. A theme leaves a field
/// `None` to keep the colour the app derives from the base tokens (see
/// `signex-app` `styles::tokens::resolve`); an all-`None` set serializes
/// without these keys.
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
    /// PCB ratsnest lines; `None` draws them in the selection colour.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ratsnest: Option<Color>,
    pub bus: Color,
    pub grid: Color,
    pub cursor: Color,
    /// PCB board edge (outline and the rim of openings); `None` uses the
    /// cursor colour. Set on the Graphite Dark canvas, the one board
    /// palette of both themes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub board_edge: Option<Color>,
    /// PCB keepout regions (magenta, as the Altium Keep-Out layer); `None`
    /// uses the DRC warning colour.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keepout: Option<Color>,
    /// The PCB layer colours the Layers panel and layer strip list; `None`
    /// uses the board palette's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub board_layers: Option<BoardLayerColors>,
    /// The PCB board area inside the outline (the Altium Board Area
    /// colour); `None` uses the canvas background.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub board_area: Option<Color>,
}

/// The colour each PCB layer family is listed with. The board edge and
/// keepouts are not here: they use [`CanvasColors::board_edge_color`] and
/// [`CanvasColors::keepout_color`], the colours they are drawn with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoardLayerColors {
    pub top_copper: Color,
    pub bottom_copper: Color,
    /// Inner copper and mechanical layers, cycled by layer number.
    pub layer_cycle: [Color; 6],
    pub margin: Color,
    pub silkscreen: Color,
    pub solder_mask: Color,
    pub paste: Color,
    /// Dimensions, Notes, Drill Drawing and the ECO layers.
    pub drawing: Color,
    /// Every other technical layer (Fab, Assembly, Courtyard, ...).
    pub other: Color,
    /// Through-hole pads and vias (the Altium Multi-Layer).
    #[serde(default = "graphite_multi_layer")]
    pub multi_layer: Color,
}

fn graphite_multi_layer() -> Color {
    GRAPHITE_BOARD_LAYERS.multi_layer
}

impl CanvasColors {
    /// The colours PCB layers are listed with.
    #[must_use]
    pub fn board_layer_colors(&self) -> BoardLayerColors {
        self.board_layers.unwrap_or(GRAPHITE_BOARD_LAYERS)
    }

    /// The colour of the PCB board edge.
    #[must_use]
    pub fn board_edge_color(&self) -> Color {
        self.board_edge.unwrap_or(self.cursor)
    }

    /// The colour of the PCB board area inside the outline.
    #[must_use]
    pub fn board_area_color(&self) -> Color {
        self.board_area.unwrap_or(self.background)
    }

    /// The colour of PCB keepout regions.
    #[must_use]
    pub fn keepout_color(&self) -> Color {
        self.keepout.unwrap_or(self.drc_warning)
    }

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
        // About 50%: the Light backdrop must visibly dim the app behind a
        // dialog or the palette (#2262, Caner 2026-11-12).
        scrim: Some(Color::new(0x15, 0x17, 0x1A, 128)),
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
    // Board views draw with the Dark canvas in both themes, so the board
    // colours live there only.
    board_edge: None,
    keepout: None,
    board_layers: None,
    board_area: None,
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
    board_edge: s(0xE8, 0xC5, 0x47),
    keepout: s(0xFF, 0x40, 0xFF),
    board_layers: Some(GRAPHITE_BOARD_LAYERS),
    // Altium default Board Area (black), to be checked by Caner.
    board_area: s(0x00, 0x00, 0x00),
};

/// Red top and blue bottom copper, as Altium lists them.
const GRAPHITE_BOARD_LAYERS: BoardLayerColors = BoardLayerColors {
    top_copper: c(0xD0, 0x40, 0x40),
    bottom_copper: c(0x38, 0x6C, 0xD0),
    layer_cycle: [
        c(0xC8, 0xA0, 0x2C),
        c(0x3C, 0xA8, 0x6A),
        c(0xA0, 0x58, 0xC8),
        c(0xC8, 0x6A, 0x2C),
        c(0x2C, 0xA8, 0xB8),
        c(0xB8, 0x4C, 0x8C),
    ],
    margin: c(0xE0, 0xD0, 0x40),
    silkscreen: c(0xE8, 0xE8, 0xE8),
    solder_mask: c(0x40, 0x9C, 0x58),
    paste: c(0xA8, 0xA8, 0xB8),
    drawing: c(0x88, 0x88, 0xA0),
    other: c(0x70, 0x90, 0xB0),
    // Altium default Multi-Layer (silver), to be checked by Caner.
    multi_layer: c(0xC0, 0xC0, 0xC0),
};

// ---------------------------------------------------------------------------
// Public accessors
// ---------------------------------------------------------------------------

pub fn theme_tokens(id: ThemeId) -> ThemeTokens {
    match id {
        ThemeId::GraphiteLight => GRAPHITE_LIGHT_TOKENS,
        ThemeId::GraphiteDark => GRAPHITE_DARK_TOKENS,
    }
}

pub fn canvas_colors(id: ThemeId) -> CanvasColors {
    match id {
        ThemeId::GraphiteLight => GRAPHITE_LIGHT_CANVAS,
        ThemeId::GraphiteDark => GRAPHITE_DARK_CANVAS,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(stored: &str) -> ThemeId {
        serde_json::from_value(serde_json::json!(stored)).expect("any string is a theme id")
    }

    #[test]
    fn the_graphite_light_sheet_is_near_white() {
        assert_eq!(
            canvas_colors(ThemeId::GraphiteLight).paper,
            c(0xFD, 0xFC, 0xFA)
        );
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
    /// kind, power and no-connect.
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
    fn the_ratsnest_keeps_its_graphite_grey() {
        for &id in ThemeId::BUILTINS {
            let r = canvas_colors(id).ratsnest_color();
            assert_eq!((r.r, r.g, r.b), (0x9E, 0xA3, 0xAE), "{id:?}");
        }
    }

    #[test]
    fn a_canvas_palette_without_a_ratsnest_draws_it_in_its_selection_colour() {
        let mut canvas = canvas_colors(ThemeId::GraphiteLight);
        canvas.selection = Color::new(1, 2, 3, 255);
        let mut json = serde_json::to_value(canvas).unwrap();
        json.as_object_mut().unwrap().remove("ratsnest");
        let old: CanvasColors = serde_json::from_value(json).unwrap();
        assert_eq!(old.ratsnest, None);
        assert_eq!(old.ratsnest_color(), Color::new(1, 2, 3, 255));
    }

    #[test]
    fn the_graphite_selection_is_green_not_the_copper_accent() {
        assert_eq!(
            canvas_colors(ThemeId::GraphiteLight).selection,
            Color::from_hex("#1F9D55")
        );
        assert_eq!(
            canvas_colors(ThemeId::GraphiteDark).selection,
            Color::from_hex("#4ADE80")
        );
        for &id in ThemeId::BUILTINS {
            let s = canvas_colors(id).selection;
            assert!(s.g > s.r && s.g > s.b, "{id:?}");
            assert_ne!(s, theme_tokens(id).accent, "{id:?}");
        }
    }

    #[test]
    fn graphite_is_the_default_and_the_only_pair_of_themes() {
        assert_eq!(ThemeId::DEFAULT, ThemeId::GraphiteLight);
        assert_eq!(
            ThemeId::BUILTINS,
            &[ThemeId::GraphiteLight, ThemeId::GraphiteDark]
        );
        assert_eq!(ThemeId::GraphiteLight.label(), "Graphite Light");
        assert_eq!(ThemeId::GraphiteDark.label(), "Graphite Dark");
        assert!(ThemeId::GraphiteLight.is_light());
        assert!(!ThemeId::GraphiteDark.is_light());
    }

    #[test]
    fn the_light_dark_counterpart_flips_lightness() {
        let flip = ThemeId::light_dark_counterpart;
        assert_eq!(flip(ThemeId::GraphiteLight), ThemeId::GraphiteDark);
        assert_eq!(flip(ThemeId::GraphiteDark), ThemeId::GraphiteLight);
    }

    #[test]
    fn the_graphite_canvas_is_one_sheet_and_every_theme_sets_extended_tokens() {
        for &id in ThemeId::BUILTINS {
            let canvas = canvas_colors(id);
            assert_eq!(canvas.background, canvas.paper, "{id:?}");
            assert!(!theme_tokens(id).extended.is_empty(), "{id:?}");
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
    fn the_graphite_ids_round_trip_through_their_stored_names() {
        for (&id, stored) in ThemeId::BUILTINS
            .iter()
            .zip(["graphite_light", "graphite_dark"])
        {
            assert_eq!(serde_json::to_value(id).unwrap(), serde_json::json!(stored));
            assert_eq!(read(stored), id);
        }
    }

    #[test]
    fn the_retired_light_theme_migrates_to_graphite_light() {
        assert_eq!(read("solarized_light"), ThemeId::GraphiteLight);
    }

    #[test]
    fn each_retired_dark_theme_migrates_to_graphite_dark() {
        for stored in [
            "signex",
            "alplab",
            "vs_code_dark",
            "catppuccin_mocha",
            "git_hub_dark",
            "nord",
        ] {
            assert_eq!(read(stored), ThemeId::GraphiteDark, "{stored}");
        }
    }

    #[test]
    fn the_retired_custom_id_migrates_to_graphite_dark() {
        assert_eq!(read("custom"), ThemeId::GraphiteDark);
    }

    #[test]
    fn an_unknown_stored_id_reads_as_the_default_without_failing() {
        for stored in ["", "monokai", "Graphite Light", "GRAPHITE_DARK"] {
            assert_eq!(read(stored), ThemeId::DEFAULT, "{stored:?}");
        }
    }
}
