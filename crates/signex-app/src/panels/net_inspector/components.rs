//! The Net Inspector's By Component tab (#1274): every part of the project
//! with its value and footprint, each expandable to its connected pins and
//! the net on each. The rows come from `NetIndex::components`
//! (`handlers/net_nav/component_rows.rs`); a pin click goes to the pin
//! through the same `MemberClicked` path a net member uses.

use iced::widget::{Column, Space, column, container, row, scrollable};
use iced::{Alignment, Element, Length, Widget as _};
use signex_i18n::{MessageId, UserText, ids, t};
use signex_net::NetAnchor;
use signex_widgets::text;

use super::{CLASS_W, NetInspectorMsg, PINS_W, ROW_H, SP, TS, msg, rule};
use crate::panels::{PanelContext, PanelMsg};
use crate::styles::{ti, tokens::resolve};

/// Width of the Footprint column.
const FOOTPRINT_W: f32 = TS.body * 9.0;

/// One connected pin of an expanded part.
#[derive(Debug, Clone, PartialEq)]
pub struct ComponentPinRow {
    pub pin: String,
    /// The net the pin is on.
    pub net: String,
    /// The sheet the pin's unit sits on, as the project names it.
    pub sheet: String,
    pub anchor: NetAnchor,
}

/// One part row.
#[derive(Debug, Clone, PartialEq)]
pub struct ComponentRow {
    /// What the expanded set knows the part by.
    pub key: String,
    pub designator: String,
    pub value: String,
    pub footprint: String,
    /// Connected pins.
    pub pins: usize,
    pub expanded: bool,
    /// Filled only while expanded.
    pub pin_rows: Vec<ComponentPinRow>,
}

/// The heading row, then one row per part and its pins when expanded.
pub(super) fn component_table<'a>(
    ctx: &'a PanelContext,
    parts: &'a [ComponentRow],
) -> Element<'a, PanelMsg> {
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
            head(ids::PANELS_NET_INSPECTOR_COLUMN_PART, Length::Fill),
            head(
                ids::PANELS_NET_INSPECTOR_COLUMN_VALUE,
                Length::Fixed(CLASS_W)
            ),
            head(
                ids::PANELS_NET_INSPECTOR_COLUMN_FOOTPRINT,
                Length::Fixed(FOOTPRINT_W)
            ),
            head(ids::PANELS_NET_INSPECTOR_COLUMN_PINS, Length::Fixed(PINS_W)),
        ]
        .padding([0.0, SP.l]),
        rule(ctx),
    ]
    .width(Length::Fill);
    if parts.is_empty() {
        list = list.push(
            container(
                text(t!(i18n, ids::PANELS_NET_INSPECTOR_EMPTY_PARTS))
                    .size(TS.body)
                    .color(muted),
            )
            .padding([SP.m, SP.l])
            .boxed(),
        );
    }
    for part in parts {
        list = list.push(part_row(ctx, part));
        for pin in &part.pin_rows {
            list = list.push(pin_row(ctx, pin));
        }
        list = list.push(rule(ctx));
    }
    scrollable(list)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(crate::panels::form_look::scrollbar_style)
        .boxed()
}

/// Text of a cell that may be blank: the text, else a dash.
fn or_blank<'a>(ctx: &'a PanelContext, value: &'a str) -> signex_widgets::Label<'a> {
    let i18n = &*ctx.i18n;
    if value.trim().is_empty() {
        t!(i18n, ids::PANELS_NET_INSPECTOR_BLANK).into()
    } else {
        UserText::new(value).into()
    }
}

/// A part: chevron and designator, value, footprint, pin count. Clicking
/// folds or unfolds it.
fn part_row<'a>(ctx: &'a PanelContext, part: &'a ComponentRow) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let tokens = &ctx.tokens;
    let (ink, muted) = (ti(tokens.text), resolve(tokens).ink3);
    let mono = crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT);
    let line = row![
        container(
            row![
                signex_widgets::chevron::chevron(part.expanded, TS.body, muted),
                text(or_blank(ctx, &part.designator))
                    .size(TS.body)
                    .font(mono)
                    .color(ink),
            ]
            .spacing(SP.s)
            .align_y(Alignment::Center),
        )
        .width(Length::Fill)
        .clip(true),
        container(text(or_blank(ctx, &part.value)).size(TS.body).color(muted))
            .width(CLASS_W)
            .clip(true),
        container(
            text(or_blank(ctx, &part.footprint))
                .size(TS.body)
                .color(muted),
        )
        .width(FOOTPRINT_W)
        .clip(true),
        container(
            text(t!(i18n, ids::PANELS_NET_INSPECTOR_PINS, "count" => part.pins))
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
        .on_press(msg(NetInspectorMsg::ComponentToggled(part.key.clone())))
        .style(crate::styles::menu_item(tokens))
        .boxed()
}

/// A connected pin of an expanded part: its number, its net, its sheet.
/// Clicking goes to the pin and highlights its net.
fn pin_row<'a>(ctx: &'a PanelContext, pin: &'a ComponentPinRow) -> Element<'a, PanelMsg> {
    let tokens = &ctx.tokens;
    let (ink, muted) = (ti(tokens.text), resolve(tokens).ink3);
    let mono = crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT);
    let line = row![
        Space::new().width(SP.xl).height(Length::Shrink),
        text(UserText::new(pin.pin.as_str()))
            .size(TS.body)
            .font(mono)
            .color(muted)
            .width(CLASS_W),
        text(UserText::new(pin.net.as_str()))
            .size(TS.body)
            .font(mono)
            .color(ink)
            .width(Length::Fill),
        text(UserText::new(pin.sheet.as_str()))
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
            net: pin.net.clone(),
            anchor: pin.anchor.clone(),
        }))
        .style(crate::styles::menu_item(tokens))
        .boxed()
}
