//! The Unrouted connections popover (mockup row 164): one row per net with its
//! unrouted connections, the pads each still has to join and, when a pour of
//! the net is on the board, that the pour will connect them; Zoom frames the
//! net's ratsnest lines. Route next frames the first unrouted connection and
//! arms Interactive Routing. Show only unrouted dims the board except those
//! lines, filled with the accent while on.

use iced::Element;
use iced::widget::{Widget, row};
use pcb_geom::PadName;
use signex_i18n::{I18n, Localized, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::text;

use super::super::graphite::TS;
use super::kit::{
    Glyph, PX, ROW_GLYPH, Tone, badge, caption, card, card_width, detail, footer, glyph, header,
    list_row, mono_weight, scrolling, small_button, two_line_row, zoom_link,
};
use super::{BoardPopoverEvent, BoardPopoverKind, NetRow, UnroutedList};
use crate::styles::ti;

/// Pad pairs written out on a row; more than fit end in the ellipsis.
const PAIRS_SHOWN: usize = 16;

/// A pad as the row writes it: `C1-1`, or the bare number of a free pad.
fn pad_label(i18n: &I18n, pad: &PadName) -> String {
    let label = if pad.designator.is_empty() {
        t!(i18n, ids::BOARD_STATUS_PAD_ALONE, "pad" => pad.pad.clone())
    } else {
        t!(
            i18n,
            ids::BOARD_STATUS_PAD,
            "designator" => pad.designator.clone(),
            "pad" => pad.pad.clone()
        )
    };
    label.to_string()
}

/// The detail line of a net: its pad pairs, then the pour note.
pub(super) fn net_detail(i18n: &I18n, net: &NetRow) -> String {
    let pairs: Vec<String> = net
        .pairs
        .iter()
        .take(PAIRS_SHOWN)
        .map(|(from, to)| {
            t!(
                i18n,
                ids::BOARD_STATUS_PAIR,
                "from" => pad_label(i18n, from),
                "to" => pad_label(i18n, to)
            )
            .to_string()
        })
        .collect();
    // The line ends in an ellipsis where it outgrows the card.
    let mut line = pairs.join(", ");
    if net.pour_will_connect {
        line.push(' ');
        line.push_str(&t!(i18n, ids::BOARD_STATUS_POUR_WILL_CONNECT).to_string());
    }
    line
}

/// The footer button: Route next, with its key when the profile binds one.
fn route_next_label(i18n: &I18n, key: &str) -> Localized {
    if key.is_empty() {
        t!(i18n, ids::BOARD_STATUS_ROUTE_NEXT)
    } else {
        t!(i18n, ids::BOARD_STATUS_ROUTE_NEXT_KEY, "key" => key.to_string())
    }
}

pub(super) fn view<'a>(
    list: &UnroutedList,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, BoardPopoverEvent> {
    let warn = ti(tokens.warning);
    let pill = badge(list.unrouted, Tone::Warning, tokens, i18n);
    let mut parts = vec![header(
        t!(i18n, ids::BOARD_STATUS_UNROUTED_TITLE),
        Some(pill),
        tokens,
    )];
    if list.nets.is_empty() {
        parts.push(list_row(
            None,
            caption(t!(i18n, ids::BOARD_STATUS_ALL_ROUTED), tokens),
            None,
            None,
            tokens,
        ));
    }
    let rows = list
        .nets
        .iter()
        .map(|net| {
            let name = text(UserText::new(net.name.clone()))
                .size(TS.body)
                .font(mono_weight(iced::font::Weight::Semibold))
                .color(ti(tokens.text))
                .wrapping(iced::widget::text::Wrapping::None);
            let count = t!(
                i18n,
                ids::BOARD_STATUS_NET_CONNECTIONS,
                "count" => net.pair_count()
            );
            let first = row![name, caption(count, tokens)]
                .spacing(4.0 * PX)
                .align_y(iced::Alignment::Center);
            two_line_row(
                glyph(Glyph::Ratsnest, ROW_GLYPH, warn),
                first.boxed(),
                detail(UserText::new(net_detail(i18n, net)), tokens),
                zoom_link(BoardPopoverEvent::NetZoomed(net.net), tokens, i18n),
            )
        })
        .collect();
    parts.push(scrolling(rows));
    parts.push(footer(
        vec![
            // Always pressable: a board routed while it is on can turn it off.
            small_button(
                t!(i18n, ids::BOARD_STATUS_SHOW_ONLY_UNROUTED),
                None,
                list.only_unrouted,
                Some(BoardPopoverEvent::OnlyUnroutedToggled),
                tokens,
            ),
            small_button(
                route_next_label(i18n, &list.route_key),
                None,
                false,
                (list.unrouted > 0).then_some(BoardPopoverEvent::RouteNextRequested),
                tokens,
            ),
        ],
        tokens,
    ));
    card(card_width(BoardPopoverKind::Unrouted), parts, tokens)
}
