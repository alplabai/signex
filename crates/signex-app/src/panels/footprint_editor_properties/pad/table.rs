//! Pad-stack table cells + the per-pad copper row.
//!
//! These helpers render the pad-properties table chrome (header, body
//! rows, individual data cells) — used by `pad_form::render_pad_form_pad_stack`
//! to compose the COPPER / HOLE / PASTE / SOLDER rows.

use iced::widget::{Widget, container, pick_list};
use iced::{Color, Length, Theme};
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::{raw, text, text_input};

use super::super::super::{PanelMsg, PanelPalette};
use super::form::{
    PadEditTarget, PadFormValues, pad_shape_msg, pad_size_x_msg, pad_size_y_msg,
    pad_thermal_relief_msg,
};
use super::stack_preview::PadShapeChoice;
use crate::styles::{Spacing, TypeScale};

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Width share of a table's row-label column.
const LABEL_PORTION: u16 = 3;

/// Width shares of each table's value cells, after the label column. The
/// header and every row of a table share one array so the columns line up;
/// number fields get the widest share, the checkbox column the narrowest.
pub(super) const COPPER_CELLS: [u16; 4] = [3, 3, 3, 2];
pub(super) const HOLE_CELLS: [u16; 4] = [3, 3, 3, 2];
pub(super) const PASTE_CELLS: [u16; 4] = [3, 2, 3, 2];
pub(super) const SOLDER_CELLS: [u16; 3] = [3, 3, 2];

/// Gap between table cells: the dock is narrow, so cells sit close.
const CELL_GAP: f32 = SP.xs;

/// One line of table text that ends in "…" instead of wrapping when its
/// cell is too narrow, so headers never stack onto two lines.
fn cell_text<'a>(label: impl Into<String>, color: Color, portion: u16) -> iced::widget::Text<'a> {
    text(raw(label.into()))
        .size(TS.caption)
        .color(color)
        .wrapping(iced::widget::text::Wrapping::None)
        .ellipsis(iced::widget::text::Ellipsis::End)
        .width(Length::FillPortion(portion))
}

/// The text a size cell shows: empty for zero so the user can clear the
/// field (an empty edit parses back to 0), else the value to 3 decimals.
pub(super) fn pad_size_buf(v: f64) -> String {
    if v.abs() < 1e-9 {
        String::new()
    } else {
        format!("{v:.3}")
    }
}

/// v0.20 — Altium-style table header row. Renders the column titles
/// in muted small text with the same FillPortion layout the data
/// rows use, so columns line up vertically. First cell is the
/// section family name (COPPER / HOLE / PASTE / SOLDER), sized like the
/// rows' label column; `portions` are the value cells' shares.
pub(super) fn pad_table_header<'a>(
    cols: &[&'static str],
    portions: &[u16],
    muted: Color,
    border_c: Color,
) -> iced::Element<'a, PanelMsg> {
    let mut row = iced::widget::Row::<iced::Element<'a, PanelMsg>>::new()
        .spacing(CELL_GAP)
        .align_y(iced::Alignment::Center);
    for (i, label) in cols.iter().enumerate() {
        let portion = match i {
            0 => LABEL_PORTION,
            _ => portions.get(i - 1).copied().unwrap_or(1),
        };
        row = row.push(cell_text(*label, muted, portion).boxed());
    }
    container(row)
        .padding([SP.s, SP.m])
        .width(Length::Fill)
        .style(move |_: &Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgba(
                1.0, 1.0, 1.0, 0.03,
            ))),
            border: iced::Border {
                width: 0.0,
                radius: 0.0.into(),
                color: border_c,
                ..iced::Border::default()
            },
            ..iced::widget::container::Style::default()
        })
        .boxed()
}

/// v0.20 — Altium-style table data row. First cell is the row label
/// (e.g. "All Layers", "Pad Hole", "Top Paste"); remaining cells are
/// caller-provided Elements. Width portions match the header row.
pub(super) fn pad_table_row<'a>(
    label: &'a str,
    cells: Vec<iced::Element<'a, PanelMsg>>,
    portions: &[u16],
    primary: Color,
    _border_c: Color,
) -> iced::Element<'a, PanelMsg> {
    let mut row = iced::widget::Row::<iced::Element<'a, PanelMsg>>::new()
        .spacing(CELL_GAP)
        .align_y(iced::Alignment::Center);
    row = row.push(cell_text(label, primary, LABEL_PORTION).boxed());
    for (i, cell) in cells.into_iter().enumerate() {
        let portion = portions.get(i).copied().unwrap_or(1);
        row = row.push(container(cell).width(Length::FillPortion(portion)).boxed());
    }
    container(row)
        .padding([3.0, SP.m])
        .width(Length::Fill)
        .boxed()
}

/// v0.20 — text_input cell with the same chrome as `pad_input_row`'s
/// input but no leading label — meant for table data rows.
pub(super) fn pad_table_input_cell<'a>(
    value: String,
    placeholder: &'a str,
    on_input: impl Fn(String) -> PanelMsg + 'a,
) -> iced::Element<'a, PanelMsg> {
    text_input(raw(placeholder), value)
        .size(TS.caption)
        .padding(SP.xs)
        .style(crate::panels::form_look::text_input_style)
        .on_input(on_input)
        .boxed()
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
    pick_list(Some(selected), options, |t: &T| t.to_string())
        .graphite()
        .on_select(on_change)
        .padding([SP.xs, 6.0])
        .width(Length::Fill)
        .boxed()
}

/// v0.20 — checkbox cell for table data rows.
pub(super) fn pad_table_check_cell<'a>(
    on: bool,
    on_toggle: impl Fn(bool) -> PanelMsg + 'a,
) -> iced::Element<'a, PanelMsg> {
    container(
        signex_widgets::checkbox(on)
            .on_toggle(on_toggle)
            .size(TS.label)
            .spacing(0)
            .style(crate::panels::form_look::checkbox_style),
    )
    .padding([SP.xs, 0.0])
    .boxed()
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
    container(cell_text(value, muted, 1))
        .padding([SP.xs, SP.xs])
        .style(move |_: &Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgba(
                1.0, 1.0, 1.0, 0.02,
            ))),
            border: iced::Border {
                width: 1.0,
                radius: 2.0.into(),
                color: border_c,
                ..iced::Border::default()
            },
            ..iced::widget::container::Style::default()
        })
        .width(Length::Fill)
        .boxed()
}

/// v0.20 — static text cell. No chrome, just dim text — used for
/// columns like "Rule Expansion" that aren't yet user-editable
/// (a v0.21 follow-up adds the per-rule override picker).
pub(super) fn pad_table_static_cell<'a>(
    value: impl Into<String>,
    muted: Color,
) -> iced::Element<'a, PanelMsg> {
    container(cell_text(value, muted, 1))
        .padding([SP.xs, SP.xs])
        .width(Length::Fill)
        .boxed()
}

/// v0.20 — single COPPER table row. Built inline because all four
/// data cells (X-Size, Y-Size, Shape, Relief) reference different
/// fields on PadFormValues + different message constructors.
pub(super) fn pad_copper_row<'a>(
    label: &'a str,
    values: &PadFormValues,
    current_shape: PadShapeChoice,
    target: PadEditTarget,
    palette: PanelPalette,
    is_authoritative: bool,
) -> iced::Element<'a, PanelMsg> {
    let PanelPalette {
        muted,
        primary,
        border: border_c,
        ..
    } = palette;
    let x_buf = pad_size_buf(values.size_x_mm);
    let y_buf = pad_size_buf(values.size_y_mm);
    let cells = if is_authoritative {
        vec![
            pad_table_input_cell(x_buf, "", move |v| pad_size_x_msg(target, v)),
            pad_table_input_cell(y_buf, "", move |v| pad_size_y_msg(target, v)),
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
            pad_table_disabled_cell(format!("{:.3}", values.size_x_mm), muted, border_c),
            pad_table_disabled_cell(format!("{:.3}", values.size_y_mm), muted, border_c),
            pad_table_static_cell(format!("{current_shape}"), muted),
            pad_table_disabled_cell("", muted, border_c),
        ]
    };
    pad_table_row(label, cells, &COPPER_CELLS, primary, border_c)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A zero size shows an empty field the user can type into; any other
    /// size shows to 3 decimals.
    #[test]
    fn a_size_cell_is_empty_at_zero_and_three_decimals_otherwise() {
        assert_eq!(pad_size_buf(0.0), "");
        assert_eq!(pad_size_buf(1.55), "1.550");
        assert_eq!(pad_size_buf(-0.6), "-0.600");
    }

    /// Number cells get at least the share of the checkbox column, so a
    /// value like "1.550" is not cut while a tick box has room to spare.
    #[test]
    fn number_cells_are_never_narrower_than_the_checkbox_column() {
        for cells in [&COPPER_CELLS[..], &HOLE_CELLS, &PASTE_CELLS, &SOLDER_CELLS] {
            let (numbers, check) = cells.split_at(cells.len() - 1);
            assert!(numbers.iter().all(|share| *share >= check[0]), "{cells:?}");
        }
    }
}
