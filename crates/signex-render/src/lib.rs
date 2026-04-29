//! wgpu rendering primitives for Signex — schematic and PCB drawing.
//!
//! This crate provides the rendering logic that bridges `signex-types`
//! domain objects to Iced Canvas draw calls. No Iced Application logic here —
//! just pure rendering functions.

pub mod colors;
pub mod pcb;
pub mod schematic;

use std::sync::{OnceLock, RwLock};

/// The schematic canvas font. Loaded as a binary asset in `main.rs` and
/// available by name once the application starts.
pub const IOSEVKA: iced::Font = iced::Font::with_name("Iosevka");

pub use signex_types::schematic::SCHEMATIC_PT_TO_MM;
pub use signex_types::schematic::SCHEMATIC_TEXT_MM;

/// Legacy stroke font stores "size" as cap-height; Iced TrueType uses em-square
/// (cap height ≈ 72 % of em). To render a stroke-font size at the same visual
/// cap height, we draw it at em = size / 0.72. Use this value for BOTH the
/// canvas font size AND any offset / hit-test math so they stay in sync —
/// applying the scale only at render sites (as a separate multiplier on
/// `screen_font`) silently breaks label/pin/text anchors.
pub const SCHEMATIC_TEXT_EM_MM: f64 = SCHEMATIC_TEXT_MM / 0.72;

/// Power-port glyph style preference. `Standard` matches the rounded
/// shapes typical of open-source schematic editors; `Altium` matches
/// the Altium Designer signature look.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PowerPortStyle {
    Standard,
    #[default]
    Altium,
}

/// Net-label glyph style preference. `Standard` / `Altium` split mirrors
/// `PowerPortStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LabelStyle {
    #[default]
    Standard,
    Altium,
}

/// How hierarchical child sheets render. `Standard` mode keeps each
/// sheet's stroke/fill colour from the source file (with theme
/// component-body fallback) so the sheet blends with the rest of the
/// schematic. `Altium` mode draws sheets with Altium Designer's
/// signature greenish palette when no per-sheet colour is set in the
/// file. Per-sheet colours from the source file always win,
/// regardless of the active style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MultisheetStyle {
    #[default]
    Standard,
    Altium,
}

/// Visible schematic grid rendering style: dots, lines, or small crosses.
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

impl std::fmt::Display for LabelStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LabelStyle::Standard => write!(f, "Standard"),
            LabelStyle::Altium => write!(f, "Altium"),
        }
    }
}

impl std::fmt::Display for PowerPortStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PowerPortStyle::Standard => write!(f, "Standard"),
            PowerPortStyle::Altium => write!(f, "Altium"),
        }
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
