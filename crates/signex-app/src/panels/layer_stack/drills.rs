//! The Via Types / Drill Pairs sub-tab (mockup row 105): both tables are
//! read from the board's vias, so they follow every via and stack edit. A
//! via's type is picked in the Via panel.

use iced::widget::{Column, Row, column, container, scrollable};
use iced::{Element, Length, Widget as _};
use signex_i18n::{MessageId, UserText, ids, t};
use signex_widgets::text;

use super::page::heading;
use super::{DrillPairRow, LayerStackFacts, SP, StackUnit, TS, ViaTypeRow};
use crate::panels::{PanelContext, PanelMsg};
use crate::styles::{ti, tokens::resolve};

/// The width of a table in body-text ems, scrolled sideways when narrower.
const TABLE_EMS: f32 = 56.0;

/// A column's title; `true` when it names the length unit.
type ColumnTitle = (MessageId, bool);

const PAIR_COLUMNS: [ColumnTitle; 8] = [
    (ids::PCB_STACK_COL_NAME, false),
    (ids::PCB_STACK_COL_FROM, false),
    (ids::PCB_STACK_COL_TO, false),
    (ids::PCB_STACK_COL_KIND, false),
    (ids::PCB_STACK_COL_DEPTH, true),
    (ids::PCB_STACK_COL_MIN_DRILL, true),
    (ids::PCB_STACK_COL_ASPECT, false),
    (ids::PCB_STACK_COL_VIAS, false),
];

const TYPE_COLUMNS: [ColumnTitle; 7] = [
    (ids::PCB_STACK_COL_NAME, false),
    (ids::PCB_STACK_COL_PAIR, false),
    (ids::PCB_STACK_COL_DRILL, true),
    (ids::PCB_STACK_COL_FINISHED, true),
    (ids::PCB_STACK_COL_PAD, true),
    (ids::PCB_STACK_COL_PROTECTION, false),
    (ids::PCB_STACK_COL_USED, false),
];

pub(super) fn view<'a>(ctx: &'a PanelContext, facts: &'a LayerStackFacts) -> Element<'a, PanelMsg> {
    let pairs = facts
        .drill_pairs
        .iter()
        .map(|row| pair_cells(ctx, facts.unit, row))
        .collect();
    let types = facts
        .via_types
        .iter()
        .map(|row| type_cells(facts.unit, row))
        .collect();
    scrollable(
        column![
            heading(
                ctx,
                ids::PCB_STACK_DRILL_PAIRS,
                ids::PCB_STACK_DRILL_PAIRS_HINT
            ),
            table(ctx, facts.unit, &PAIR_COLUMNS, pairs),
            heading(ctx, ids::PCB_STACK_VIA_TYPES, ids::PCB_STACK_VIA_TYPES_HINT),
            table(ctx, facts.unit, &TYPE_COLUMNS, types),
        ]
        .spacing(SP.m)
        .padding(SP.l)
        .width(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .boxed()
}

fn dash(ctx: &PanelContext) -> String {
    t!(ctx.i18n, ids::PCB_STACK_NONE).into_string()
}

fn pair_cells(ctx: &PanelContext, unit: StackUnit, row: &DrillPairRow) -> Vec<String> {
    let length = |mm: Option<f64>| mm.map_or_else(|| dash(ctx), |mm| unit.show(mm));
    let aspect = row.aspect_ratio.map_or_else(
        || dash(ctx),
        |ratio| {
            let value = format!("{ratio:.2}");
            t!(ctx.i18n, ids::PCB_STACK_ASPECT_VALUE, "value" => value).into_string()
        },
    );
    vec![
        row.name.clone(),
        row.from.clone(),
        row.to.clone(),
        row.kind.clone(),
        length(row.depth_mm),
        length(row.min_drill_mm),
        aspect,
        row.vias.to_string(),
    ]
}

fn type_cells(unit: StackUnit, row: &ViaTypeRow) -> Vec<String> {
    vec![
        row.name.clone(),
        row.pair.clone(),
        unit.show(row.drill_mm),
        unit.show(row.finished_mm),
        unit.show(row.pad_mm),
        row.protection.clone(),
        row.used.to_string(),
    ]
}

/// A header row and one row of plain cells per entry, in equal columns.
fn table<'a>(
    ctx: &'a PanelContext,
    unit: StackUnit,
    columns: &[ColumnTitle],
    rows: Vec<Vec<String>>,
) -> Element<'a, PanelMsg> {
    let muted = resolve(&ctx.tokens).ink3;
    let ink = ti(ctx.tokens.text);
    let unit_name = unit.label(&ctx.i18n).into_string();
    let mut header: Row<Element<'a, PanelMsg>> = Row::new().spacing(SP.s);
    for &(id, has_unit) in columns {
        let title = if has_unit {
            t!(ctx.i18n, id, "unit" => unit_name.clone())
        } else {
            t!(ctx.i18n, id)
        };
        header = header.push(
            text(title)
                .size(TS.dense)
                .color(muted)
                .width(Length::Fill)
                .boxed(),
        );
    }
    let mut lines: Column<Element<'a, PanelMsg>> =
        Column::new().spacing(SP.xs).push(header.boxed());
    for cells in rows {
        let mut line: Row<Element<'a, PanelMsg>> = Row::new().spacing(SP.s);
        for cell in cells {
            line = line.push(
                text(UserText::new(cell))
                    .size(TS.dense)
                    .color(ink)
                    .width(Length::Fill)
                    .boxed(),
            );
        }
        lines = lines.push(line.boxed());
    }
    let sized = container(lines).width(Length::Fixed(TS.body * TABLE_EMS));
    scrollable(sized)
        .direction(iced::widget::scrollable::Direction::Horizontal(
            iced::widget::scrollable::Scrollbar::new().spacing(SP.xs),
        ))
        .width(Length::Fill)
        .boxed()
}
