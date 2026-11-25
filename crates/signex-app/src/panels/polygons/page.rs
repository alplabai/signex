//! The tab's frame (mockup row 178): the shared document-tab shell with the
//! toolbar, the sub-tabs and a footer, around the filters, the pour table
//! and, on the right, the selected pour with the issues of its fill. Bulk
//! edit puts the fields the ticked pours share over the table of them.

use iced::widget::{Column, column, container, pick_list, row, scrollable};
use iced::{Alignment, Element, Length, Widget as _};
use signex_i18n::{I18n, MessageId, UserText, ids, t};
use signex_widgets::doc_tab_shell::{DocTabShell, DocTabShellStyle, Footer, SubTab, SubTabs};
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::{Label, checkbox, text, text_input};

use super::widgets::{area_text, net_label, state_chip};
use super::{
    PolygonFacts, PolygonMsg, PolygonSubTab, PourField, PourRow, PourState, SP, TS, msg,
    shared_dead_copper, shared_value, table,
};
use crate::panels::pcb_primitive_properties::format_number;
use crate::panels::{PanelContext, PanelMsg, empty_state_card, form_look};
use crate::styles::{ti, tokens::resolve};

/// The page's share of the width for the table, and for the detail.
const TABLE_SHARE: u16 = 3;
const DETAIL_SHARE: u16 = 1;

/// The tab's content, or its empty state without a board or a pour.
pub fn view(ctx: &PanelContext) -> Element<'_, PanelMsg> {
    let Some(facts) = ctx.polygons.as_ref() else {
        return empty_state_card(
            &ctx.tokens,
            t!(ctx.i18n, ids::POLYGON_MANAGER_NO_BOARD_TITLE),
            t!(ctx.i18n, ids::POLYGON_MANAGER_NO_BOARD),
            None,
        );
    };
    let body = if facts.rows.is_empty() {
        empty_state_card(
            &ctx.tokens,
            t!(ctx.i18n, ids::POLYGON_MANAGER_NONE_TITLE),
            t!(ctx.i18n, ids::POLYGON_MANAGER_NONE),
            None,
        )
    } else {
        match sub_tab(ctx, facts) {
            PolygonSubTab::Pours | PolygonSubTab::Shelved => pours_page(ctx, facts),
            PolygonSubTab::BulkEdit => bulk_page(ctx, facts),
        }
    };
    DocTabShell::new(body, DocTabShellStyle::from_tokens(&ctx.tokens, TS.body))
        .toolbar(super::toolbar::view(ctx, facts))
        .sub_tabs(sub_tabs(ctx, facts))
        .footer(footer(ctx))
        .view()
}

/// The page in view: the one picked for this board, else Pours.
fn sub_tab(ctx: &PanelContext, facts: &PolygonFacts) -> PolygonSubTab {
    let ui = &ctx.pcb_panel.polygons;
    if ui.is_for(&facts.board) {
        ui.sub_tab
    } else {
        PolygonSubTab::default()
    }
}

fn sub_tabs<'a>(ctx: &'a PanelContext, facts: &'a PolygonFacts) -> SubTabs<'a, PanelMsg> {
    let tabs = PolygonSubTab::ALL
        .into_iter()
        .map(|tab| SubTab::new(t!(ctx.i18n, tab.message())))
        .collect();
    let current = sub_tab(ctx, facts);
    let selected = PolygonSubTab::ALL
        .iter()
        .position(|tab| *tab == current)
        .unwrap_or_default();
    let board = facts.board.clone();
    SubTabs::new(tabs, selected).on_select(move |index| {
        msg(PolygonMsg::SubTabPicked {
            board: board.clone(),
            tab: PolygonSubTab::ALL.get(index).copied().unwrap_or_default(),
        })
    })
}

fn footer<'a>(ctx: &'a PanelContext) -> Footer<'a> {
    Footer::new(t!(ctx.i18n, ids::POLYGON_MANAGER_FOOTER))
        .note(t!(ctx.i18n, ids::POLYGON_MANAGER_FOOTER_NOTE))
}

/// Filters and table on the left, the selected pour on the right.
fn pours_page<'a>(ctx: &'a PanelContext, facts: &'a PolygonFacts) -> Element<'a, PanelMsg> {
    let shown = ctx.pcb_panel.polygons.tab_rows(facts);
    let mut left: Column<Element<'a, PanelMsg>> = column![filters(ctx, facts)]
        .spacing(SP.s)
        .padding([SP.s, 0.0])
        .width(Length::Fill);
    if shown.is_empty() {
        left = left.push(note(ctx, ids::POLYGON_MANAGER_NO_MATCH));
    } else {
        left = left.push(table::view(ctx, facts, &shown));
    }
    let left = scrollable(left)
        .width(Length::FillPortion(TABLE_SHARE))
        .height(Length::Fill);
    let right = container(detail(ctx, facts))
        .padding(SP.m)
        .width(Length::FillPortion(DETAIL_SHARE))
        .height(Length::Fill);
    row![left, right].height(Length::Fill).boxed()
}

/// A muted line of the page.
fn note<'a>(ctx: &PanelContext, id: MessageId) -> Element<'a, PanelMsg> {
    container(
        text(t!(ctx.i18n, id))
            .size(TS.dense)
            .color(resolve(&ctx.tokens).ink3),
    )
    .padding([SP.xs, SP.l])
    .boxed()
}

/// One entry of a filter list: its value and what it reads.
#[derive(Debug, Clone, PartialEq)]
struct Choice<T> {
    value: T,
    label: String,
}

/// A filter's label and list; the first entry is "all".
fn filter_list<'a, T: Clone + PartialEq + 'a>(
    ctx: &PanelContext,
    title: MessageId,
    options: Vec<Choice<T>>,
    current: &T,
    on_pick: impl Fn(T) -> PanelMsg + 'a,
) -> Element<'a, PanelMsg> {
    let picked = options.iter().find(|c| c.value == *current).cloned();
    let list = pick_list(picked, options, |c: &Choice<T>| c.label.clone())
        .graphite()
        .text_size(TS.dense)
        .padding([SP.xs, SP.s])
        .on_select(move |c: Choice<T>| on_pick(c.value));
    row![
        text(t!(ctx.i18n, title))
            .size(TS.dense)
            .color(resolve(&ctx.tokens).ink3),
        list,
    ]
    .spacing(SP.xs)
    .align_y(Alignment::Center)
    .boxed()
}

/// "All …" (`all`), then one entry per name.
fn named_choices(i18n: &I18n, all: MessageId, names: Vec<String>) -> Vec<Choice<Option<String>>> {
    let mut choices = vec![Choice {
        value: None,
        label: t!(i18n, all).into_string(),
    }];
    choices.extend(names.into_iter().map(|name| Choice {
        label: name.clone(),
        value: Some(name),
    }));
    choices
}

/// "All", then each state.
fn state_choices(i18n: &I18n) -> Vec<Choice<Option<PourState>>> {
    let mut choices = vec![Choice {
        value: None,
        label: t!(i18n, ids::POLYGON_MANAGER_ALL_STATES).into_string(),
    }];
    choices.extend(PourState::ALL.into_iter().map(|state| Choice {
        value: Some(state),
        label: t!(i18n, state.message()).into_string(),
    }));
    choices
}

/// The Layer, Net and State filters.
fn filters<'a>(ctx: &'a PanelContext, facts: &'a PolygonFacts) -> Element<'a, PanelMsg> {
    let ui = &ctx.pcb_panel.polygons;
    let ours = ui.is_for(&facts.board);
    let i18n = &*ctx.i18n;
    let layers = named_choices(i18n, ids::POLYGON_MANAGER_ALL_LAYERS, facts.layers());
    let nets = named_choices(i18n, ids::POLYGON_MANAGER_ALL_NETS, facts.nets());
    let states = state_choices(i18n);
    let layer = ui.layer.clone().filter(|_| ours);
    let net = ui.net.clone().filter(|_| ours);
    let state = ui.state.filter(|_| ours);
    let (b1, b2, b3) = (
        facts.board.clone(),
        facts.board.clone(),
        facts.board.clone(),
    );
    row![
        filter_list(ctx, ids::PCB_PRIM_LAYER, layers, &layer, move |layer| {
            msg(PolygonMsg::LayerFilterPicked {
                board: b1.clone(),
                layer,
            })
        }),
        filter_list(ctx, ids::PCB_PRIM_NET, nets, &net, move |net| {
            msg(PolygonMsg::NetFilterPicked {
                board: b2.clone(),
                net,
            })
        }),
        filter_list(
            ctx,
            ids::POLYGON_MANAGER_COL_STATE,
            states,
            &state,
            move |state| {
                msg(PolygonMsg::StateFilterPicked {
                    board: b3.clone(),
                    state,
                })
            }
        ),
    ]
    .spacing(SP.m)
    .padding([0.0, SP.l])
    .align_y(Alignment::Center)
    .boxed()
}

/// A label and its value, one line of the detail.
fn fact<'a>(ctx: &PanelContext, label: MessageId, shown: Label<'a>) -> Element<'a, PanelMsg> {
    row![
        container(
            text(t!(ctx.i18n, label))
                .size(TS.dense)
                .color(resolve(&ctx.tokens).ink3)
        )
        .width(Length::FillPortion(1)),
        container(text(shown).size(TS.dense).color(ti(ctx.tokens.text)))
            .width(Length::FillPortion(1)),
    ]
    .spacing(SP.s)
    .boxed()
}

/// A plain value as a detail line's value.
fn plain<'a>(shown: String) -> Label<'a> {
    UserText::new(shown).into()
}

/// The selected pour: what it is, what its fill holds and the issues of
/// that fill; a hint while none is selected.
fn detail<'a>(ctx: &'a PanelContext, facts: &'a PolygonFacts) -> Element<'a, PanelMsg> {
    let Some(pour) = ctx.pcb_panel.polygons.current_in(facts) else {
        return note(ctx, ids::POLYGON_MANAGER_DETAIL_NONE);
    };
    let i18n = &*ctx.i18n;
    let copper = match (pour.area_mm2, pour.pieces) {
        (Some(area), Some(pieces)) => t!(
            i18n,
            ids::PCB_PRIM_POUR_COPPER_VALUE,
            "area" => area_text(area),
            "pieces" => pieces
        ),
        _ => t!(i18n, ids::POLYGON_MANAGER_NONE_VALUE),
    };
    let heading = text(UserText::new(pour.name.clone()))
        .size(TS.body)
        .color(ti(ctx.tokens.text));
    let col = column![
        row![heading, state_chip(i18n, &ctx.tokens, pour.state)]
            .spacing(SP.s)
            .align_y(Alignment::Center),
        fact(ctx, ids::PCB_PRIM_NET, plain(net_label(i18n, pour))),
        fact(ctx, ids::PCB_PRIM_LAYER, plain(pour.layer.clone())),
        fact(ctx, ids::PCB_PRIM_POUR_COPPER, copper.into()),
        fact(
            ctx,
            ids::POLYGON_MANAGER_ISLANDS_REMOVED,
            plain(
                pour.islands_removed
                    .map_or_else(String::new, |n| n.to_string())
            ),
        ),
        text(t!(i18n, ids::POLYGON_MANAGER_ISSUES))
            .size(TS.dense)
            .color(resolve(&ctx.tokens).ink3),
        issues(ctx, pour),
    ]
    .spacing(SP.s);
    scrollable(col).height(Length::Fill).boxed()
}

/// The issues of the pour's fill: why it has none, or none.
fn issues<'a>(ctx: &PanelContext, pour: &PourRow) -> Element<'a, PanelMsg> {
    let line = match pour.unfilled {
        Some(why) => {
            let why = t!(ctx.i18n, why).into_string();
            t!(ctx.i18n, ids::PCB_PRIM_POUR_UNFILLED, "why" => why)
        }
        None => t!(ctx.i18n, ids::POLYGON_MANAGER_NO_ISSUES),
    };
    text(line).size(TS.dense).color(ti(ctx.tokens.text)).boxed()
}

/// Bulk edit: the fields the ticked pours share, then the table of them.
fn bulk_page<'a>(ctx: &'a PanelContext, facts: &'a PolygonFacts) -> Element<'a, PanelMsg> {
    let ticked = ctx.pcb_panel.polygons.selected_in(facts);
    let rows: Vec<&PourRow> = facts
        .rows
        .iter()
        .filter(|row| ticked.contains(&row.zone))
        .collect();
    if rows.len() < 2 {
        return note(ctx, ids::POLYGON_MANAGER_BULK_PICK);
    }
    let hint = text(t!(ctx.i18n, ids::POLYGON_MANAGER_BULK_HINT, "count" => rows.len()))
        .size(TS.dense)
        .color(resolve(&ctx.tokens).ink3);
    let page = column![
        container(hint).padding([0.0, SP.l]),
        bulk_bar(ctx, facts, &rows),
        table::view(ctx, facts, &rows),
    ]
    .spacing(SP.s)
    .padding([SP.s, 0.0])
    .width(Length::Fill);
    scrollable(page).height(Length::Fill).boxed()
}

/// One field per typed column and the Dead copper tick box. A field the
/// pours do not share is empty with "mixed" as its placeholder.
fn bulk_bar<'a>(
    ctx: &'a PanelContext,
    facts: &'a PolygonFacts,
    rows: &[&PourRow],
) -> Element<'a, PanelMsg> {
    let ui = &ctx.pcb_panel.polygons;
    let mut line = row![].spacing(SP.m).align_y(Alignment::Center);
    for field in PourField::ALL {
        let shared = shared_value(rows, field);
        let shown = ui
            .bulk_drafts
            .get(&field)
            .cloned()
            .or_else(|| shared.map(format_number))
            .unwrap_or_default();
        let placeholder = t!(ctx.i18n, ids::POLYGON_MANAGER_MIXED).into_string();
        let mut input = text_input(UserText::new(placeholder), shown)
            .size(TS.dense)
            .padding(SP.xs)
            .style(form_look::text_input_style)
            .width(Length::Fixed(TS.body * 6.0));
        if !facts.read_only {
            let board = facts.board.clone();
            let submitted = msg(PolygonMsg::BulkSubmitted {
                board: board.clone(),
                field,
            });
            input = input
                .on_input(move |text| {
                    msg(PolygonMsg::BulkTyped {
                        board: board.clone(),
                        field,
                        text,
                    })
                })
                .on_submit(submitted);
        }
        line = line.push(labelled(ctx, field.message(), input.boxed()));
    }
    line = line.push(labelled(
        ctx,
        ids::POLYGON_MANAGER_COL_DEAD,
        bulk_dead(facts, rows),
    ));
    container(line).padding([0.0, SP.l]).boxed()
}

fn labelled<'a>(
    ctx: &PanelContext,
    label: MessageId,
    field: Element<'a, PanelMsg>,
) -> Element<'a, PanelMsg> {
    row![
        text(t!(ctx.i18n, label))
            .size(TS.dense)
            .color(resolve(&ctx.tokens).ink3),
        field,
    ]
    .spacing(SP.xs)
    .align_y(Alignment::Center)
    .boxed()
}

/// Dead copper for every ticked pour: on when they all remove it; a press
/// sets it on all of them, on unless they all have it already.
fn bulk_dead<'a>(facts: &PolygonFacts, rows: &[&PourRow]) -> Element<'a, PanelMsg> {
    let shared = shared_dead_copper(rows);
    let mut tick = checkbox(shared == Some(true))
        .size(TS.body)
        .style(if shared.is_some() {
            form_look::checkbox_style
        } else {
            form_look::mixed_checkbox_style
        });
    if !facts.read_only {
        let board = facts.board.clone();
        tick = tick.on_toggle(move |on| {
            msg(PolygonMsg::BulkDeadCopperToggled {
                board: board.clone(),
                on,
            })
        });
    }
    tick.boxed()
}
