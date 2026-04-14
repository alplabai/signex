//! wgpu rendering primitives for Signex — schematic and PCB drawing.
//!
//! This crate provides the rendering logic that bridges `signex-types`
//! domain objects to Iced Canvas draw calls. No Iced Application logic here —
//! just pure rendering functions.

pub mod colors;
pub mod schematic;

use std::sync::{OnceLock, RwLock};

/// The schematic canvas font. Loaded as a binary asset in `main.rs` and
/// available by name once the application starts.
pub const IOSEVKA: iced::Font = iced::Font::with_name("Iosevka");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PowerPortStyle {
	KiCad,
	#[default]
	Altium,
}

impl std::fmt::Display for PowerPortStyle {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			PowerPortStyle::KiCad => write!(f, "KiCad"),
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

pub fn canvas_font() -> iced::Font {
	canvas_text_config().read().map(|c| c.font).unwrap_or(IOSEVKA)
}

pub fn canvas_font_size_scale() -> f32 {
	canvas_text_config()
		.read()
		.map(|c| c.size_scale)
		.unwrap_or(1.0)
}
