//! Label / value rows of the footprint Properties panel. The shared form
//! rows give the label two parts of seven, which clips "Description" in this
//! narrow dock; these give it four of nine so a long label wraps by word.

use iced::widget::{container, row};
use iced::{Background, Border, Color, Element, Length, Theme, Widget};
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::{raw, text, text_input};

use super::{PanelMsg, SP, TS};
use crate::panels::{PROPERTY_ROW_PAD_X, form_look};

const LABEL_PORTION: u16 = 4;
const VALUE_PORTION: u16 = 5;

fn label<'a, M: 'a>(label: &str, color: Color) -> Element<'a, M> {
    container(
        text(raw(label.to_string()))
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
            text_input(raw(""), value.to_owned())
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

/// An editable text row with a short muted note after the field
/// ("default", "Axes off").
pub(super) fn edit_row_note<'a>(
    key: &str,
    value: &str,
    note: signex_i18n::Localized,
    label_c: Color,
    on_input: impl Fn(String) -> PanelMsg + 'a,
) -> Element<'a, PanelMsg> {
    let field = row![
        text_input(signex_i18n::UserText::new(String::new()), value.to_owned())
            .on_input(on_input)
            .size(TS.body)
            .padding(SP.s)
            .style(form_look::text_input_style)
            .width(Length::Fill),
        text(note)
            .size(TS.caption)
            .color(label_c)
            .wrapping(iced::widget::text::Wrapping::None),
    ]
    .spacing(SP.s)
    .align_y(iced::Alignment::Center);
    control_row(key, field.boxed(), label_c)
}

/// An editable number row outlined in `error` while its text is refused,
/// with an optional muted note after the field.
pub(super) fn refusable_edit_row<'a>(
    key: &str,
    value: &str,
    error: Option<Color>,
    note: Option<signex_i18n::Localized>,
    label_c: Color,
    on_input: impl Fn(String) -> PanelMsg + 'a,
) -> Element<'a, PanelMsg> {
    let input = text_input(signex_i18n::UserText::new(String::new()), value.to_owned())
        .on_input(on_input)
        .size(TS.body)
        .padding(SP.s)
        .style(form_look::refusable_input_style(error))
        .width(Length::Fill);
    // One tree for both, so a note coming and going keeps the field's focus.
    let gap = if note.is_some() { SP.s } else { 0.0 };
    let note: Element<'a, PanelMsg> = match note {
        Some(note) => text(note)
            .size(TS.caption)
            .color(label_c)
            .wrapping(iced::widget::text::Wrapping::None)
            .boxed(),
        None => iced::widget::Space::new().boxed(),
    };
    let field = row![input, note]
        .spacing(gap)
        .align_y(iced::Alignment::Center);
    control_row(key, field.boxed(), label_c)
}

/// A row with `control` in the value column.
pub(super) fn control_row<'a>(
    key: &str,
    control: Element<'a, PanelMsg>,
    label_c: Color,
) -> Element<'a, PanelMsg> {
    container(
        row![
            label(key, label_c),
            container(control).width(Length::FillPortion(VALUE_PORTION)),
        ]
        .spacing(SP.m)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .boxed()
}

/// A checkbox row: the label, then the checkbox in the value column.
pub(super) fn check_row<'a>(
    key: &str,
    checked: bool,
    on_toggle: impl Fn(bool) -> PanelMsg + 'a,
    label_c: Color,
) -> Element<'a, PanelMsg> {
    control_row(
        key,
        signex_widgets::checkbox(checked)
            .on_toggle(on_toggle)
            .size(TS.body)
            .spacing(0)
            .style(form_look::checkbox_style)
            .boxed(),
        label_c,
    )
}

/// A read-only value row.
pub(super) fn value_row<'a>(
    key: &str,
    value: &str,
    label_c: Color,
    input_bg: Color,
) -> Element<'a, PanelMsg> {
    let ink = crate::styles::tokens::readable_on(input_bg);
    container(
        row![
            label(key, label_c),
            container(
                container(
                    text(raw(value.to_string()))
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
                let (bg, bdr, radius) = form_look::value_box(theme);
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
    pick_row_labelled(
        key,
        options,
        selected,
        |t: &T| t.to_string(),
        on_change,
        label_c,
    )
}

/// [`pick_row`] with each option shown as `label_of` names it (a catalog
/// label rather than the option's `Display`).
pub(super) fn pick_row_labelled<'a, T>(
    key: &str,
    options: &'a [T],
    selected: T,
    label_of: impl Fn(&T) -> String + 'a,
    on_change: impl Fn(T) -> PanelMsg + 'a,
    label_c: Color,
) -> Element<'a, PanelMsg>
where
    T: Clone + Eq + 'static,
{
    container(
        row![
            label(key, label_c),
            iced::widget::pick_list(Some(selected), options, label_of)
                .graphite()
                .on_select(on_change)
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
