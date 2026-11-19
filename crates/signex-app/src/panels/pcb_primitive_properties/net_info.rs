//! The Net Information section (#2523, m95-m99): the net and class as
//! links, a Selected / Total table of Length and Delay, and Show More with
//! what else is on the net as one caption line. A track opens with it; a
//! via, pad, arc or area keeps it closed, its header naming the net and its
//! class.

use iced::widget::{container, row};
use iced::{Color, Element, Length, Theme, Widget as _};
use signex_i18n::{Localized, MessageId, UserText, ids, t};
use signex_widgets::{button, text};

use super::rows::{Look, Section, labelled, labelled_text, link, mm_or_na, value_box, with_tip};
use super::{PcbPrimitiveMsg, PrimitiveCommon, SP, TS};
use crate::panels::PanelMsg;
use crate::styles::{ti, tokens::resolve};

/// The collapse key of a Net Information section open by default.
const OPEN_KEY: &str = "pcb_prim.net_info.open";
/// The collapse key of a Net Information section closed by default.
const CLOSED_KEY: &str = "pcb_prim.net_info";

/// The section a via, pad, arc or area shows: closed by default, its header
/// naming the net and its class.
pub(super) fn section<'a>(
    look: Look<'a>,
    common: &'a PrimitiveCommon,
) -> Option<Element<'a, PanelMsg>> {
    let summary = summary(look, common);
    let s = Section::new(look, ids::PCB_PRIM_NET_INFORMATION).collapsible(
        CLOSED_KEY,
        true,
        Some(summary.into()),
    );
    rows(look, common, None, s)
}

/// The section a track shows: open, its net beside a dot of the tracks'
/// layer colour (`dot`).
pub(super) fn open_section<'a>(
    look: Look<'a>,
    common: &'a PrimitiveCommon,
    dot: Option<Color>,
) -> Option<Element<'a, PanelMsg>> {
    let s = Section::new(look, ids::PCB_PRIM_NET_INFORMATION).collapsible(OPEN_KEY, false, None);
    rows(look, common, dot, s)
}

/// "{net} · {class}" for a closed header; No Net and No Class stand in, and
/// Mixed where the selection does not share one.
fn summary(look: Look<'_>, common: &PrimitiveCommon) -> Localized {
    let info = &common.net_info;
    let net = match (info.net, info.name.as_ref()) {
        (Some(n), Some(name)) if n != 0 => name.clone(),
        (Some(0), _) => look.t(ids::PCB_PRIM_NO_NET).into_string(),
        _ => look.t(ids::PCB_PRIM_MIXED).into_string(),
    };
    let class = match info.class.as_ref() {
        Some(class) => class.clone(),
        None if info.net.is_some() => look.t(ids::PCB_PRIM_NO_CLASS).into_string(),
        None => look.t(ids::PCB_PRIM_MIXED).into_string(),
    };
    t!(look.ctx.i18n, ids::PCB_PRIM_NET_SUMMARY, "net" => net, "class" => class)
}

fn rows<'a>(
    look: Look<'a>,
    common: &'a PrimitiveCommon,
    dot: Option<Color>,
    mut s: Section<'a>,
) -> Option<Element<'a, PanelMsg>> {
    let info = &common.net_info;
    let board = common.target.board.clone();
    let blank = || value_box(look, UserText::new(String::new()));
    let net = match (info.net, info.name.as_ref()) {
        (Some(number), Some(name)) if number != 0 => link(
            look,
            UserText::new(name.clone()),
            Some(PcbPrimitiveMsg::NetLinkPressed {
                board: board.clone(),
                net: number,
            }),
            ids::PCB_PRIM_NET_LINK_TIP,
        ),
        (Some(0), _) => value_box(look, look.t(ids::PCB_PRIM_NO_NET)),
        _ => blank(),
    };
    let net = match dot {
        Some(colour) => row![super::linked::dot(colour), net]
            .spacing(SP.s)
            .align_y(iced::Alignment::Center)
            .boxed(),
        None => net,
    };
    s.push(
        common,
        ids::PCB_PRIM_NET,
        labelled(look, ids::PCB_PRIM_NET, net),
    );
    let class = match info.class.as_ref() {
        Some(class) => link(
            look,
            UserText::new(class.clone()),
            Some(PcbPrimitiveMsg::ClassLinkPressed {
                board,
                class: class.clone(),
            }),
            ids::PCB_PRIM_CLASS_LINK_TIP,
        ),
        None if info.net.is_some() => value_box(look, look.t(ids::PCB_PRIM_NO_CLASS)),
        None => blank(),
    };
    s.push(
        common,
        ids::PCB_PRIM_CLASS,
        labelled(look, ids::PCB_PRIM_CLASS, class),
    );
    // The Selected / Total table: its column heads, Length, then Delay.
    let faint = resolve(look.tokens()).ink4;
    let heads = two_columns(
        text(look.t(ids::PCB_PRIM_SELECTED))
            .size(TS.caption)
            .color(faint)
            .boxed(),
        text(look.t(ids::PCB_PRIM_TOTAL))
            .size(TS.caption)
            .color(faint)
            .boxed(),
    );
    s.push(
        common,
        ids::PCB_PRIM_LENGTH,
        labelled_text(look, UserText::new(String::new()), heads),
    );
    let length = two_columns(
        figure(mm_or_na(look, info.selected_length_mm), look.ink),
        figure(mm_or_na(look, info.total_length_mm), look.ink),
    );
    s.push(
        common,
        ids::PCB_PRIM_LENGTH,
        labelled(look, ids::PCB_PRIM_LENGTH, length),
    );
    // Delay needs the stackup's dielectric, which the board does not
    // record: the row is greyed and says why on hover.
    let greyed = Look {
        muted: faint,
        ..look
    };
    let delay = two_columns(
        figure(look.t(ids::PCB_PRIM_NO_VALUE), faint),
        figure(look.t(ids::PCB_PRIM_NO_VALUE), faint),
    );
    s.push(
        common,
        ids::PCB_PRIM_DELAY,
        with_tip(
            look,
            labelled(greyed, ids::PCB_PRIM_DELAY, delay),
            ids::PCB_PRIM_DELAY_UNKNOWN,
        ),
    );
    let toggle = if common.show_more {
        ids::PCB_PRIM_SHOW_LESS
    } else {
        ids::PCB_PRIM_SHOW_MORE
    };
    s.push(common, toggle, show_more(look, common, toggle));
    s.finish()
}

/// A figure of the table, in the monospace face.
fn figure<'a>(value: Localized, ink: Color) -> Element<'a, PanelMsg> {
    text(value)
        .size(TS.body)
        .color(ink)
        .font(iced::Font::MONOSPACE)
        .wrapping(iced::widget::text::Wrapping::None)
        .boxed()
}

/// Show More (or Show Less) as a link, and once pressed what else is on
/// the net, on one caption line.
fn show_more<'a>(
    look: Look<'a>,
    common: &PrimitiveCommon,
    toggle: MessageId,
) -> Element<'a, PanelMsg> {
    let info = &common.net_info;
    let faint = resolve(look.tokens()).ink4;
    let press = button(
        text(look.t(toggle))
            .size(TS.caption)
            .color(ti(look.tokens().accent)),
    )
    .padding(0)
    .style(|_: &Theme, _| iced::widget::button::Style::default())
    .on_press(PanelMsg::PcbPrimitive(PcbPrimitiveMsg::ShowMoreToggled));
    let mut line = row![press].spacing(SP.xs).align_y(iced::Alignment::Center);
    if common.show_more && info.net.is_some_and(|n| n != 0) {
        let counts = t!(
            look.ctx.i18n,
            ids::PCB_PRIM_NET_COUNTS,
            "tracks" => info.tracks,
            "vias" => info.vias,
            "pads" => info.pads
        );
        line = line.push(text(counts).size(TS.caption).color(faint).boxed());
    }
    container(line)
        .padding([SP.xs, super::rows::PAD_X])
        .width(Length::Fill)
        .boxed()
}

/// The Selected and Total columns of the table.
fn two_columns<'a>(
    selected: Element<'a, PanelMsg>,
    total: Element<'a, PanelMsg>,
) -> Element<'a, PanelMsg> {
    row![
        container(selected).width(Length::Fill),
        container(total).width(Length::Fill),
    ]
    .spacing(SP.s)
    .align_y(iced::Alignment::Center)
    .boxed()
}
