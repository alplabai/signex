//! Local render configuration for signex-app.
//!
//! This module replaces the old renderer runtime config surface so app-side
//! preferences stay independent from removed legacy code.

use std::sync::{OnceLock, RwLock};

pub const IOSEVKA: iced::Font = iced::Font::with_name("Iosevka");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PowerPortStyle {
    Standard,
    #[default]
    Altium,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LabelStyle {
    #[default]
    Standard,
    Altium,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MultisheetStyle {
    #[default]
    Standard,
    Altium,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GridStyle {
    #[default]
    Dots,
    Lines,
    SmallCrosses,
}

impl GridStyle {
    pub const ALL: &'static [GridStyle] =
        &[GridStyle::Dots, GridStyle::Lines, GridStyle::SmallCrosses];
}

impl std::fmt::Display for PowerPortStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PowerPortStyle::Standard => write!(f, "Standard"),
            PowerPortStyle::Altium => write!(f, "Altium"),
        }
    }
}

impl std::fmt::Display for LabelStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LabelStyle::Standard => write!(f, "Standard"),
            LabelStyle::Altium => write!(f, "Altium"),
        }
    }
}

impl std::fmt::Display for MultisheetStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MultisheetStyle::Standard => write!(f, "Standard"),
            MultisheetStyle::Altium => write!(f, "Altium"),
        }
    }
}

impl std::fmt::Display for GridStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GridStyle::Dots => write!(f, "Dots"),
            GridStyle::Lines => write!(f, "Lines"),
            GridStyle::SmallCrosses => write!(f, "Small crosses"),
        }
    }
}

/// How a click selects/drags a pin in the symbol editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PinSelectionMode {
    /// Altium parity — only the pin body/tip is grabbable.
    #[default]
    PinOnly,
    /// The pin is also grabbable by its name or number label, and a
    /// selected pin's labels glow with it.
    TextAndPin,
}

impl PinSelectionMode {
    pub const ALL: [PinSelectionMode; 2] = [PinSelectionMode::PinOnly, PinSelectionMode::TextAndPin];
    /// True when name/number labels are grabbable + glow.
    pub fn allows_label_grab(self) -> bool {
        matches!(self, PinSelectionMode::TextAndPin)
    }

    /// Stable token used to persist this mode to `prefs.json`.
    pub fn pref_token(self) -> &'static str {
        match self {
            PinSelectionMode::PinOnly => "pin_only",
            PinSelectionMode::TextAndPin => "text_and_pin",
        }
    }

    /// Parse a persisted token back into a mode — unknown/legacy values
    /// fall back to the `PinOnly` default.
    pub fn from_pref_token(s: &str) -> Self {
        match s {
            "text_and_pin" => PinSelectionMode::TextAndPin,
            _ => PinSelectionMode::PinOnly,
        }
    }
}

impl std::fmt::Display for PinSelectionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            PinSelectionMode::PinOnly => "Pin only",
            PinSelectionMode::TextAndPin => "Text and pin",
        })
    }
}

#[derive(Clone, Copy)]
struct CanvasTextConfig {
    font_name: &'static str,
    font: iced::Font,
    size_scale: f32,
    bold: bool,
    italic: bool,
    power_port_style: PowerPortStyle,
    label_style: LabelStyle,
    multisheet_style: MultisheetStyle,
    grid_style: GridStyle,
    /// Grid style for the symbol editor (independent of schematic grid style).
    symbol_grid_style: GridStyle,
}

fn build_font(name: &'static str, bold: bool, italic: bool) -> iced::Font {
    iced::Font {
        family: iced::font::Family::Name(name),
        weight: if bold {
            iced::font::Weight::Bold
        } else {
            iced::font::Weight::Normal
        },
        stretch: iced::font::Stretch::Normal,
        style: if italic {
            iced::font::Style::Italic
        } else {
            iced::font::Style::Normal
        },
    }
}

fn canvas_text_config() -> &'static RwLock<CanvasTextConfig> {
    static CONFIG: OnceLock<RwLock<CanvasTextConfig>> = OnceLock::new();
    CONFIG.get_or_init(|| {
        RwLock::new(CanvasTextConfig {
            font_name: "Iosevka",
            font: IOSEVKA,
            size_scale: 1.0,
            bold: false,
            italic: false,
            power_port_style: PowerPortStyle::Altium,
            label_style: LabelStyle::Standard,
            multisheet_style: MultisheetStyle::Standard,
            grid_style: GridStyle::Dots,
            symbol_grid_style: GridStyle::Dots,
        })
    })
}

pub fn set_canvas_font_name(name: &str) {
    let leaked: &'static str = Box::leak(name.to_string().into_boxed_str());
    if let Ok(mut cfg) = canvas_text_config().write() {
        cfg.font_name = leaked;
        cfg.font = build_font(leaked, cfg.bold, cfg.italic);
    }
}

pub fn set_canvas_font_size(size_px: f32) {
    let scale = (size_px / 11.0).clamp(0.5, 3.0);
    if let Ok(mut cfg) = canvas_text_config().write() {
        cfg.size_scale = scale;
    }
}

pub fn set_canvas_font_style(bold: bool, italic: bool) {
    if let Ok(mut cfg) = canvas_text_config().write() {
        cfg.bold = bold;
        cfg.italic = italic;
        cfg.font = build_font(cfg.font_name, cfg.bold, cfg.italic);
    }
}

pub fn set_power_port_style(style: PowerPortStyle) {
    if let Ok(mut cfg) = canvas_text_config().write() {
        cfg.power_port_style = style;
    }
}

pub fn power_port_style() -> PowerPortStyle {
    canvas_text_config()
        .read()
        .map(|c| c.power_port_style)
        .unwrap_or(PowerPortStyle::Altium)
}

pub fn set_label_style(style: LabelStyle) {
    if let Ok(mut cfg) = canvas_text_config().write() {
        cfg.label_style = style;
    }
}

pub fn label_style() -> LabelStyle {
    canvas_text_config()
        .read()
        .map(|c| c.label_style)
        .unwrap_or(LabelStyle::Standard)
}

pub fn set_multisheet_style(style: MultisheetStyle) {
    if let Ok(mut cfg) = canvas_text_config().write() {
        cfg.multisheet_style = style;
    }
}

pub fn multisheet_style() -> MultisheetStyle {
    canvas_text_config()
        .read()
        .map(|c| c.multisheet_style)
        .unwrap_or(MultisheetStyle::Standard)
}

pub fn set_grid_style(style: GridStyle) {
    if let Ok(mut cfg) = canvas_text_config().write() {
        cfg.grid_style = style;
    }
}

pub fn grid_style() -> GridStyle {
    canvas_text_config()
        .read()
        .map(|c| c.grid_style)
        .unwrap_or(GridStyle::Dots)
}

pub fn set_symbol_grid_style(style: GridStyle) {
    if let Ok(mut cfg) = canvas_text_config().write() {
        cfg.symbol_grid_style = style;
    }
}

pub fn symbol_grid_style() -> GridStyle {
    canvas_text_config()
        .read()
        .map(|c| c.symbol_grid_style)
        .unwrap_or(GridStyle::Dots)
}

pub fn canvas_font() -> iced::Font {
    canvas_text_config()
        .read()
        .map(|c| c.font)
        .unwrap_or(IOSEVKA)
}

pub fn canvas_font_size_scale() -> f32 {
    canvas_text_config()
        .read()
        .map(|c| c.size_scale)
        .unwrap_or(1.0)
}

pub fn to_iced(c: &signex_types::theme::Color) -> iced::Color {
    iced::Color::from_rgba8(c.r, c.g, c.b, c.a as f32 / 255.0)
}
