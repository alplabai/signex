//! The stack table (mockup row 105): a header row, then one row per layer.
//! The # tile selects a row and shows its copper place; the other cells are
//! typed in place and applied with Enter. A cell that does not apply to the
//! row's kind shows a dash. Net, Items and Drill pairs are read from the
//! board; Df and Cu % have nothing to read yet and show a dash with a hint.

use iced::widget::scrollable::{Direction, Scrollbar};
use iced::widget::tooltip::Position;
use iced::widget::{Column, Row, container, row, scrollable};
use iced::{Background, Border, Element, Length, Theme, Widget as _};
use signex_i18n::{MessageId, UserText, ids, t};
use signex_types::pcb_stack::{StackLayer, StackLayerKind};
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::{button, text, text_input};

use super::{
    KindChoice, LayerStackFacts, LayerStackMsg, SP, StackField, TS, copper_places, kind_label, msg,
    spans_at,
};
use crate::panels::graphite::{hint_tooltip, selection_fill};
use crate::panels::pcb_primitive_properties::format_number;
use crate::panels::{PanelContext, PanelMsg, form_look};
use crate::styles::{ti, tokens::resolve};

/// A column after the # tile: its title, its share of the row and the hint
/// its title carries.
struct TableColumn {
    title: MessageId,
    portion: u16,
    tip: Option<MessageId>,
}

const fn col(title: MessageId, portion: u16, tip: Option<MessageId>) -> TableColumn {
    TableColumn {
        title,
        portion,
        tip,
    }
}

/// The columns in mockup order, after the # tile. A row's cells come in the
/// same order (`cells`).
const COLUMNS: [TableColumn; 11] = [
    col(ids::PCB_STACK_COL_NAME, 4, None),
    col(ids::PCB_STACK_COL_TYPE, 3, None),
    col(ids::PCB_STACK_COL_MATERIAL, 3, None),
    col(ids::PCB_STACK_COL_THICKNESS, 3, None),
    col(ids::PCB_STACK_COL_WEIGHT, 2, None),
    col(ids::PCB_STACK_COL_DK, 2, Some(ids::PCB_STACK_COL_DK_TIP)),
    col(ids::PCB_STACK_COL_DF, 2, Some(ids::PCB_STACK_COL_DF_TIP)),
    col(ids::PCB_STACK_COL_NET, 3, Some(ids::PCB_STACK_COL_NET_TIP)),
    col(
        ids::PCB_STACK_COL_COPPER,
        2,
        Some(ids::PCB_STACK_COL_COPPER_TIP),
    ),
    col(
        ids::PCB_STACK_COL_ITEMS,
        2,
        Some(ids::PCB_STACK_COL_ITEMS_TIP),
    ),
    col(
        ids::PCB_STACK_COL_DRILL_PAIRS,
        3,
        Some(ids::PCB_STACK_COL_DRILL_PAIRS_TIP),
    ),
];

/// The table width in body-text ems: wide enough for every column, scrolled
/// sideways in a narrow tab.
const TABLE_EMS: f32 = 72.0;

pub(super) fn view<'a>(ctx: &'a PanelContext, facts: &'a LayerStackFacts) -> Element<'a, PanelMsg> {
    let places = copper_places(&facts.stack);
    let mut rows: Column<Element<'a, PanelMsg>> =
        Column::new().spacing(SP.xs).push(header(ctx, facts));
    for (index, layer) in facts.stack.layers.iter().enumerate() {
        let place = places.get(index).copied().flatten();
        rows = rows.push(layer_row(ctx, facts, index, place, layer));
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

/// The # column's width, in body-text ems.
fn number_width() -> Length {
    Length::Fixed(TS.body * 2.0)
}

fn header<'a>(ctx: &'a PanelContext, facts: &LayerStackFacts) -> Element<'a, PanelMsg> {
    let muted = resolve(&ctx.tokens).ink3;
    let number = text(t!(ctx.i18n, ids::PCB_STACK_COL_NUMBER))
        .size(TS.dense)
        .color(muted);
    let mut line: Row<Element<'a, PanelMsg>> =
        row![container(number).width(number_width())].spacing(SP.xs);
    let unit = facts.unit.label(&ctx.i18n).into_string();
    for column in &COLUMNS {
        let title = if column.title == ids::PCB_STACK_COL_THICKNESS {
            t!(ctx.i18n, column.title, "unit" => unit.clone())
        } else {
            t!(ctx.i18n, column.title)
        };
        let title = text(title).size(TS.dense).color(muted).boxed();
        let title = match column.tip {
            Some(tip) => hint_tooltip(title, t!(ctx.i18n, tip), &ctx.tokens, Position::Bottom),
            None => title,
        };
        line = line.push(cell(title, column.portion));
    }
    line.boxed()
}

fn layer_row<'a>(
    ctx: &'a PanelContext,
    facts: &'a LayerStackFacts,
    index: usize,
    place: Option<usize>,
    layer: &'a StackLayer,
) -> Element<'a, PanelMsg> {
    let selected = facts.selected == Some(index);
    let number = place.map_or_else(
        || t!(ctx.i18n, ids::PCB_STACK_NONE).into_string(),
        |place| place.to_string(),
    );
    let number = button(
        text(UserText::new(number))
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
    let mut line: Row<Element<'a, PanelMsg>> =
        row![number].spacing(SP.xs).align_y(iced::Alignment::Center);
    for (content, column) in cells(ctx, facts, index, place, layer)
        .into_iter()
        .zip(&COLUMNS)
    {
        line = line.push(cell(content, column.portion));
    }
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

/// The cells of one row after its # tile, in [`COLUMNS`] order.
fn cells<'a>(
    ctx: &'a PanelContext,
    facts: &'a LayerStackFacts,
    index: usize,
    place: Option<usize>,
    layer: &'a StackLayer,
) -> [Element<'a, PanelMsg>; 11] {
    let name = facts.names.get(index).cloned().unwrap_or_default();
    let thickness = facts.unit.show(layer.thickness_mm);
    let copper = layer.kind.is_copper();
    let insulator = matches!(
        layer.kind,
        StackLayerKind::Dielectric | StackLayerKind::SolderMask
    );
    let weight = if copper {
        field(
            facts,
            index,
            StackField::CopperWeight,
            Some(format_number(layer.copper_weight_oz)),
        )
    } else {
        dash(ctx)
    };
    let dk = if insulator {
        field(
            facts,
            index,
            StackField::DielectricConstant,
            Some(format_number(layer.dielectric_constant)),
        )
    } else {
        dash(ctx)
    };
    let used = facts.copper_use.get(index).and_then(Option::as_ref);
    let items = used.map_or_else(|| dash(ctx), |u| value(ctx, u.items.to_string()));
    let pairs = place.map_or_else(|| dash(ctx), |place| drill_pairs_cell(ctx, facts, place));
    [
        field(facts, index, StackField::Name, Some(name)),
        kind_cell(ctx, facts, index, layer),
        field(
            facts,
            index,
            StackField::Material,
            Some(layer.material.clone()),
        ),
        field(facts, index, StackField::Thickness, Some(thickness)),
        weight,
        dk,
        // Df: the stack stores no loss tangent.
        dash(ctx),
        net_cell(ctx, layer.kind, used.map(|u| u.nets.as_slice())),
        // Cu %: copper coverage is not measured.
        dash(ctx),
        items,
        pairs,
    ]
}

/// A plane row names the nets poured on it; a signal row says signal.
fn net_cell<'a>(
    ctx: &'a PanelContext,
    kind: StackLayerKind,
    nets: Option<&[String]>,
) -> Element<'a, PanelMsg> {
    match (kind, nets) {
        (StackLayerKind::Signal, _) => text(t!(ctx.i18n, ids::PCB_STACK_NET_SIGNAL))
            .size(TS.dense)
            .color(resolve(&ctx.tokens).ink3)
            .boxed(),
        (StackLayerKind::Plane, Some(nets)) if !nets.is_empty() => value(ctx, nets.join(", ")),
        _ => dash(ctx),
    }
}

/// The drill pairs that start or end at the copper `place`, "1–4, 1–2".
fn drill_pairs_cell<'a>(
    ctx: &'a PanelContext,
    facts: &LayerStackFacts,
    place: usize,
) -> Element<'a, PanelMsg> {
    let spans: Vec<String> = spans_at(&facts.drill_pairs, place)
        .into_iter()
        .map(|(from, to)| {
            t!(ctx.i18n, ids::PCB_STACK_SPAN, "from" => from, "to" => to).into_string()
        })
        .collect();
    if spans.is_empty() {
        return dash(ctx);
    }
    value(ctx, spans.join(", "))
}

/// A cell read from the board, not typed.
fn value<'a>(ctx: &'a PanelContext, shown: String) -> Element<'a, PanelMsg> {
    text(UserText::new(shown))
        .size(TS.dense)
        .color(ti(ctx.tokens.text))
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
