//! The group header: chevron, mono name, caption fields, spacer and status
//! pill on the sunken band, a `line-soft` rule under it.

use iced::widget::text::{Ellipsis, Wrapping};
use iced::widget::{Row, Space, column, container};
use iced::{Alignment, Background, Element, Font, Length, Padding, Theme, Widget, font};

use super::{CompactGroupStyle, GroupHeader, bottom_rule};
use crate::chevron::chevron;
use crate::chips::kind_chip;
use crate::{button, text};

/// The header of a group that is `is_open`; a click sends `toggle`.
pub(super) fn view<'a, M: Clone + 'a>(
    header: &GroupHeader<'a>,
    is_open: bool,
    toggle: M,
    style: CompactGroupStyle,
) -> Element<'a, M> {
    let m = style.metrics();
    let name = text(header.name.clone())
        .size(style.text_size)
        .font(Font {
            weight: font::Weight::Semibold,
            ..Font::MONOSPACE
        })
        .color(style.ink)
        .wrapping(Wrapping::None)
        .ellipsis(Ellipsis::End);
    let mut fields: Row<Element<'a, M>> = Row::new()
        .spacing(m.header_gap)
        .align_y(Alignment::Center)
        .push(chevron(is_open, style.text_size, style.ink4).boxed())
        .push(name.boxed());
    for caption in &header.captions {
        fields = fields.push(
            text(caption.clone())
                .size(style.caption_size)
                .color(style.ink3)
                .wrapping(Wrapping::None)
                .boxed(),
        );
    }
    fields = fields.push(Space::new().width(Length::Fill).boxed());
    if let Some(status) = &header.status {
        fields = fields.push(kind_chip(
            status.label.clone(),
            style.pill_tone(status.tone),
            style.chip_metrics(),
        ));
    }
    let band = button(fields)
        .width(Length::Fill)
        .height(m.header_height - crate::theme_ext::HAIRLINE)
        .padding(Padding::from([m.header_pad.0, m.header_pad.1]))
        .on_press(toggle)
        .style(move |_: &Theme, _| crate::button::Style {
            background: Some(Background::Color(style.sunken)),
            text_color: style.ink,
            ..crate::button::Style::default()
        });
    container(column![band, bottom_rule(style.line_soft)])
        .width(Length::Fill)
        .boxed()
}
