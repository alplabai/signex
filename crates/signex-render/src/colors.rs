//! Color utilities — convert signex-types theme colors to iced::Color.

use signex_types::theme;

/// Convert a signex Color to an iced Color.
pub fn to_iced(c: &theme::Color) -> iced::Color {
    iced::Color::from_rgba8(c.r, c.g, c.b, c.a as f32 / 255.0)
}
