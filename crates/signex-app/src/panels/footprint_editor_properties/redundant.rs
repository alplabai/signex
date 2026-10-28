//! #1649 — the "Redundant" rows of the DOF / Last solve section: consistent
//! constraints that restate earlier ones. Advisory (amber), not a conflict.

use iced::widget::{Column, button, container, text};
use iced::{Color, Element, Length, Theme};

use super::super::{FootprintSolveSummary, PanelMsg};
use super::props_kv_row;
use crate::styles::{Spacing, TypeScale};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;
/// Rows listed before the "+N more" footer.
const MAX_ROWS: usize = 8;

pub(super) fn push_redundant_rows<'a>(
    mut col: Column<'a, PanelMsg>,
    s: &'a FootprintSolveSummary,
    muted: Color,
    input_bg: Color,
    input_bdr: Color,
) -> Column<'a, PanelMsg> {
    col = props_kv_row(
        col,
        muted,
        input_bg,
        input_bdr,
        "Redundant",
        s.redundant_constraints.len().to_string(),
    );
    let amber = Color::from_rgba(1.00, 0.78, 0.35, 1.00);
    for rc in s.redundant_constraints.iter().take(MAX_ROWS) {
        let label = format!("{} — already implied by earlier constraints", rc.kind_label);
        let row: Element<'_, _> = match rc.focus_entity_id {
            Some(id) => button(text(label).size(TS.caption).color(amber))
                .padding([SP.xs, SP.m])
                .width(Length::Fill)
                .on_press(PanelMsg::FpEditorSelectSketchEntity { id })
                .style(|_t: &Theme, _| iced::widget::button::Style::default())
                .into(),
            None => container(text(label).size(TS.caption).color(amber))
                .padding([SP.xs, SP.m])
                .width(Length::Fill)
                .into(),
        };
        col = col.push(row);
    }
    if s.redundant_constraints.len() > MAX_ROWS {
        col = col.push(
            container(
                text(format!(
                    "… +{} more",
                    s.redundant_constraints.len() - MAX_ROWS
                ))
                .size(TS.caption)
                .color(muted),
            )
            .padding([SP.xs, SP.m])
            .width(Length::Fill),
        );
    }
    col
}
