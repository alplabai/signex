//! Badges of a styled tree row that are not kind tags: the trailing pill
//! ("missing"), the warn glyph that leads a problem row and a group
//! header's count.

use std::sync::OnceLock;

use iced::widget::{container, svg};
use iced::{Background, Color, Element, Theme, Widget};

use super::TreeMsg;
use crate::text;
use crate::theme_ext::{Radii, TypeScale};

/// Height of a trailing pill.
const PILL_HEIGHT: f32 = 16.0;
/// Space left and right of a pill's text.
const PILL_PAD_X: f32 = 6.0;

const WARN: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16"><path d="M8 2.5l6 10.5H2z" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linejoin="round"/><path d="M8 6.5v3M8 11.3v.2" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/></svg>"#;

fn warn_handle() -> svg::Handle {
    static HANDLE: OnceLock<svg::Handle> = OnceLock::new();
    HANDLE
        .get_or_init(|| svg::Handle::from_memory(WARN))
        .clone()
}

/// Caption size of the scale whose body is the row font `font`.
pub(super) fn caption(font: f32) -> f32 {
    TypeScale::from_base(font).caption
}

/// Section-label size of the scale whose body is the row font `font`.
pub(super) fn section(font: f32) -> f32 {
    TypeScale::from_base(font).section
}

/// The warn glyph that leads a problem row in place of its kind tag.
pub(super) fn warn_glyph(side: f32, ink: Color) -> Element<'static, TreeMsg> {
    svg(warn_handle())
        .width(side)
        .height(side)
        .style(move |_: &Theme, _| svg::Style {
            color: Some(ink),
            ..Default::default()
        })
        .boxed()
}

/// A trailing pill: caption text on its tone fill, fully rounded.
pub(super) fn pill(
    label: crate::Label<'static>,
    font: f32,
    fill: Color,
    ink: Color,
) -> Element<'static, TreeMsg> {
    container(
        text(label)
            .size(caption(font))
            .color(ink)
            .wrapping(iced::widget::text::Wrapping::None),
    )
    .padding([0.0, PILL_PAD_X])
    .height(PILL_HEIGHT)
    .align_y(iced::alignment::Vertical::Center)
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(fill)),
        border: iced::Border {
            radius: Radii::GRAPHITE.pill.into(),
            ..iced::Border::default()
        },
        ..container::Style::default()
    })
    .boxed()
}

/// A group header's count: a mono caption note, no fill.
pub(super) fn count(
    label: crate::Label<'static>,
    font: f32,
    ink: Color,
) -> Element<'static, TreeMsg> {
    text(label)
        .size(caption(font))
        .font(iced::Font::MONOSPACE)
        .color(ink)
        .wrapping(iced::widget::text::Wrapping::None)
        .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The pill and the count read at the caption step under the row font,
    /// the group label at the section step, as the type scale has them.
    #[test]
    fn sizes_follow_the_type_scale_of_the_row_font() {
        let scale = TypeScale::from_base(12.0);
        assert_eq!(caption(12.0), scale.caption);
        assert_eq!(section(12.0), scale.section);
        assert!(caption(12.0) < 12.0 && section(12.0) < 12.0);
    }

    #[test]
    fn the_warn_glyph_parses_as_svg() {
        assert!(std::str::from_utf8(WARN).is_ok_and(|s| s.starts_with("<svg")));
    }
}
