//! Pad-stack table cells + the per-pad copper row.
//!
//! These helpers render the pad-properties table chrome (header, body
//! rows, individual data cells) — used by `pad_form::render_pad_form_pad_stack`
//! to compose the COPPER / HOLE / PASTE / SOLDER rows.

use iced::widget::{
    Column, Space, button, column, container, pick_list, row, scrollable, text, text_input,
};
use iced::{Background, Border, Color, Element, Length, Theme};

use super::super::PanelMsg;
use super::pad_form::{
    pad_size_x_msg, pad_size_y_msg, pad_shape_msg, pad_thermal_relief_msg, PadEditTarget,
    PadFormValues,
};
use super::pad_stack_preview::PadShapeChoice;

/// v0.20 — Altium-style table header row. Renders the column titles
/// in muted small text with the same FillPortion layout the data
/// rows use, so columns line up vertically. First cell is the
/// section family name (COPPER / HOLE / PASTE / SOLDER).
pub(super) fn pad_table_header<'a>(
    cols: &[&'static str],
    portions: &[u16],
    muted: Color,
    border_c: Color,
) -> iced::Element<'a, PanelMsg> {
    let mut row = iced::widget::Row::new().spacing(4).align_y(iced::Alignment::Center);
    for (i, label) in cols.iter().enumerate() {
        let portion = portions.get(i).copied().unwrap_or(1);
        row = row.push(
            text(label.to_string())
                .size(9)
                .color(muted)
                .width(Length::FillPortion(portion)),
        );
    }
    container(row)
        .padding([4, 8])
        .width(Length::Fill)
        .style(move |_: &Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgba(
                1.0, 1.0, 1.0, 0.03,
            ))),
            border: iced::Border {
                width: 0.0,
                radius: 0.0.into(),
                color: border_c,
            },
            ..iced::widget::container::Style::default()
        })
        .into()
}

/// v0.20 — Altium-style table data row. First cell is the row label
/// (e.g. "All Layers", "Pad Hole", "Top Paste"); remaining cells are
/// caller-provided Elements. Width portions match the header row.
pub(super) fn pad_table_row<'a>(
    label: &'a str,
    cells: Vec<iced::Element<'a, PanelMsg>>,
    portions: &[u16],
    muted: Color,
    primary: Color,
    _border_c: Color,
) -> iced::Element<'a, PanelMsg> {
    let _ = muted;
    let label_portion = 3_u16;
    let mut row = iced::widget::Row::new().spacing(4).align_y(iced::Alignment::Center);
    row = row.push(
        text(label.to_string())
            .size(10)
            .color(primary)
            .width(Length::FillPortion(label_portion)),
    );
    for (i, cell) in cells.into_iter().enumerate() {
        let portion = portions.get(i).copied().unwrap_or(1);
        row = row.push(container(cell).width(Length::FillPortion(portion)));
    }
    container(row).padding([3, 8]).width(Length::Fill).into()
}

/// v0.20 — text_input cell with the same chrome as `pad_input_row`'s
/// input but no leading label — meant for table data rows.
pub(super) fn pad_table_input_cell<'a>(
    value: String,
    placeholder: &'a str,
    on_input: impl Fn(String) -> PanelMsg + 'a,
    muted: Color,
    primary: Color,
    border_c: Color,
) -> iced::Element<'a, PanelMsg> {
    text_input(placeholder, &value)
        .size(10)
        .padding(2)
        .style(move |_: &Theme, _| iced::widget::text_input::Style {
            background: iced::Background::Color(iced::Color::from_rgba(1.0, 1.0, 1.0, 0.04)),
            border: iced::Border {
                width: 1.0,
                radius: 2.0.into(),
                color: border_c,
            },
            icon: iced::Color::TRANSPARENT,
            placeholder: muted,
            value: primary,
            selection: iced::Color::from_rgba(0.4, 0.6, 1.0, 0.4),
        })
        .on_input(on_input)
        .into()
}

/// v0.20 — pick_list cell for table data rows.
pub(super) fn pad_table_picklist_cell<'a, T>(
    options: &'a [T],
    selected: T,
    on_change: impl Fn(T) -> PanelMsg + 'a + Clone,
) -> iced::Element<'a, PanelMsg>
where
    T: Clone + Eq + std::fmt::Display + 'static,
{
    pick_list(options, Some(selected), on_change)
        .text_size(10)
        .padding([2, 6])
        .width(Length::Fill)
        .into()
}

/// v0.20 — checkbox cell for table data rows.
pub(super) fn pad_table_check_cell<'a>(
    on: bool,
    on_toggle: impl Fn(bool) -> PanelMsg + 'a,
) -> iced::Element<'a, PanelMsg> {
    container(
        iced::widget::checkbox(on)
            .on_toggle(on_toggle)
            .size(12)
            .spacing(0),
    )
    .padding([2, 4])
    .into()
}

/// v0.20 — disabled / read-only cell. Shows a value with greyed
/// chrome but no input handler. Used for column placeholders that
/// don't yet have a backing field (e.g. "0%" in the PASTE table
/// where percentage overrides aren't wired yet).
pub(super) fn pad_table_disabled_cell<'a>(
    value: impl Into<String>,
    muted: Color,
    border_c: Color,
) -> iced::Element<'a, PanelMsg> {
    container(text(value.into()).size(10).color(muted))
        .padding([3, 6])
        .style(move |_: &Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgba(
                1.0, 1.0, 1.0, 0.02,
            ))),
            border: iced::Border {
                width: 1.0,
                radius: 2.0.into(),
                color: border_c,
            },
            ..iced::widget::container::Style::default()
        })
        .width(Length::Fill)
        .into()
}

/// v0.20 — static text cell. No chrome, just dim text — used for
/// columns like "Rule Expansion" that aren't yet user-editable
/// (a v0.21 follow-up adds the per-rule override picker).
pub(super) fn pad_table_static_cell<'a>(
    value: impl Into<String>,
    muted: Color,
) -> iced::Element<'a, PanelMsg> {
    container(text(value.into()).size(10).color(muted))
        .padding([3, 6])
        .width(Length::Fill)
        .into()
}

/// v0.20 — single COPPER table row. Built inline because all four
/// data cells (X-Size, Y-Size, Shape, Relief) reference different
/// fields on PadFormValues + different message constructors.
pub(super) fn pad_copper_row<'a>(
    label: &'a str,
    values: &PadFormValues,
    current_shape: PadShapeChoice,
    target: PadEditTarget,
    muted: Color,
    primary: Color,
    border_c: Color,
    is_authoritative: bool,
) -> iced::Element<'a, PanelMsg> {
    // v0.25 polish — render empty when the value is effectively
    // zero so the user can clear the input by deleting all
    // characters. With format!("{:.3}", 0.0) the field rebuilds to
    // "0.000" on every render, which fights the user's typing as
    // soon as they backspace past the first character. The fp_parse
    // handler turns an empty string into 0.0 so the round-trip
    // works: empty → 0.0 → empty. A nonzero size renders the value
    // verbatim. The longer-term per-field buffer pattern (see
    // reference_erasable_numeric_input.md) is queued for v0.26.
    let buf_or_empty = |v: f64| -> String {
        if v.abs() < 1e-9 {
            String::new()
        } else {
            format!("{v:.3}")
        }
    };
    let x_buf = buf_or_empty(values.size_x_mm);
    let y_buf = buf_or_empty(values.size_y_mm);
    let cells = if is_authoritative {
        vec![
            pad_table_input_cell(
                x_buf,
                "",
                move |v| pad_size_x_msg(target, v),
                muted,
                primary,
                border_c,
            ),
            pad_table_input_cell(
                y_buf,
                "",
                move |v| pad_size_y_msg(target, v),
                muted,
                primary,
                border_c,
            ),
            pad_table_picklist_cell(PadShapeChoice::ALL, current_shape, move |c| {
                pad_shape_msg(target, c.to_lib())
            }),
            pad_table_check_cell(values.stack.thermal_relief, move |v| {
                pad_thermal_relief_msg(target, v)
            }),
        ]
    } else {
        // Mid / Bottom rows mirror Top — no per-layer overrides yet.
        vec![
            pad_table_disabled_cell(&format!("{:.3}", values.size_x_mm), muted, border_c),
            pad_table_disabled_cell(&format!("{:.3}", values.size_y_mm), muted, border_c),
            pad_table_static_cell(&format!("{current_shape}"), muted),
            pad_table_disabled_cell("", muted, border_c),
        ]
    };
    pad_table_row(label, cells, &[2, 2, 3, 1], muted, primary, border_c)
}

