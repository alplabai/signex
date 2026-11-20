//! Row builders of the Document Options General tab (#2531): one setting per
//! row on the fixed label column, a segmented choice, a label-column enable
//! box whose value greys and takes no input while it is off, and the two
//! typed numbers of a W / H or V / H pair.

use super::super::*;
use crate::panels::form_look;
use crate::styles::{Spacing, TypeScale, ti};
use iced::Widget;
use signex_widgets::enable_section::dependents_veil;
use signex_widgets::inert::Inert;
use signex_widgets::{CheckboxLabel as _, Label, SegmentedStyle, segmented};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// A row: `label` on the fixed label column, then `control`.
pub(super) fn label_row<'a>(
    label: Element<'a, PanelMsg>,
    control: Element<'a, PanelMsg>,
) -> Element<'a, PanelMsg> {
    container(
        row![
            container(label).width(PROPERTY_LABEL_COLUMN).clip(true),
            container(control).width(Length::Fill),
        ]
        .spacing(PROPERTY_ROW_GAP)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .boxed()
}

/// A plain row label in the muted ink.
pub(super) fn row_label<'a>(
    label: impl Into<Label<'a>>,
    tokens: &ThemeTokens,
) -> Element<'a, PanelMsg> {
    property_label(label, crate::styles::tokens::resolve(tokens).ink3)
}

/// The Graphite segmented look on `tokens`, segments sharing the width.
pub(super) fn segmented_style(tokens: &ThemeTokens) -> SegmentedStyle {
    let r = crate::styles::tokens::resolve(tokens);
    SegmentedStyle::new(
        r.sunken,
        r.raised,
        r.shadow,
        r.ink3,
        ti(tokens.text),
        TS.body,
    )
    .fill_width(true)
}

/// A labelled segmented choice over `options`.
pub(super) fn segmented_row<'a, V>(
    label: impl Into<Label<'a>>,
    options: Vec<(V, Label<'a>)>,
    selected: V,
    tokens: &ThemeTokens,
    on_select: impl Fn(V) -> PanelMsg + 'a,
) -> Element<'a, PanelMsg>
where
    V: Copy + PartialEq + 'a,
{
    label_row(
        row_label(label, tokens),
        segmented(options, selected, segmented_style(tokens), on_select),
    )
}

/// An enable box with its label, for the label column or a sub-header.
pub(super) fn enable_box<'a>(
    label: impl Into<Label<'a>>,
    is_on: bool,
    on_toggle: PanelMsg,
) -> Element<'a, PanelMsg> {
    signex_widgets::checkbox(is_on)
        .labelled(label)
        .on_toggle(move |_| on_toggle.clone())
        .size(TS.label)
        .text_size(TS.body)
        .spacing(SP.s)
        .style(form_look::checkbox_style)
        .boxed()
}

/// `control` as given while `is_on`; greyed under the panel veil and deaf
/// to input while off (the disable-when-off rule, BB3).
pub(super) fn unless_off<'a>(
    is_on: bool,
    tokens: &ThemeTokens,
    control: Element<'a, PanelMsg>,
) -> Element<'a, PanelMsg> {
    match dependents_veil(ti(tokens.panel_bg), is_on) {
        Some(veil) => Inert::new(control, veil).into(),
        None => control,
    }
}

/// One typed number of a pair: its short axis name, the field and an
/// optional unit. An `invalid` field (its text is not a number) is outlined
/// in the error colour.
pub(super) fn axis_field<'a>(
    axis: impl Into<Label<'a>>,
    value: String,
    unit: Option<Label<'a>>,
    tokens: &ThemeTokens,
    on_input: impl Fn(String) -> PanelMsg + 'a,
    on_submit: PanelMsg,
    invalid: bool,
) -> Element<'a, PanelMsg> {
    let muted = crate::styles::tokens::resolve(tokens).ink3;
    let error = ti(tokens.error);
    let input = signex_widgets::text_input(signex_i18n::UserText::new(String::new()), value)
        .on_input(on_input)
        .on_submit(on_submit)
        .size(TS.body)
        .padding([3, 6])
        .width(Length::Fill)
        .style(move |theme: &Theme, status| {
            let mut style = form_look::text_input_style(theme, status);
            if invalid {
                style.border.color = error;
            }
            style
        });
    let mut cells = row![text(axis).size(TS.caption).color(muted), input]
        .spacing(SP.xs)
        .align_y(iced::Alignment::Center);
    if let Some(unit) = unit {
        cells = cells.push(text(unit).size(TS.caption).color(muted).boxed());
    }
    cells.width(Length::Fill).boxed()
}

/// Two fields side by side on one row.
pub(super) fn pair<'a>(
    a: Element<'a, PanelMsg>,
    b: Element<'a, PanelMsg>,
) -> Element<'a, PanelMsg> {
    row![a, b].spacing(SP.m).width(Length::Fill).boxed()
}
