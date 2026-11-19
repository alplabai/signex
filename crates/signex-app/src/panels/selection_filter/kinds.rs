//! The kind toggles both filter panels draw (#55, #56): All / None with the
//! "10 of 12 kinds" readout, then one full-width toggle per kind with its
//! checkbox, name and count. One view, generic over the kind like the
//! preset list (`active_bar::presets::PresetKind`).

use iced::widget::{Column, Row, Space, container};
use iced::{Alignment, Background, Border, Element, Length, Theme, Widget as _};
use pcb_geom::PickKind;
use signex_i18n::{MessageId, UserText, ids, t};
use signex_widgets::chips::count_text;
use signex_widgets::{Label, SegmentedStyle, button, checkbox, segmented, text};

use super::KindCounts;
use crate::active_bar::{PresetKind, SelectionFilter};
use crate::panels::form_look::checkbox_style;
use crate::panels::graphite::hint_tooltip;
use crate::panels::pcb_board_properties::pick_kind_label;
use crate::panels::{PanelContext, PanelMsg};
use crate::styles::tokens::resolve;
use crate::styles::{Radii, Spacing, TypeScale, dimmed, dimmed_button, ti};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// A kind a filter panel lists.
pub(super) trait FilterKind: PresetKind {
    /// The kind's name in the catalog.
    fn message_id(self) -> MessageId;

    /// Whether turning the kind off changes what a pick selects; a kind
    /// that does not is an inert row with why on hover.
    fn is_live(self) -> bool {
        true
    }
}

impl FilterKind for SelectionFilter {
    fn message_id(self) -> MessageId {
        self.message()
    }
}

impl FilterKind for PickKind {
    fn message_id(self) -> MessageId {
        pick_kind_label(self)
    }

    fn is_live(self) -> bool {
        self.is_pickable()
    }
}

/// Which cell of All / None reads as picked: All with every kind on, None
/// with none, neither in between.
pub(super) fn bulk_pick(on: usize, total: usize) -> Option<bool> {
    if on == total {
        Some(true)
    } else if on == 0 {
        Some(false)
    } else {
        None
    }
}

/// All / None as a segmented pair, and how many kinds are on.
pub(super) fn header<'a>(
    ctx: &'a PanelContext,
    on: usize,
    total: usize,
    all: PanelMsg,
    none: PanelMsg,
) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let r = resolve(&ctx.tokens);
    let style = SegmentedStyle::new(
        r.sunken,
        r.raised,
        r.shadow,
        r.ink3,
        ti(ctx.tokens.text),
        TS.body,
    );
    let all_label: Label<'a> = t!(i18n, ids::PANELS_FILTER_ALL).into();
    let none_label: Label<'a> = t!(i18n, ids::PANELS_FILTER_NONE).into();
    let bulk = segmented(
        [(Some(true), all_label), (Some(false), none_label)],
        bulk_pick(on, total),
        style,
        move |pick| {
            if pick == Some(false) {
                none.clone()
            } else {
                all.clone()
            }
        },
    );
    let readout = text(t!(i18n, ids::PANELS_FILTER_READOUT, "on" => on, "total" => total))
        .size(TS.caption)
        .color(r.ink3);
    let line: Row<Element<'a, PanelMsg>> = Row::new()
        .spacing(SP.s)
        .align_y(Alignment::Center)
        .push(bulk)
        .push(Space::new().width(Length::Fill).boxed())
        .push(readout.boxed());
    container(line)
        .padding([SP.xs, SP.m])
        .width(Length::Fill)
        .boxed()
}

/// One full-width toggle per kind, in [`PresetKind::all`] order. `counts`
/// is `None` where the panel has no document to count.
pub(super) fn toggle_column<'a, K: FilterKind>(
    ctx: &'a PanelContext,
    is_on: impl Fn(K) -> bool,
    counts: Option<&KindCounts<K>>,
    toggled: impl Fn(K) -> PanelMsg,
) -> Element<'a, PanelMsg> {
    let mut column: Column<Element<'a, PanelMsg>> =
        Column::new().spacing(SP.xs).width(Length::Fill);
    for &kind in K::all() {
        let count = counts.map(|c| c.get(kind));
        column = column.push(toggle_row(ctx, kind, is_on(kind), count, toggled(kind)));
    }
    container(column)
        .padding([SP.xs, SP.m])
        .width(Length::Fill)
        .boxed()
}

/// A kind's toggle: checkbox, name and count. An off kind takes the
/// Graphite disabled look (faded fill, border and ink).
fn toggle_row<'a, K: FilterKind>(
    ctx: &'a PanelContext,
    kind: K,
    on: bool,
    count: Option<usize>,
    msg: PanelMsg,
) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let tokens = &ctx.tokens;
    let r = resolve(tokens);
    let live = kind.is_live();
    let mut check = checkbox(on).size(TS.body).style(checkbox_style);
    if live {
        let toggled = msg.clone();
        check = check.on_toggle(move |_| toggled.clone());
    }
    let mut cells: Row<Element<'a, PanelMsg>> = Row::new()
        .spacing(SP.s)
        .align_y(Alignment::Center)
        .push(check.boxed())
        .push(text(t!(i18n, kind.message_id())).size(TS.body).boxed())
        .push(Space::new().width(Length::Fill).boxed());
    if let Some(count) = count {
        let shown = count_text(i18n, u32::try_from(count).unwrap_or(u32::MAX));
        let ink = if on { r.ink3 } else { dimmed(r.ink3) };
        cells = cells.push(
            text(UserText::new(shown))
                .size(TS.caption)
                .color(ink)
                .boxed(),
        );
    }
    let (fill, hover, ink, line) = (r.field, r.sunken, ti(tokens.text), ti(tokens.border));
    let row = button(cells)
        .width(Length::Fill)
        .padding([SP.xs, SP.s])
        .on_press_maybe(live.then_some(msg))
        .style(move |_: &Theme, status: button::Status| {
            let hovered = live && matches!(status, button::Status::Hovered);
            let style = button::Style {
                background: Some(Background::Color(if hovered { hover } else { fill })),
                text_color: ink,
                border: Border {
                    width: 1.0,
                    radius: Radii::GRAPHITE.control.into(),
                    color: line,
                    ..Border::default()
                },
                ..button::Style::default()
            };
            if on { style } else { dimmed_button(style) }
        })
        .boxed();
    if live {
        row
    } else {
        let why = t!(i18n, ids::PCB_BOARD_FILTER_UNAVAILABLE);
        hint_tooltip(row, why, tokens, iced::widget::tooltip::Position::Bottom)
    }
}
