//! Label / value rows of the footprint Properties panel. The shared form
//! rows give the label two parts of seven, which clips "Description" in this
//! narrow dock; these give it four of nine so a long label wraps by word.

use iced::widget::{container, row, text, text_input};
use iced::{Background, Border, Color, Element, Length, Theme, Widget};

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
            text_input("", value)
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
