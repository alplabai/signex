//! The stack table: a header row, then one row per layer. A row's number
//! selects it; its cells are typed in place and applied with Enter. A cell
//! that does not apply to the row's kind shows a dash.

use iced::widget::scrollable::{Direction, Scrollbar};
use iced::widget::{Column, Row, container, row, scrollable};
use iced::{Background, Border, Element, Length, Theme, Widget as _};
use signex_i18n::{MessageId, UserText, ids, t};
use signex_types::pcb_stack::{StackLayer, StackLayerKind};
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::{button, text, text_input};

use super::{KindChoice, LayerStackFacts, LayerStackMsg, SP, StackField, TS, kind_label, msg};
use crate::panels::graphite::selection_fill;
use crate::panels::pcb_primitive_properties::format_number;
use crate::panels::{PanelContext, PanelMsg, form_look};
use crate::styles::{ti, tokens::resolve};

/// The columns and their share of the row, after the row number.
const COLUMNS: [(MessageId, u16); 6] = [
    (ids::PCB_STACK_COL_NAME, 4),
    (ids::PCB_STACK_COL_TYPE, 3),
    (ids::PCB_STACK_COL_MATERIAL, 3),
    (ids::PCB_STACK_COL_THICKNESS, 3),
    (ids::PCB_STACK_COL_ER, 2),
    (ids::PCB_STACK_COL_WEIGHT, 2),
];

/// The table width in body-text ems: wide enough for every column, scrolled
/// sideways in a narrow tab.
const TABLE_EMS: f32 = 44.0;

pub(super) fn view<'a>(ctx: &'a PanelContext, facts: &'a LayerStackFacts) -> Element<'a, PanelMsg> {
    let mut rows: Column<Element<'a, PanelMsg>> =
        Column::new().spacing(SP.xs).push(header(ctx, facts));
    for (index, layer) in facts.stack.layers.iter().enumerate() {
        rows = rows.push(layer_row(ctx, facts, index, layer));
    }
    let table = container(rows).width(Length::Fixed(TS.body * TABLE_EMS));
    container(
        scrollable(table)
            .direction(Direction::Horizontal(Scrollbar::new().spacing(SP.xs)))
            .width(Length::Fill),
    )
    .padding([0.0, SP.l])
    .width(Length::Fill)
    .boxed()
}

fn cell<'a>(content: Element<'a, PanelMsg>, portion: u16) -> Element<'a, PanelMsg> {
    container(content)
        .width(Length::FillPortion(portion))
        .clip(true)
        .boxed()
}

/// The row-number column's width, in body-text ems.
fn number_width() -> Length {
    Length::Fixed(TS.body * 2.0)
}

fn header<'a>(ctx: &'a PanelContext, facts: &LayerStackFacts) -> Element<'a, PanelMsg> {
    let muted = resolve(&ctx.tokens).ink3;
    let mut line: Row<Element<'a, PanelMsg>> =
        row![container(iced::widget::Space::new()).width(number_width())].spacing(SP.xs);
    let unit = facts.unit.label(&ctx.i18n).into_string();
    for (id, portion) in COLUMNS {
        let title = if id == ids::PCB_STACK_COL_THICKNESS {
            t!(ctx.i18n, id, "unit" => unit.clone())
        } else {
            t!(ctx.i18n, id)
        };
        line = line.push(cell(
            text(title).size(TS.dense).color(muted).boxed(),
            portion,
        ));
    }
    line.boxed()
}

fn layer_row<'a>(
    ctx: &'a PanelContext,
    facts: &'a LayerStackFacts,
    index: usize,
    layer: &'a StackLayer,
) -> Element<'a, PanelMsg> {
    let selected = facts.selected == Some(index);
    let number = button(
        text(UserText::new((index + 1).to_string()))
            .size(TS.dense)
            .color(ti(ctx.tokens.text)),
    )
    .padding([SP.xs, SP.s])
    .width(number_width())
    .style(form_look::option_tile(selected))
    .on_press(msg(LayerStackMsg::RowSelected {
        board: facts.board.clone(),
        row: index,
    }));
    let copper = layer.kind.is_copper();
    let insulator = matches!(
        layer.kind,
        StackLayerKind::Dielectric | StackLayerKind::SolderMask
    );
    let name = facts.names.get(index).cloned().unwrap_or_default();
    let line = row![
        number,
        cell(field(facts, index, StackField::Name, Some(name)), 4),
        cell(kind_cell(ctx, facts, index, layer), 3),
        cell(
            field(
                facts,
                index,
                StackField::Material,
                Some(layer.material.clone())
            ),
            3
        ),
        cell(
            field(
                facts,
                index,
                StackField::Thickness,
                Some(facts.unit.show(layer.thickness_mm))
            ),
            3
        ),
        cell(
            if insulator {
                field(
                    facts,
                    index,
                    StackField::DielectricConstant,
                    Some(format_number(layer.dielectric_constant)),
                )
            } else {
                dash(ctx)
            },
            2
        ),
        cell(
            if copper {
                field(
                    facts,
                    index,
                    StackField::CopperWeight,
                    Some(format_number(layer.copper_weight_oz)),
                )
            } else {
                dash(ctx)
            },
            2
        ),
    ]
    .spacing(SP.xs)
    .align_y(iced::Alignment::Center);
    let fill = selected.then(|| selection_fill(&ctx.tokens));
    container(line)
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: fill.map(Background::Color),
            border: Border {
                radius: (TS.body / 3.0).into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// A dash in a cell that does not apply.
fn dash<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    text(t!(ctx.i18n, ids::PCB_STACK_NONE))
        .size(TS.dense)
        .color(resolve(&ctx.tokens).ink4)
        .boxed()
}

/// A copper row picks Signal or Plane; any other row shows its kind.
fn kind_cell<'a>(
    ctx: &'a PanelContext,
    facts: &'a LayerStackFacts,
    index: usize,
    layer: &'a StackLayer,
) -> Element<'a, PanelMsg> {
    if !layer.kind.is_copper() {
        return text(kind_label(&ctx.i18n, layer.kind))
            .size(TS.dense)
            .color(ti(ctx.tokens.text))
            .boxed();
    }
    let choice = |kind| KindChoice {
        kind,
        label: kind_label(&ctx.i18n, kind).into_string(),
    };
    let options = vec![
        choice(StackLayerKind::Signal),
        choice(StackLayerKind::Plane),
    ];
    let list = iced::widget::pick_list(Some(choice(layer.kind)), options, |c: &KindChoice| {
        c.to_string()
    })
    .graphite()
    .text_size(TS.dense)
    .padding(SP.xs)
    .width(Length::Fill);
    if facts.read_only {
        return list.boxed();
    }
    let board = facts.board.clone();
    list.on_select(move |c: KindChoice| {
        msg(LayerStackMsg::KindPicked {
            board: board.clone(),
            row: index,
            kind: c.kind,
        })
    })
    .boxed()
}

/// A typed cell: its draft while one is typed, else `value`. Read-only on a
/// read-only board.
fn field<'a>(
    facts: &'a LayerStackFacts,
    index: usize,
    field: StackField,
    value: Option<String>,
) -> Element<'a, PanelMsg> {
    let shown = facts
        .drafts
        .get(&(index, field))
        .cloned()
        .or(value)
        .unwrap_or_default();
    let mut input = text_input(UserText::new(String::new()), shown)
        .size(TS.dense)
        .padding(SP.xs)
        .style(form_look::text_input_style)
        .width(Length::Fill);
    if !facts.read_only {
        let board = facts.board.clone();
        let submitted = msg(LayerStackMsg::FieldSubmitted {
            board: board.clone(),
            row: index,
            field,
        });
        input = input
            .on_input(move |text| {
                msg(LayerStackMsg::FieldTyped {
                    board: board.clone(),
                    row: index,
                    field,
                    text,
                })
            })
            .on_submit(submitted);
    }
    input.boxed()
}
