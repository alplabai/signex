//! The pour table (mockup row 178): a header, then the pours grouped by
//! layer in pour order, each layer under its name. A tick box marks a row
//! for the toolbar and Bulk edit, a click on the name shows its detail.
//! Clearance, Min width and Islands below are typed in place and applied
//! with Enter; Dead copper is a tick box; Area and Pieces are read from the
//! fill. Repour and Shelve / Unshelve end each row.

use iced::widget::scrollable::{Direction, Scrollbar};
use iced::widget::tooltip::Position;
use iced::widget::{Column, Row, container, row, scrollable};
use iced::{Alignment, Background, Border, Element, Length, Theme, Widget as _};
use signex_i18n::{MessageId, UserText, ids, t};
use signex_widgets::{button, checkbox, text, text_input};

use super::widgets::{action, area_text, dash, net_label, state_chip, value};
use super::{PolygonFacts, PolygonMsg, PourField, PourRow, PourState, SP, TS, msg};
use crate::panels::graphite::{hint_tooltip, selection_fill};
use crate::panels::pcb_primitive_properties::format_number;
use crate::panels::{PanelContext, PanelMsg, form_look};
use crate::styles::{ti, tokens::resolve};

/// A column: its title, its share of the row and the hint its title carries.
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

/// The columns after the tick box, in mockup order; a row's cells come in
/// the same order (`cells`).
const COLUMNS: [TableColumn; 12] = [
    col(
        ids::POLYGON_MANAGER_COL_ORDER,
        2,
        Some(ids::POLYGON_MANAGER_COL_ORDER_TIP),
    ),
    col(ids::POLYGON_MANAGER_COL_NAME, 5, None),
    col(ids::PCB_PRIM_NET, 3, None),
    col(ids::PCB_PRIM_LAYER, 3, None),
    col(ids::POLYGON_MANAGER_COL_STATE, 3, None),
    col(
        ids::PCB_PRIM_CLEARANCE,
        3,
        Some(ids::POLYGON_MANAGER_CLEARANCE_TIP),
    ),
    col(ids::POLYGON_MANAGER_COL_MIN_WIDTH, 3, None),
    col(ids::POLYGON_MANAGER_COL_ISLANDS, 3, None),
    col(ids::POLYGON_MANAGER_COL_DEAD, 2, None),
    col(
        ids::POLYGON_MANAGER_COL_AREA,
        3,
        Some(ids::POLYGON_MANAGER_COL_FACTS_TIP),
    ),
    col(
        ids::POLYGON_MANAGER_COL_PIECES,
        2,
        Some(ids::POLYGON_MANAGER_COL_FACTS_TIP),
    ),
    col(ids::POLYGON_MANAGER_COL_ACTIONS, 5, None),
];

/// The table width in body-text ems: wide enough for every column, scrolled
/// sideways in a narrow tab.
const TABLE_EMS: f32 = 84.0;

pub(super) fn view<'a>(
    ctx: &'a PanelContext,
    facts: &'a PolygonFacts,
    shown: &[&'a PourRow],
) -> Element<'a, PanelMsg> {
    let mut rows: Column<Element<'a, PanelMsg>> =
        Column::new().spacing(SP.xs).push(header(ctx, facts, shown));
    let mut layer: Option<&str> = None;
    for &pour in shown {
        if layer != Some(pour.layer.as_str()) {
            layer = Some(pour.layer.as_str());
            rows = rows.push(group_label(ctx, pour));
        }
        rows = rows.push(pour_row(ctx, facts, pour));
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

/// The tick column's width, in body-text ems.
fn tick_width() -> Length {
    Length::Fixed(TS.body * 2.0)
}

/// The header's tick box: on while every shown row is ticked; a press
/// ticks them all, or none.
fn header_tick<'a>(
    ctx: &PanelContext,
    facts: &PolygonFacts,
    shown: &[&PourRow],
) -> Element<'a, PanelMsg> {
    let ticked = ctx.pcb_panel.polygons.selected_in(facts);
    let all = !shown.is_empty() && shown.iter().all(|row| ticked.contains(&row.zone));
    let zones: Vec<_> = shown.iter().map(|row| row.zone).collect();
    let board = facts.board.clone();
    checkbox(all)
        .size(TS.body)
        .style(form_look::checkbox_style)
        .on_toggle(move |on| {
            msg(PolygonMsg::ShownTicked {
                board: board.clone(),
                zones: zones.clone(),
                on,
            })
        })
        .boxed()
}

fn header<'a>(
    ctx: &'a PanelContext,
    facts: &PolygonFacts,
    shown: &[&PourRow],
) -> Element<'a, PanelMsg> {
    let muted = resolve(&ctx.tokens).ink3;
    let tick = container(header_tick(ctx, facts, shown)).width(tick_width());
    let mut line: Row<Element<'a, PanelMsg>> = row![tick].spacing(SP.xs).align_y(Alignment::Center);
    for column in &COLUMNS {
        let title = text(t!(ctx.i18n, column.title))
            .size(TS.dense)
            .color(muted)
            .boxed();
        let title = match column.tip {
            Some(tip) => hint_tooltip(title, t!(ctx.i18n, tip), &ctx.tokens, Position::Bottom),
            None => title,
        };
        line = line.push(cell(title, column.portion));
    }
    line.boxed()
}

/// The name of a layer over its pours.
fn group_label<'a>(ctx: &PanelContext, pour: &PourRow) -> Element<'a, PanelMsg> {
    let semibold = iced::Font {
        weight: iced::font::Weight::Semibold,
        ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
    };
    container(
        text(UserText::new(pour.layer.clone()))
            .size(TS.dense)
            .font(semibold)
            .color(ti(ctx.tokens.text)),
    )
    .padding([SP.xs, 0.0])
    .boxed()
}

fn pour_row<'a>(
    ctx: &'a PanelContext,
    facts: &'a PolygonFacts,
    pour: &'a PourRow,
) -> Element<'a, PanelMsg> {
    let ui = &ctx.pcb_panel.polygons;
    let ours = ui.is_for(&facts.board);
    let ticked = ours && ui.selected.contains(&pour.zone);
    let current = ours && ui.current == Some(pour.zone);
    let (board, zone) = (facts.board.clone(), pour.zone);
    let tick = checkbox(ticked)
        .size(TS.body)
        .style(form_look::checkbox_style)
        .on_toggle(move |on| {
            msg(PolygonMsg::RowTicked {
                board: board.clone(),
                zone,
                on,
            })
        });
    let mut line: Row<Element<'a, PanelMsg>> = row![container(tick).width(tick_width())]
        .spacing(SP.xs)
        .align_y(Alignment::Center);
    for (content, column) in cells(ctx, facts, pour).into_iter().zip(&COLUMNS) {
        line = line.push(cell(content, column.portion));
    }
    let fill = (ticked || current).then(|| selection_fill(&ctx.tokens));
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

/// The cells of one row after its tick box, in [`COLUMNS`] order.
fn cells<'a>(
    ctx: &'a PanelContext,
    facts: &'a PolygonFacts,
    pour: &'a PourRow,
) -> [Element<'a, PanelMsg>; 12] {
    let order = pour
        .order
        .map_or_else(|| dash(ctx), |order| value(ctx, order.to_string()));
    let area = pour
        .area_mm2
        .map_or_else(|| dash(ctx), |area| value(ctx, area_text(area)));
    let pieces = pour
        .pieces
        .map_or_else(|| dash(ctx), |pieces| value(ctx, pieces.to_string()));
    [
        order,
        name_cell(ctx, facts, pour),
        value(ctx, net_label(&ctx.i18n, pour)),
        value(ctx, pour.layer.clone()),
        state_chip(&ctx.i18n, &ctx.tokens, pour.state),
        field(ctx, facts, pour, PourField::Clearance),
        field(ctx, facts, pour, PourField::MinWidth),
        field(ctx, facts, pour, PourField::IslandsBelow),
        dead_copper(facts, pour),
        area,
        pieces,
        row_actions(ctx, facts, pour),
    ]
}

/// The pour's name; a click shows its detail and ticks it alone.
fn name_cell<'a>(
    ctx: &'a PanelContext,
    facts: &PolygonFacts,
    pour: &'a PourRow,
) -> Element<'a, PanelMsg> {
    let mono = crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT);
    button(
        text(UserText::new(pour.name.as_str()))
            .size(TS.dense)
            .font(mono)
            .color(ti(ctx.tokens.text)),
    )
    .padding([SP.xs, 0.0])
    .style(form_look::ghost_button)
    .on_press(msg(PolygonMsg::RowClicked {
        board: facts.board.clone(),
        zone: pour.zone,
    }))
    .boxed()
}

/// A typed cell: its draft while one is typed, else the stored value. A
/// clearance of 0 follows the board and shows the placeholder. Read-only on
/// a read-only board.
fn field<'a>(
    ctx: &'a PanelContext,
    facts: &'a PolygonFacts,
    pour: &'a PourRow,
    field: PourField,
) -> Element<'a, PanelMsg> {
    let ui = &ctx.pcb_panel.polygons;
    let stored = pour.value(field);
    let follows_board = field == PourField::Clearance && stored == 0.0;
    let shown = ui
        .draft(pour.zone, field)
        .filter(|_| ui.is_for(&facts.board))
        .map(str::to_owned)
        .unwrap_or_else(|| {
            if follows_board {
                String::new()
            } else {
                format_number(stored)
            }
        });
    let placeholder = if follows_board {
        t!(ctx.i18n, ids::POLYGON_MANAGER_CLEARANCE_BOARD).into_string()
    } else {
        String::new()
    };
    let mut input = text_input(UserText::new(placeholder), shown)
        .size(TS.dense)
        .padding(SP.xs)
        .style(form_look::text_input_style)
        .width(Length::Fill);
    if !facts.read_only {
        let (board, zone) = (facts.board.clone(), pour.zone);
        let submitted = msg(PolygonMsg::FieldSubmitted {
            board: board.clone(),
            zone,
            field,
        });
        input = input
            .on_input(move |text| {
                msg(PolygonMsg::FieldTyped {
                    board: board.clone(),
                    zone,
                    field,
                    text,
                })
            })
            .on_submit(submitted);
    }
    input.boxed()
}

fn dead_copper<'a>(facts: &PolygonFacts, pour: &PourRow) -> Element<'a, PanelMsg> {
    let mut tick = checkbox(pour.dead_copper)
        .size(TS.body)
        .style(form_look::checkbox_style);
    if !facts.read_only {
        let (board, zone) = (facts.board.clone(), pour.zone);
        tick = tick.on_toggle(move |on| {
            msg(PolygonMsg::DeadCopperToggled {
                board: board.clone(),
                zone,
                on,
            })
        });
    }
    tick.boxed()
}

/// Repour, and Shelve or Unshelve. Repour is greyed while the pour is
/// shelved, as on its Copper card.
fn row_actions<'a>(
    ctx: &'a PanelContext,
    facts: &PolygonFacts,
    pour: &PourRow,
) -> Element<'a, PanelMsg> {
    let (board, zone) = (facts.board.clone(), pour.zone);
    let shelved = pour.state == PourState::Shelved;
    let (repour_tip, repour) = if shelved {
        (ids::PCB_PRIM_POUR_REPOUR_SHELVED_TIP, None)
    } else {
        let event = PolygonMsg::RepourPressed {
            board: board.clone(),
            zone,
        };
        (ids::PCB_PRIM_POUR_COPPER_TIP, Some(msg(event)))
    };
    let (shelve_label, shelve_tip) = if shelved {
        (ids::PCB_PRIM_POUR_UNSHELVE, ids::PCB_PRIM_POUR_SHELVE_NOTE)
    } else {
        (ids::PCB_PRIM_POUR_SHELVE, ids::PCB_PRIM_POUR_SHELVE_TIP)
    };
    let shelve = (!facts.read_only).then(|| {
        msg(PolygonMsg::ShelveToggled {
            board,
            zone,
            shelved: !shelved,
        })
    });
    row![
        action(ctx, ids::PCB_PRIM_POUR_REPOUR, repour, repour_tip),
        action(ctx, shelve_label, shelve, shelve_tip),
    ]
    .spacing(SP.xs)
    .align_y(Alignment::Center)
    .boxed()
}
