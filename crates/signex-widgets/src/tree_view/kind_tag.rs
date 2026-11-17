//! The leading kind tag of a styled tree row ("SCH", "PCB").

use iced::widget::container;
use iced::{Background, Color, Element, Theme, Widget};

use super::TreeMsg;
use crate::text;

/// A leading kind tag ("SCH"): mono caption on its tone fill, small radius.
pub(super) fn kind_tag(
    tag: crate::Label<'static>,
    size: f32,
    fill: Color,
    ink: Color,
) -> Element<'static, TreeMsg> {
    container(
        text(tag)
            .size(size)
            .font(iced::Font::MONOSPACE)
            .color(ink)
            .wrapping(iced::widget::text::Wrapping::None),
    )
    .padding([0.0, 4.0])
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(fill)),
        border: iced::Border {
            radius: 3.0.into(),
            ..iced::Border::default()
        },
        ..container::Style::default()
    })
    .boxed()
}
