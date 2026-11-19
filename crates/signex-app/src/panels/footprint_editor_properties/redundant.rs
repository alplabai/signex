//! #1649 — the "Redundant" rows of the Sketch solve section: consistent
//! constraints that restate earlier ones. Advisory (the warning tone), not a
//! conflict.

use iced::widget::{Column, Widget, container};
use iced::{Element, Length, Theme};
use signex_i18n::{ids, t};
use signex_widgets::{button, text};

use super::super::{FootprintSolveSummary, PanelMsg};
use super::body::{Ctx, kv_row};
use crate::styles::{PillTone, Spacing, TypeScale, pill_colors};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;
/// Rows listed before the "+N more" footer.
const MAX_ROWS: usize = 8;

pub(super) fn push_redundant_rows<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    s: &'a FootprintSolveSummary,
    cx: Ctx<'a, '_>,
) -> Column<Element<'a, PanelMsg>> {
    let i18n = cx.i18n;
    col = col.push(kv_row(
        &t!(i18n, ids::EDITORS_FP_PROPS_REDUNDANT),
        i18n.count(s.redundant_constraints.len() as i64),
        cx.tokens,
        i18n,
    ));
    let (_, warn) = pill_colors(cx.tokens, PillTone::Warn);
    for rc in s.redundant_constraints.iter().take(MAX_ROWS) {
        let label = t!(i18n, ids::EDITORS_FP_PROPS_REDUNDANT_ROW, "kind" => rc.kind_label);
        let label = text(label).size(TS.caption).color(warn);
        let row: Element<'a, PanelMsg> = match rc.focus_entity_id {
            Some(id) => button(label)
                .padding([SP.xs, SP.m])
                .width(Length::Fill)
                .on_press(PanelMsg::FpEditorSelectSketchEntity { id })
                .style(|_t: &Theme, _| iced::widget::button::Style::default())
                .boxed(),
            None => container(label)
                .padding([SP.xs, SP.m])
                .width(Length::Fill)
                .boxed(),
        };
        col = col.push(row);
    }
    if s.redundant_constraints.len() > MAX_ROWS {
        col = col.push(super::sections::more_rows(
            cx,
            s.redundant_constraints.len() - MAX_ROWS,
        ));
    }
    col
}
