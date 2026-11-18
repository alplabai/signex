//! Net Inspector panel (#1269): every net of the active document's project,
//! read from the cached stitched project net index (`signex_net::NetIndex`),
//! with its net class and pin count. A net expands to its pins, labels, power
//! ports and sheet entries, each with its sheet; clicking one goes to it
//! (`handlers/net_nav/inspector.rs`). A pure view over data the panel
//! context copies from the index; nothing here re-derives a net.

use std::collections::BTreeSet;

use iced::widget::{Column, Space, column, container, row, scrollable};
use iced::{Alignment, Background, Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, MessageId, UserText, ids, t};
use signex_net::NetAnchor;
use signex_widgets::{text, text_input};

use super::{PanelContext, PanelMsg};
use crate::styles::{Spacing, TypeScale, ti, tokens::resolve};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Height of one net row.
const ROW_H: f32 = TS.body * 2.4;
/// Width of the Class column.
const CLASS_W: f32 = TS.body * 7.0;
/// Width of the Pins column.
const PINS_W: f32 = TS.body * 3.5;

/// The panel's own state: the filter text and the nets shown expanded, by
/// name. Kept in `UiState`; the panel context carries a copy.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct NetInspectorState {
    pub filter: String,
    pub expanded: BTreeSet<String>,
}

/// What happened in the Net Inspector panel.
#[derive(Debug, Clone)]
pub enum NetInspectorMsg {
    FilterChanged(String),
    /// A net row was clicked: expand or fold it.
    NetToggled(String),
    /// A member row was clicked: go to it and highlight its net. `net` is the
    /// name it was listed under; `anchor` finds it again in a rebuilt index.
    MemberClicked {
        net: String,
        anchor: NetAnchor,
    },
    /// A board net row's Select (#2403): select the net on the board.
    BoardNetSelected(String),
    /// A board net row's Highlight: draw the net bright, or no longer.
    BoardNetHighlighted(String),
}

/// One net of the board in view (#2403), from `pcb_geom::net_reports`.
#[derive(Debug, Clone, PartialEq)]
pub struct BoardNetRow {
    pub name: String,
    pub routed_length_mm: f64,
    pub vias: usize,
    pub pads: usize,
    /// Connections still to route; 0 when the net is routed.
    pub unrouted: usize,
}

/// What the panel lists with a board in view: its nets the filter keeps.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BoardNetView {
    pub nets: Vec<BoardNetRow>,
    /// Every net of the board.
    pub total: usize,
}

/// Where the list comes from.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NetListStatus {
    /// The active document is not a schematic of a project (or none is open).
    #[default]
    NoProject,
    /// The project's net index is being built.
    Reading,
    /// The last build failed (the reason is in Messages).
    Unavailable,
    Ready,
}

/// What a member of a net is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetMemberRowKind {
    Pin,
    Label,
    PowerPort,
    SheetEntry,
}

/// One member of an expanded net.
#[derive(Debug, Clone, PartialEq)]
pub struct NetMemberRow {
    pub kind: NetMemberRowKind,
    /// `R1.2` for a pin, else the label, port or entry text.
    pub text: String,
    /// The sheet it sits on, as the project names it.
    pub sheet: String,
    pub anchor: NetAnchor,
}

/// One net row.
#[derive(Debug, Clone, PartialEq)]
pub struct NetRow {
    pub name: String,
    /// The first net class that contains the net.
    pub class: Option<String>,
    /// Component pins on the net.
    pub pins: usize,
    pub expanded: bool,
    /// Filled only while expanded.
    pub members: Vec<NetMemberRow>,
}

/// What the panel shows.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NetInspectorView {
    pub filter: String,
    pub status: NetListStatus,
    /// The nets the filter keeps, named nets first, then by name.
    pub nets: Vec<NetRow>,
    /// Every net of the project.
    pub total: usize,
    /// With a board in view, its nets instead of the project's (#2403).
    pub board: Option<BoardNetView>,
}

fn msg(m: NetInspectorMsg) -> PanelMsg {
    PanelMsg::NetInspector(m)
}

/// Render the panel.
pub fn view<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let tokens = &ctx.tokens;
    let i18n = &*ctx.i18n;
    let r = resolve(tokens);
    let muted = r.ink3;
    let view = &ctx.net_inspector;
    let filter = container(
        text_input(t!(i18n, ids::PANELS_NET_INSPECTOR_FILTER), &view.filter)
            .on_input(|s| msg(NetInspectorMsg::FilterChanged(s)))
            .style(super::form_look::text_input_style)
            .size(TS.body)
            .padding([SP.xs, SP.s])
            .width(Length::Fill),
    )
    .padding([SP.m, SP.l])
    .width(Length::Fill);
    let note = |id: MessageId| -> Element<'a, PanelMsg> {
        container(text(t!(i18n, id)).size(TS.caption).color(muted))
            .padding([SP.m, SP.l])
            .width(Length::Fill)
            .boxed()
    };
    let body: Element<'a, PanelMsg> = match (&view.board, view.status) {
        (Some(board), _) => board_table(ctx, board),
        (None, NetListStatus::NoProject) => note(ids::PANELS_NET_INSPECTOR_NO_PROJECT),
        (None, NetListStatus::Reading) => note(ids::PANELS_NET_INSPECTOR_READING),
        (None, NetListStatus::Unavailable) => note(ids::PANELS_NET_INSPECTOR_UNAVAILABLE),
        (None, NetListStatus::Ready) => table(ctx, view),
    };
    let summary = match &view.board {
        Some(board) => t!(
            i18n,
            ids::BOARD_NETS_SUMMARY,
            "shown" => board.nets.len(),
            "total" => board.total
        ),
        None => t!(
            i18n,
            ids::PANELS_NET_INSPECTOR_SUMMARY,
            "shown" => view.nets.len(),
            "total" => view.total
        ),
    };
    let footer = container(text(summary).size(TS.caption).color(muted))
        .padding([SP.m, SP.l])
        .width(Length::Fill);
    column![filter, body, rule(ctx), footer]
        .width(Length::Fill)
        .height(Length::Fill)
        .boxed()
}

/// A one-pixel soft rule across the panel.
fn rule<'a>(ctx: &PanelContext) -> Element<'a, PanelMsg> {
    let color = resolve(&ctx.tokens).line_soft;
    container(Space::new())
        .width(Length::Fill)
        .height(1)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(color)),
            ..container::Style::default()
        })
        .boxed()
}

/// The heading row, then one row per net and its members when expanded.
fn table<'a>(ctx: &'a PanelContext, view: &'a NetInspectorView) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let muted = resolve(&ctx.tokens).ink3;
    let head = |id: MessageId, w: Length| -> Element<'a, PanelMsg> {
        container(text(t!(i18n, id)).size(TS.section).color(muted))
            .width(w)
            .height(ROW_H)
            .align_y(iced::alignment::Vertical::Center)
            .boxed()
    };
    let mut list: Column<Element<'a, PanelMsg>> = column![
        row![
            head(ids::PANELS_NET_INSPECTOR_COLUMN_NET, Length::Fill),
            head(
                ids::PANELS_NET_INSPECTOR_COLUMN_CLASS,
                Length::Fixed(CLASS_W)
            ),
            head(ids::PANELS_NET_INSPECTOR_COLUMN_PINS, Length::Fixed(PINS_W)),
        ]
        .padding([0.0, SP.l]),
        rule(ctx),
    ]
    .width(Length::Fill);
    if view.nets.is_empty() {
        list = list.push(
            container(
                text(t!(i18n, ids::PANELS_NET_INSPECTOR_EMPTY))
                    .size(TS.body)
                    .color(muted),
            )
            .padding([SP.m, SP.l])
            .boxed(),
        );
    }
    for net in &view.nets {
        list = list.push(net_row(ctx, net));
        for member in &net.members {
            list = list.push(member_row(ctx, &net.name, member));
        }
        list = list.push(rule(ctx));
    }
    scrollable(list)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(super::form_look::scrollbar_style)
        .boxed()
}

/// A net: chevron and name, class, pin count. Clicking folds or unfolds it.
fn net_row<'a>(ctx: &'a PanelContext, net: &'a NetRow) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let tokens = &ctx.tokens;
    let (ink, muted) = (ti(tokens.text), resolve(tokens).ink3);
    let mono = crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT);
    let class: signex_widgets::Label<'a> = match &net.class {
        Some(class) => UserText::new(class.as_str()).into(),
        None => t!(i18n, ids::PANELS_NET_INSPECTOR_NO_CLASS).into(),
    };
    let line = row![
        container(
            row![
                signex_widgets::chevron::chevron(net.expanded, TS.body, muted),
                text(UserText::new(net.name.as_str()))
                    .size(TS.body)
                    .font(mono)
                    .color(ink),
            ]
            .spacing(SP.s)
            .align_y(Alignment::Center),
        )
        .width(Length::Fill)
        .clip(true),
        container(text(class).size(TS.body).color(muted))
            .width(CLASS_W)
            .clip(true),
        container(
            text(t!(i18n, ids::PANELS_NET_INSPECTOR_PINS, "count" => net.pins))
                .size(TS.body)
                .font(mono)
                .color(ink),
        )
        .width(PINS_W),
    ]
    .height(ROW_H)
    .align_y(Alignment::Center);
    signex_widgets::button(line)
        .width(Length::Fill)
        .padding([0.0, SP.l])
        .on_press(msg(NetInspectorMsg::NetToggled(net.name.clone())))
        .style(crate::styles::menu_item(tokens))
        .boxed()
}

/// A member of an expanded net: what it is, its text, its sheet. Clicking
/// goes to it.
fn member_row<'a>(
    ctx: &'a PanelContext,
    net: &str,
    member: &'a NetMemberRow,
) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let tokens = &ctx.tokens;
    let (ink, muted) = (ti(tokens.text), resolve(tokens).ink3);
    let mono = crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT);
    let line = row![
        Space::new().width(SP.xl).height(Length::Shrink),
        text(kind_label(i18n, member.kind))
            .size(TS.caption)
            .color(muted)
            .width(CLASS_W),
        text(UserText::new(member.text.as_str()))
            .size(TS.body)
            .font(mono)
            .color(ink)
            .width(Length::Fill),
        text(UserText::new(member.sheet.as_str()))
            .size(TS.caption)
            .color(muted)
            .wrapping(iced::widget::text::Wrapping::None),
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center);
    signex_widgets::button(line)
        .width(Length::Fill)
        .padding([SP.xs, SP.l])
        .on_press(msg(NetInspectorMsg::MemberClicked {
            net: net.to_string(),
            anchor: member.anchor.clone(),
        }))
        .style(crate::styles::menu_item(tokens))
        .boxed()
}

/// Width of the Length column of a board net row.
const LENGTH_W: f32 = TS.body * 6.5;
/// Width of the routed state column.
const STATE_W: f32 = TS.body * 5.0;

/// The board's nets (#2403): the heading row, then one row per net with
/// its routed length, vias, pads, state and its Select and Highlight.
fn board_table<'a>(ctx: &'a PanelContext, view: &'a BoardNetView) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let muted = resolve(&ctx.tokens).ink3;
    let head = |id: MessageId, w: Length| -> Element<'a, PanelMsg> {
        container(text(t!(i18n, id)).size(TS.section).color(muted))
            .width(w)
            .height(ROW_H)
            .align_y(iced::alignment::Vertical::Center)
            .boxed()
    };
    let mut list: Column<Element<'a, PanelMsg>> = column![
        row![
            head(ids::PANELS_NET_INSPECTOR_COLUMN_NET, Length::Fill),
            head(ids::BOARD_NETS_COLUMN_LENGTH, Length::Fixed(LENGTH_W)),
            head(ids::BOARD_NETS_COLUMN_VIAS, Length::Fixed(PINS_W)),
            head(ids::BOARD_NETS_COLUMN_PADS, Length::Fixed(PINS_W)),
            head(ids::BOARD_NETS_COLUMN_STATE, Length::Fixed(STATE_W)),
        ]
        .spacing(SP.s)
        .padding([0.0, SP.l]),
        rule(ctx),
    ]
    .width(Length::Fill);
    if view.nets.is_empty() {
        list = list.push(
            container(
                text(t!(i18n, ids::BOARD_NETS_EMPTY))
                    .size(TS.body)
                    .color(muted),
            )
            .padding([SP.m, SP.l])
            .boxed(),
        );
    }
    for net in &view.nets {
        list = list.push(board_net_row(ctx, net));
        list = list.push(rule(ctx));
    }
    scrollable(list)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(super::form_look::scrollbar_style)
        .boxed()
}

/// A board net: name, length, vias, pads, state, then its two actions.
fn board_net_row<'a>(ctx: &'a PanelContext, net: &'a BoardNetRow) -> Element<'a, PanelMsg> {
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
    .height(ROW_H)
    .padding([0.0, SP.l])
    .align_y(Alignment::Center)
    .boxed()
}

fn kind_label(i18n: &I18n, kind: NetMemberRowKind) -> signex_i18n::Localized {
    match kind {
        NetMemberRowKind::Pin => t!(i18n, ids::PANELS_NET_INSPECTOR_KIND_PIN),
        NetMemberRowKind::Label => t!(i18n, ids::PANELS_NET_INSPECTOR_KIND_LABEL),
        NetMemberRowKind::PowerPort => t!(i18n, ids::PANELS_NET_INSPECTOR_KIND_PORT),
        NetMemberRowKind::SheetEntry => t!(i18n, ids::PANELS_NET_INSPECTOR_KIND_SHEET_ENTRY),
    }
}
