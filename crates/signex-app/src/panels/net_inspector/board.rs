//! The Net Inspector with a board in view (#2403): the board's nets, each
//! with its routed length, vias, pads, state and its Select and Highlight.

use iced::widget::{Column, column, container, row};
use iced::{Alignment, Element, Length, Widget as _};
use signex_i18n::{MessageId, UserText, ids, t};
use signex_widgets::text;

use super::by_net::{PINS_W, heading};
use super::{BoardNetRow, BoardNetView, NetInspectorMsg, SP, TS, msg};
use crate::panels::{PanelContext, PanelMsg};
use crate::styles::{ChromeMetrics, ti, tokens::resolve};

const CM: ChromeMetrics = ChromeMetrics::DEFAULT;
/// Width of the Length column of a board net row.
const LENGTH_W: f32 = TS.body * 6.5;
/// Width of the routed state column.
const STATE_W: f32 = TS.body * 5.0;

/// The filter, the board's nets and the footer.
pub(super) fn view<'a>(ctx: &'a PanelContext, board: &'a BoardNetView) -> Element<'a, PanelMsg> {
    let summary = t!(
        ctx.i18n,
        ids::BOARD_NETS_SUMMARY,
        "shown" => board.nets.len(),
        "total" => board.total
    );
    column![
        super::filter_field(
            ctx,
            ids::PANELS_NET_INSPECTOR_FILTER,
            &ctx.net_inspector.filter
        ),
        table(ctx, board),
        super::rule(ctx),
        super::footer(ctx, summary, None),
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .boxed()
}

/// The heading row, then one row per net.
fn table<'a>(ctx: &'a PanelContext, view: &'a BoardNetView) -> Element<'a, PanelMsg> {
    let muted = resolve(&ctx.tokens).ink3;
    let mut list: Column<Element<'a, PanelMsg>> = column![
        heading(
            ctx,
            &[
                (ids::PANELS_NET_INSPECTOR_COLUMN_NET, Length::Fill),
                (ids::BOARD_NETS_COLUMN_LENGTH, Length::Fixed(LENGTH_W)),
                (ids::BOARD_NETS_COLUMN_VIAS, Length::Fixed(PINS_W)),
                (ids::BOARD_NETS_COLUMN_PADS, Length::Fixed(PINS_W)),
                (ids::BOARD_NETS_COLUMN_STATE, Length::Fixed(STATE_W)),
            ],
        ),
        super::rule(ctx),
    ]
    .width(Length::Fill);
    if view.nets.is_empty() {
        list = list.push(
            container(
                text(t!(ctx.i18n, ids::BOARD_NETS_EMPTY))
                    .size(TS.body)
                    .color(muted),
            )
            .padding([SP.m, SP.l])
            .boxed(),
        );
    }
    for net in &view.nets {
        list = list.push(net_row(ctx, net));
        list = list.push(super::rule(ctx));
    }
    super::scrolled(list.boxed())
}

/// A board net: name, length, vias, pads, state, then its two actions.
fn net_row<'a>(ctx: &'a PanelContext, net: &'a BoardNetRow) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let tokens = &ctx.tokens;
    let (ink, muted) = (ti(tokens.text), resolve(tokens).ink3);
    let mono = crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT);
    let length = signex_types::measure::format_length_mm(net.routed_length_mm, ctx.unit);
    let state = if net.unrouted == 0 {
        t!(i18n, ids::BOARD_NETS_ROUTED)
    } else {
        t!(i18n, ids::BOARD_NETS_OPEN, "count" => net.unrouted)
    };
    let cell = |label: signex_widgets::Label<'a>, w: f32| -> Element<'a, PanelMsg> {
        container(text(label).size(TS.body).font(mono).color(ink))
            .width(w)
            .clip(true)
            .boxed()
    };
    let action = |id: MessageId, message: NetInspectorMsg| -> Element<'a, PanelMsg> {
        signex_widgets::button(text(t!(i18n, id)).size(TS.caption).color(muted))
            .padding([SP.xs, SP.s])
            .on_press(msg(message))
            .style(crate::styles::menu_item(tokens))
            .boxed()
    };
    let numbers =
        |count: usize| -> signex_widgets::Label<'a> { UserText::new(count.to_string()).into() };
    row![
        container(
            text(UserText::new(net.name.as_str()))
                .size(TS.body)
                .font(mono)
                .color(ink),
        )
        .width(Length::Fill)
        .clip(true),
        cell(UserText::new(length).into(), LENGTH_W),
        cell(numbers(net.vias), PINS_W),
        cell(numbers(net.pads), PINS_W),
        container(text(state).size(TS.body).color(muted)).width(STATE_W),
        action(
            ids::BOARD_NETS_SELECT,
            NetInspectorMsg::BoardNetSelected(net.name.clone())
        ),
        action(
            ids::BOARD_NETS_HIGHLIGHT,
            NetInspectorMsg::BoardNetHighlighted(net.name.clone())
        ),
    ]
    .spacing(SP.s)
    .height(CM.field)
    .padding([0.0, SP.l])
    .align_y(Alignment::Center)
    .boxed()
}
