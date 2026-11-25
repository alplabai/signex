//! The PCB panel's Polygons mode (mockup row 106): the compact half of the
//! Polygon Manager pair. A header with Repour all and Open Polygon Manager,
//! the All / Needs attention chips, then one line per pour (name, net,
//! layer, state chip, Repour and Shelve / Unshelve). A click on a line
//! picks the same pour the tab shows; the panel stays live while routing,
//! since it reads the board's rows each frame.

use iced::widget::{Column, Row, column, container, row, scrollable};
use iced::{Alignment, Element, Length, Widget as _};
use signex_i18n::{UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::chips::{ChipMetrics, ChipStyle, chip};
use signex_widgets::{button, text};

use super::widgets::{action, net_label, state_chip};
use super::{PolygonFacts, PolygonMsg, PourRow, PourState, SP, TS, msg};
use crate::panels::paired::PairedHeader;
use crate::panels::{PanelContext, PanelMsg};
use crate::styles::{ti, tokens::resolve};

fn chip_style(tokens: &ThemeTokens) -> ChipStyle {
    let r = resolve(tokens);
    ChipStyle {
        fill: r.field,
        edge: ti(tokens.border),
        hover: ti(tokens.hover),
        ink: r.ink3,
        on_fill: r.accent_soft,
        on_ink: r.accent_soft_ink,
    }
}

/// The mode for the board in view, or why there is nothing to list.
pub fn view(ctx: &PanelContext) -> Element<'_, PanelMsg> {
    let header = PairedHeader {
        title: ids::POLYGON_MANAGER_PAGE,
        open_tab: ids::POLYGON_MANAGER_OPEN,
        on_open_tab: msg(PolygonMsg::ManagerOpened),
    }
    .view(&ctx.tokens, &ctx.i18n);
    let Some(facts) = ctx.polygons.as_ref() else {
        return column![header].width(Length::Fill).boxed();
    };
    let list = scrollable(lines(ctx, facts))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(crate::panels::form_look::scrollbar_style);
    column![header, top(ctx, facts), list]
        .width(Length::Fill)
        .height(Length::Fill)
        .boxed()
}

/// The All / Needs attention chips, then Repour all.
fn top<'a>(ctx: &'a PanelContext, facts: &'a PolygonFacts) -> Element<'a, PanelMsg> {
    let repour = (!facts.rows.is_empty()).then(|| {
        msg(PolygonMsg::RepourAllPressed {
            board: facts.board.clone(),
        })
    });
    row![
        chips(ctx, facts),
        iced::widget::Space::new().width(Length::Fill),
        action(
            ctx,
            ids::PCB_PRIM_POUR_REPOUR_ALL,
            repour,
            ids::POLYGON_MANAGER_REPOUR_ALL_TIP,
        ),
    ]
    .align_y(Alignment::Center)
    .padding([SP.s, SP.l])
    .width(Length::Fill)
    .boxed()
}

/// All, and Needs attention (the pours that are not poured), with counts.
fn chips<'a>(ctx: &'a PanelContext, facts: &'a PolygonFacts) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let ui = &ctx.pcb_panel.polygons;
    let attention = ui.attention && ui.is_for(&facts.board);
    let needs = facts
        .rows
        .iter()
        .filter(|r| r.state.needs_attention())
        .count();
    let metrics = ChipMetrics::from_text_size(TS.body);
    let count = |n: usize| Some(u32::try_from(n).unwrap_or(u32::MAX));
    let picked = |on: bool| {
        msg(PolygonMsg::AttentionPicked {
            board: facts.board.clone(),
            on,
        })
    };
    row![
        chip(
            t!(i18n, ids::POLYGON_MANAGER_ALL_CHIP),
            count(facts.rows.len()),
            !attention,
            chip_style(&ctx.tokens),
            metrics,
            i18n,
            Some(picked(false)),
        ),
        chip(
            t!(i18n, ids::POLYGON_MANAGER_NEEDS_ATTENTION),
            count(needs),
            attention,
            chip_style(&ctx.tokens),
            metrics,
            i18n,
            Some(picked(true)),
        ),
    ]
    .spacing(metrics.row_gap)
    .boxed()
}

/// One line per pour the chips keep, or why there is none.
fn lines<'a>(ctx: &'a PanelContext, facts: &'a PolygonFacts) -> Column<Element<'a, PanelMsg>> {
    let shown = ctx.pcb_panel.polygons.panel_rows(facts);
    let mut list: Column<Element<'a, PanelMsg>> = Column::new().width(Length::Fill);
    if shown.is_empty() {
        let id = if facts.rows.is_empty() {
            ids::POLYGON_MANAGER_NONE
        } else {
            ids::POLYGON_MANAGER_NO_MATCH
        };
        list = list.push(
            container(
                text(t!(ctx.i18n, id))
                    .size(TS.body)
                    .color(resolve(&ctx.tokens).ink3),
            )
            .padding([SP.m, SP.l])
            .boxed(),
        );
    }
    for pour in shown {
        list = list.push(line(ctx, facts, pour));
    }
    list
}

/// A pour: a click picks it; Repour and Shelve / Unshelve at the end.
fn line<'a>(
    ctx: &'a PanelContext,
    facts: &PolygonFacts,
    pour: &'a PourRow,
) -> Element<'a, PanelMsg> {
    let ui = &ctx.pcb_panel.polygons;
    let picked = ui.is_for(&facts.board) && ui.current == Some(pour.zone);
    let mono = crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT);
    let muted = resolve(&ctx.tokens).ink3;
    let what = column![
        text(UserText::new(pour.name.as_str()))
            .size(TS.body)
            .font(mono)
            .color(ti(ctx.tokens.text)),
        text(t!(
            ctx.i18n,
            ids::POLYGON_MANAGER_PANEL_WHERE,
            "net" => net_label(&ctx.i18n, pour),
            "layer" => pour.layer.as_str()
        ))
        .size(TS.caption)
        .color(muted),
    ];
    let face = button(what)
        .width(Length::Fill)
        .padding([SP.xs, 0.0])
        .on_press(msg(PolygonMsg::RowClicked {
            board: facts.board.clone(),
            zone: pour.zone,
        }))
        .style(crate::styles::hover_row(&ctx.tokens, picked));
    let shelved = pour.state == PourState::Shelved;
    let (label, tip) = if shelved {
        (ids::PCB_PRIM_POUR_UNSHELVE, ids::PCB_PRIM_POUR_SHELVE_NOTE)
    } else {
        (ids::PCB_PRIM_POUR_SHELVE, ids::PCB_PRIM_POUR_SHELVE_TIP)
    };
    let shelve = (!facts.read_only).then(|| {
        msg(PolygonMsg::ShelveToggled {
            board: facts.board.clone(),
            zone: pour.zone,
            shelved: !shelved,
        })
    });
    let repour = (!shelved).then(|| {
        msg(PolygonMsg::RepourPressed {
            board: facts.board.clone(),
            zone: pour.zone,
        })
    });
    let mut bar: Row<Element<'a, PanelMsg>> =
        row![face, state_chip(&ctx.i18n, &ctx.tokens, pour.state)]
            .spacing(SP.s)
            .align_y(Alignment::Center);
    bar = bar.push(action(
        ctx,
        ids::PCB_PRIM_POUR_REPOUR,
        repour,
        ids::PCB_PRIM_POUR_COPPER_TIP,
    ));
    bar = bar.push(action(ctx, label, shelve, tip));
    container(bar)
        .padding([0.0, SP.l])
        .width(Length::Fill)
        .boxed()
}
