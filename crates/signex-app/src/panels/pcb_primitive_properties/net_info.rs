//! The Net Information section every primitive panel opens with: the net
//! and class as links, Delay and Length for the selection and the whole
//! net, and Show More with what else is on the net.

use iced::widget::row;
use iced::{Element, Length, Widget as _};
use signex_i18n::{UserText, ids};
use signex_widgets::{button, text};

use super::rows::{Look, Section, labelled, link, mm_or_na, value_box, with_tip};
use super::{PcbPrimitiveMsg, PrimitiveCommon, SP, TS};
use crate::panels::{PanelMsg, form_look};

pub(super) fn section<'a>(
    look: Look<'a>,
    common: &'a PrimitiveCommon,
) -> Option<Element<'a, PanelMsg>> {
    let info = &common.net_info;
    let board = common.target.board.clone();
    let mut s = Section::new(look, ids::PCB_PRIM_NET_INFORMATION);
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
        _ => value_box(look, UserText::new(String::new())),
    };
    s.push(common, ids::PCB_PRIM_NET, labelled(look, ids::PCB_PRIM_NET, net));
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
        None => value_box(look, UserText::new(String::new())),
    };
    s.push(common, ids::PCB_PRIM_CLASS, labelled(look, ids::PCB_PRIM_CLASS, class));
    // Delay needs the stackup's dielectric, which the board does not
    // record: both columns stay blank, and say why on hover.
    let blank = || value_box(look, UserText::new(String::new()));
    let delay = with_tip(
        look,
        two_columns(look, blank(), blank()),
        ids::PCB_PRIM_DELAY_UNKNOWN,
    );
    s.push(common, ids::PCB_PRIM_DELAY, labelled(look, ids::PCB_PRIM_DELAY, delay));
    let length = two_columns(
        look,
        value_box(look, mm_or_na(look, info.selected_length_mm)),
        value_box(look, mm_or_na(look, info.total_length_mm)),
    );
    s.push(common, ids::PCB_PRIM_LENGTH, labelled(look, ids::PCB_PRIM_LENGTH, length));
    if common.show_more {
        for (id, count) in [
            (ids::PCB_PRIM_NET_TRACKS, info.tracks),
            (ids::PCB_PRIM_NET_VIAS, info.vias),
            (ids::PCB_PRIM_NET_PADS, info.pads),
        ] {
            let shown = if info.net.is_some_and(|n| n != 0) {
                UserText::new(count.to_string())
            } else {
                UserText::new(String::new())
            };
            s.push(common, id, labelled(look, id, value_box(look, shown)));
        }
    }
    let toggle_label = if common.show_more {
        ids::PCB_PRIM_SHOW_LESS
    } else {
        ids::PCB_PRIM_SHOW_MORE
    };
    let toggle = button(text(look.t(toggle_label)).size(TS.body))
        .padding([SP.xs, SP.s])
        .style(form_look::ghost_button)
        .on_press(PanelMsg::PcbPrimitive(PcbPrimitiveMsg::ShowMoreToggled));
    s.push(
        common,
        toggle_label,
        iced::widget::container(toggle)
            .padding([SP.xs, crate::panels::PROPERTY_ROW_PAD_X])
            .boxed(),
    );
    s.finish()
}

/// The Selected and Total columns of a Delay or Length row, each under its
/// small caption.
fn two_columns<'a>(
    look: Look<'a>,
    selected: Element<'a, PanelMsg>,
    total: Element<'a, PanelMsg>,
) -> Element<'a, PanelMsg> {
    let cell = |caption, value| {
        iced::widget::column![
            text(look.t(caption)).size(TS.caption).color(look.muted),
            value
        ]
        .spacing(SP.xs)
        .width(Length::Fill)
    };
    row![
        cell(ids::PCB_PRIM_SELECTED, selected),
        cell(ids::PCB_PRIM_TOTAL, total)
    ]
    .spacing(SP.s)
    .boxed()
}
