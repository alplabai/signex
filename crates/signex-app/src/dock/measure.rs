//! Real text measurement for the collapsed rail.

use iced::advanced::text::{self, Paragraph as _};
use iced::{Pixels, Size};

type Para = <iced::Renderer as text::Renderer>::Paragraph;

/// Width in px of `label` on one line at `size`, shaped by the font system.
pub(super) fn text_width(label: &str, size: f32) -> f32 {
    let para = Para::with_text(text::Text {
        content: label,
        bounds: Size::INFINITE,
        size: Pixels(size),
        line_height: text::LineHeight::default(),
        font: iced::Font::DEFAULT,
        align_x: text::Alignment::Default,
        align_y: iced::alignment::Vertical::Top,
        shaping: text::Shaping::Basic,
        wrapping: text::Wrapping::None,
        ellipsis: text::Ellipsis::None,
        hint_factor: None,
    });
    para.min_width()
}
