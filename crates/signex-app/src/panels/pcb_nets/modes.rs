//! The PCB panel's other modes (mockup row 106): Net Classes, Differential
//! Pairs, xSignals and Polygons, each a list over the selected item's
//! detail on the page the Nets mode uses.

use std::path::Path;

use iced::widget::{Column, Row, Space, column, container, row};
use iced::{Element, Length, Widget as _};
use pcb_geom::{ItemRef, PickKind};
use signex_i18n::{UserText, ids, t};
use signex_types::format::parse_pcb_layer;
use signex_types::net_class::first_matching_class;
use signex_widgets::chips::{ChipMetrics, chip};
use signex_widgets::{Label, button, text};

use super::detail::{layer_text, length_value, pair_rules};
use super::model::{PcbDiffPairRow, PcbNetsSummary, PcbPanelMsg, PcbPourChip, PcbPourRow};
use super::table::{chip_style, foot, note, page};
use super::{SP, TS, board_classes, msg, separator};
use crate::panels::pcb_primitive_properties::{PourMsg, PrimitiveTarget};
use crate::panels::{PanelContext, PanelMsg, PcbPrimitiveMsg, board_net_classes};
use crate::pcb_canvas::layer_style::layer_colour;
use crate::styles::{DialogButtonKind, dialog_button, ti, tokens::resolve};

fn semibold() -> iced::Font {
    iced::Font {
        weight: iced::font::Weight::Semibold,
        ..signex_widgets::ui_font::regular()
    }
}

/// Net Classes: the class list over the selected class's rules and members.
pub(super) fn classes_mode<'a>(
    ctx: &'a PanelContext,
    summary: &'a PcbNetsSummary,
) -> Element<'a, PanelMsg> {
    let view = match board_net_classes::table_in_view(ctx) {
        Ok(view) => view,
        Err(why) => return why,
    };
    let detail = ctx
        .pcb_panel
        .selected_class(&view.classes)
        .map(|class| board_net_classes::class_detail(ctx, view, class, &summary.nets));
    let head = vec![board_net_classes::list_head(ctx, view), separator(ctx)];
    let count = t!(&*ctx.i18n, ids::PANELS_PCB_CLASSES_SUMMARY, "count" => view.classes.len());
    page(
        ctx,
        head,
        board_net_classes::class_list(ctx, view),
        detail,
        foot(ctx, count.into()),
    )
}

/// One pair: its name and skew, each half's net and routed length, and the
/// gap and width it keeps. A click selects it.
fn pair_row<'a>(ctx: &'a PanelContext, pair: &'a PcbDiffPairRow) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let muted = resolve(&ctx.tokens).ink3;
    let caption = |label: Label<'a>| text(label).size(TS.caption).color(muted).boxed();
    let class = first_matching_class(board_classes(ctx), &pair.positive);
    let (gap, width) = pair_rules(i18n, class);
    let skew = t!(i18n, ids::PANELS_PCB_PAIRS_SKEW, "skew" => format!("{:.2}", pair.skew_mm()));
    let half = |id: signex_i18n::MessageId, net: &str, mm: f64| -> Label<'a> {
        t!(i18n, id, "net" => net.to_owned(), "length" => length_value(i18n, mm)).into()
    };
    let lines = column![
        row![
            text(UserText::new(pair.name.as_str()))
                .size(TS.body)
                .font(semibold())
                .color(ti(ctx.tokens.text))
                .boxed(),
            Space::new().width(Length::Fill).boxed(),
            caption(skew.into()),
        ]
        .spacing(SP.s)
        .align_y(iced::Alignment::Center)
        .boxed(),
        row![
            caption(half(
                ids::PANELS_PCB_PAIRS_POSITIVE,
                &pair.positive,
                pair.positive_mm
            )),
            caption(half(
                ids::PANELS_PCB_PAIRS_NEGATIVE,
                &pair.negative,
                pair.negative_mm
            )),
        ]
        .spacing(SP.m)
        .boxed(),
        caption(t!(i18n, ids::PANELS_PCB_PAIRS_RULES, "gap" => gap, "width" => width).into()),
    ]
    .spacing(SP.xs);
    let is_selected = ctx.pcb_panel.pair.as_deref() == Some(pair.name.as_str());
    button(lines)
        .width(Length::Fill)
        .padding([SP.s, SP.l])
        .on_press(msg(PcbPanelMsg::PairClicked(pair.name.clone())))
        .style(crate::styles::hover_row(&ctx.tokens, is_selected))
        .boxed()
}

/// Differential Pairs: the pairs the router finds, over the selected one.
pub(super) fn pairs_mode<'a>(
    ctx: &'a PanelContext,
    summary: &'a PcbNetsSummary,
) -> Element<'a, PanelMsg> {
    let mut list: Column<Element<'a, PanelMsg>> = Column::new().width(Length::Fill);
    if summary.pairs.is_empty() {
        list = list.push(note(ctx, ids::PANELS_PCB_PAIRS_EMPTY));
    }
    for pair in &summary.pairs {
        list = list.push(pair_row(ctx, pair));
        list = list.push(separator(ctx));
    }
    let detail = ctx
        .pcb_panel
        .selected_pair(summary)
        .map(|pair| super::detail::pair(ctx, pair));
    let count = t!(&*ctx.i18n, ids::PANELS_PCB_PAIRS_SUMMARY, "count" => summary.pairs.len());
    page(ctx, Vec::new(), list, detail, foot(ctx, count.into()))
}

/// xSignals: the board stores none yet, so the list says so.
pub(super) fn xsignals_mode<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let list = Column::new()
        .width(Length::Fill)
        .push(note(ctx, ids::PANELS_PCB_XSIGNALS_EMPTY));
    let count = t!(&*ctx.i18n, ids::PANELS_PCB_XSIGNALS_SUMMARY, "count" => 0_usize);
    page(ctx, Vec::new(), list, None, foot(ctx, count.into()))
}

fn pour_msg(event: PourMsg) -> PanelMsg {
    PanelMsg::PcbPrimitive(PcbPrimitiveMsg::Pour(event))
}

/// The pour on `board`, as the Copper card of the pour names it.
fn pour_target(board: &Path, pour: &PcbPourRow) -> PrimitiveTarget {
    PrimitiveTarget {
        board: board.to_path_buf(),
        items: vec![ItemRef::Zone {
            zone: pour.zone,
            kind: PickKind::Polygons,
        }],
    }
}

/// Repour all and Open Polygon Manager; the tab it opens is not built yet,
/// so the button is off and says so.
fn pours_toolbar<'a>(ctx: &'a PanelContext, board: Option<&Path>) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let repour_all = board.map(|board| {
        pour_msg(PourMsg::RepourAllPressed {
            board: board.to_path_buf(),
        })
    });
    // The Polygon Manager tab of the board in view (mockup row 178).
    let open_manager = board.map(|_| {
        msg(PcbPanelMsg::Polygons(
            crate::panels::PolygonMsg::ManagerOpened,
        ))
    });
    let manager = dialog_button(
        t!(i18n, ids::PANELS_PCB_POURS_OPEN_MANAGER),
        DialogButtonKind::Secondary,
        open_manager,
        &ctx.tokens,
    );
    row![
        dialog_button(
            t!(i18n, ids::PCB_PRIM_POUR_REPOUR_ALL),
            DialogButtonKind::Primary,
            repour_all,
            &ctx.tokens,
        ),
        Space::new().width(Length::Fill).boxed(),
        manager,
    ]
    .spacing(SP.s)
    .padding([SP.s, SP.l])
    .align_y(iced::Alignment::Center)
    .boxed()
}

/// The All and Needs attention chips with their counts.
fn pour_chips<'a>(ctx: &'a PanelContext, summary: &PcbNetsSummary) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let metrics = ChipMetrics::from_text_size(TS.body);
    let attention = summary.pours.iter().filter(|p| p.needs_attention()).count();
    let chips = [
        (
            PcbPourChip::All,
            ids::PANELS_PCB_NETS_CHIP_ALL,
            summary.pours.len(),
        ),
        (
            PcbPourChip::NeedsAttention,
            ids::PANELS_PCB_POURS_CHIP_ATTENTION,
            attention,
        ),
    ];
    let mut line: Row<Element<'a, PanelMsg>> = Row::new().spacing(metrics.row_gap);
    for (value, id, count) in chips {
        line = line.push(chip(
            t!(i18n, id),
            Some(u32::try_from(count).unwrap_or(u32::MAX)),
            value == ctx.pcb_panel.pour_chip,
            chip_style(&ctx.tokens),
            metrics,
            i18n,
            Some(msg(PcbPanelMsg::PourChipPicked(value))),
        ));
    }
    container(line.wrap().vertical_spacing(metrics.row_gap).boxed())
        .padding([SP.xs, SP.l])
        .width(Length::Fill)
        .boxed()
}

/// The layer's colour as a small square; none for a layer this build does
/// not know.
fn layer_swatch<'a>(ctx: &PanelContext, layer: &str) -> Element<'a, PanelMsg> {
    let fill = parse_pcb_layer(layer).map(|l| layer_colour(l, &ctx.tokens));
    container(Space::new())
        .width(TS.caption)
        .height(TS.caption)
        .style(move |_: &iced::Theme| container::Style {
            background: fill.map(iced::Background::Color),
            ..container::Style::default()
        })
        .boxed()
}

/// One pour: layer swatch, net, state, layer and priority (a click selects
/// it), then Repour and Shelve / Unshelve, as on the pour's Copper card.
fn pour_row<'a>(
    ctx: &'a PanelContext,
    pour: &'a PcbPourRow,
    board: Option<&Path>,
) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let r = resolve(&ctx.tokens);
    let net: Label<'a> = if pour.net.is_empty() {
        t!(i18n, ids::PANELS_PCB_POURS_NO_NET).into()
    } else {
        UserText::new(pour.net.clone()).into()
    };
    let (state, state_ink) = if pour.shelved {
        (
            ids::PANELS_PCB_POURS_SHELVED,
            signex_widgets::theme_ext::warning_color(&ctx.tokens),
        )
    } else {
        (ids::PANELS_PCB_POURS_POURED, r.ink3)
    };
    let layer = layer_text(i18n, &pour.layer);
    let placement = t!(
        i18n,
        ids::PANELS_PCB_POURS_DETAIL,
        "layer" => layer.as_str().to_owned(),
        "priority" => pour.priority
    );
    let pick = button(
        column![
            row![
                layer_swatch(ctx, &pour.layer),
                text(net)
                    .size(TS.body)
                    .color(ti(ctx.tokens.text))
                    .width(Length::Fill)
                    .boxed(),
                text(t!(i18n, state))
                    .size(TS.caption)
                    .color(state_ink)
                    .boxed(),
            ]
            .spacing(SP.s)
            .align_y(iced::Alignment::Center)
            .boxed(),
            text(placement).size(TS.caption).color(r.ink3).boxed(),
        ]
        .spacing(SP.xs),
    )
    .width(Length::Fill)
    .padding([SP.s, SP.s])
    .on_press(msg(PcbPanelMsg::PourClicked(pour.zone)))
    .style(crate::styles::hover_row(
        &ctx.tokens,
        ctx.pcb_panel.pour == Some(pour.zone),
    ));
    let target = board.map(|board| pour_target(board, pour));
    let repour = target
        .clone()
        .filter(|_| !pour.shelved)
        .map(|target| pour_msg(PourMsg::RepourPressed { target }));
    let shelve = target.map(|target| {
        pour_msg(PourMsg::ShelveToggled {
            target,
            shelved: !pour.shelved,
        })
    });
    let shelve_id = if pour.shelved {
        ids::PCB_PRIM_POUR_UNSHELVE
    } else {
        ids::PCB_PRIM_POUR_SHELVE
    };
    row![
        pick.boxed(),
        dialog_button(
            t!(i18n, ids::PCB_PRIM_POUR_REPOUR),
            DialogButtonKind::Secondary,
            repour,
            &ctx.tokens,
        ),
        dialog_button(
            t!(i18n, shelve_id),
            DialogButtonKind::Secondary,
            shelve,
            &ctx.tokens,
        ),
    ]
    .spacing(SP.xs)
    .padding([0.0, SP.s])
    .align_y(iced::Alignment::Center)
    .boxed()
}

/// Polygons: the board's pours, the Polygon Manager's rows, with Repour
/// all, the chips and each pour's Repour and Shelve.
pub(super) fn polygons_mode<'a>(
    ctx: &'a PanelContext,
    summary: &'a PcbNetsSummary,
) -> Element<'a, PanelMsg> {
    let board = ctx.pcb_layers.board.as_deref();
    let shown = ctx.pcb_panel.shown_pours(summary);
    let mut list: Column<Element<'a, PanelMsg>> = Column::new().width(Length::Fill);
    if shown.is_empty() {
        let id = if summary.pours.is_empty() {
            ids::PANELS_PCB_POURS_EMPTY
        } else {
            ids::PANELS_PCB_POURS_NO_MATCH
        };
        list = list.push(note(ctx, id));
    }
    for pour in shown {
        list = list.push(pour_row(ctx, pour, board));
        list = list.push(separator(ctx));
    }
    let head = vec![
        pours_toolbar(ctx, board),
        pour_chips(ctx, summary),
        separator(ctx),
    ];
    let count = t!(&*ctx.i18n, ids::PANELS_PCB_POURS_SUMMARY, "count" => summary.pours.len());
    page(ctx, head, list, None, foot(ctx, count.into()))
}
