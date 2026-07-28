use iced::Color;
use signex_types::theme::Color as ThemeColor;

pub(crate) fn color(color: ThemeColor) -> Color {
    Color::from_rgba8(color.r, color.g, color.b, f32::from(color.a) / 255.0)
}
