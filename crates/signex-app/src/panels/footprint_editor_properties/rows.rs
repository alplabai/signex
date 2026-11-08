//! Label / value rows of the footprint Properties panel. The shared form
//! rows give the label two parts of seven, which clips "Description" in this
//! narrow dock; these give it four of nine so a long label wraps by word.

use iced::widget::{container, row, text, text_input};
use iced::{Background, Border, Color, Element, Length, Theme, Widget};
use signex_widgets::dropdown_style::GraphiteDropdown as _;

use super::{PanelMsg, SP, TS};
use crate::panels::{PROPERTY_ROW_PAD_X, form_look};

const LABEL_PORTION: u16 = 4;
const VALUE_PORTION: u16 = 5;

fn label<'a, M: 'a>(label: &str, color: Color) -> Element<'a, M> {
    container(
        text(label.to_string())
            .size(TS.body)
            .color(color)
            .wrapping(iced::widget::text::Wrapping::Word),
    )
    .width(Length::FillPortion(LABEL_PORTION))
    .clip(true)
    .boxed()
}

/// An editable text row.
pub(super) fn edit_row<'a>(
    key: &str,
    value: &str,
    label_c: Color,
    on_input: impl Fn(String) -> PanelMsg + 'a,
) -> Element<'a, PanelMsg> {
    container(
        row![
            label(key, label_c),
            text_input("", value.to_owned())
                .on_input(on_input)
                .size(TS.body)
                .padding(SP.s)
                .style(form_look::text_input_style)
                .width(Length::FillPortion(VALUE_PORTION)),
        ]
        .spacing(SP.m)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .boxed()
}

/// A read-only value row.
pub(super) fn value_row<'a>(
    key: &str,
    value: &str,
    label_c: Color,
    input_bg: Color,
    input_border: Color,
) -> Element<'a, PanelMsg> {
    let ink = crate::styles::tokens::readable_on(input_bg);
    container(
        row![
            label(key, label_c),
            container(
                container(
                    text(value.to_string())
                        .size(TS.body)
                        .color(ink)
                        .wrapping(iced::widget::text::Wrapping::None),
                )
                .width(Length::Fill)
                .clip(true),
            )
            .padding([3, 6])
            .width(Length::FillPortion(VALUE_PORTION))
            .style(move |theme: &Theme| {
                let (bg, bdr, radius) = form_look::value_box(theme, input_bg, input_border, 2.0);
                container::Style {
                    background: Some(Background::Color(bg)),
                    border: Border {
                        width: 1.0,
                        radius: radius.into(),
                        color: bdr,
                        ..Border::default()
                    },
                    ..container::Style::default()
                }
            }),
        ]
        .spacing(SP.m)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .boxed()
}

/// A pick-list row aligned with `edit_row`: the Graphite pick-list look and
/// the same label / value split, so a dropdown is as wide as a text field.
pub(super) fn pick_row<'a, T>(
    key: &str,
    options: &'a [T],
    selected: T,
    on_change: impl Fn(T) -> PanelMsg + 'a,
    label_c: Color,
) -> Element<'a, PanelMsg>
where
    T: Clone + Eq + std::fmt::Display + 'static,
{
    container(
        row![
            label(key, label_c),
            iced::widget::pick_list(Some(selected), options, |t: &T| t.to_string())
                .graphite()
                .on_select(on_change)
                .text_size(TS.body)
                .padding(SP.s)
                .width(Length::FillPortion(VALUE_PORTION))
                .boxed(),
        ]
        .spacing(SP.m)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .boxed()
}
