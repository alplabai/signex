//! One member row: mono name (ellipsis) with an optional caption, the
//! right-aligned Δ and the small target bar. Hover takes the `hover` token;
//! a selected row is `accent-soft` with a 2 px `accent` bar on its left
//! edge. An out-of-band Δ is `err`, medium weight.

use iced::alignment::Horizontal;
use iced::widget::text::{Ellipsis, Wrapping};
use iced::widget::{Row, Space, column, container, row};
use iced::{Alignment, Background, Element, Font, Length, Padding, Theme, Widget, font};

use super::{CompactGroupStyle, Member, bottom_rule};
use crate::target_band::{BandSize, signed_delta, target_band};
use crate::theme_ext::HAIRLINE;
use crate::{LengthFormat, button, text};

/// The row of `member`; a click sends `select`.
pub(super) fn view<'a, K, M: Clone + 'a>(
    member: &Member<'a, K>,
    format: &LengthFormat,
    select: M,
    style: CompactGroupStyle,
) -> Element<'a, M> {
    let m = style.metrics();
    let is_selected = member.is_selected;
    let is_out = member.is_out_of_band();

    let mut name: Row<Element<'a, M>> = Row::new()
        .spacing(m.column_gap)
        .align_y(Alignment::Center)
        .width(Length::Fill)
        .clip(true)
        .push(
            text(member.name.clone())
                .size(style.text_size)
                .font(Font::MONOSPACE)
                .color(style.ink)
                .wrapping(Wrapping::None)
                .ellipsis(Ellipsis::End)
                .boxed(),
        );
    if let Some(caption) = &member.caption {
        name = name.push(
            text(caption.clone())
                .size(style.caption_size)
                .color(style.ink4)
                .wrapping(Wrapping::None)
                .boxed(),
        );
    }
    let delta_font = if is_out {
        Font {
            weight: font::Weight::Medium,
            ..Font::MONOSPACE
        }
    } else {
        Font::MONOSPACE
    };
    let delta = text(signed_delta(format, member.delta()))
        .size(style.text_size)
        .font(delta_font)
        .color(if is_out { style.err } else { style.ink })
        .wrapping(Wrapping::None)
        .width(m.delta_width)
        .align_x(Horizontal::Right);
    let bar = container(target_band(
        member.length,
        member.target,
        member.tolerance,
        BandSize::Small,
        style.band,
    ))
    .width(m.band_width);

    let cells = row![name, delta, bar]
        .spacing(m.column_gap)
        .align_y(Alignment::Center)
        .width(Length::Fill);
    // The accent bar's slot is always there, so selection does not shift
    // the row's content.
    let accent = container(Space::new())
        .width(m.accent_bar)
        .height(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: is_selected.then_some(Background::Color(style.accent)),
            ..container::Style::default()
        });
    let body = row![
        accent,
        container(cells)
            .padding(Padding::from([0.0, m.row_pad - m.accent_bar]))
            .width(Length::Fill)
            .height(Length::Fill)
            .align_y(Alignment::Center),
    ]
    .height(Length::Fill);
    let pressable = button(body)
        .width(Length::Fill)
        .height(m.row_height - HAIRLINE)
        .padding(0)
        .on_press(select)
        .style(move |_: &Theme, status: crate::button::Status| {
            let fill = if is_selected {
                Some(style.accent_soft)
            } else if matches!(
                status,
                crate::button::Status::Hovered | crate::button::Status::Pressed
            ) {
                Some(style.hover)
            } else {
                None
            };
            crate::button::Style {
                background: fill.map(Background::Color),
                text_color: style.ink,
                ..crate::button::Style::default()
            }
        });
    column![pressable, bottom_rule(style.line_soft)]
        .width(Length::Fill)
        .boxed()
}
