//! The By Component list: one row per part (designator, value and
//! footprint, pin count); an expanded part lists every pin with its net and
//! pin name under a muted Pin · Net · Pin name heading.

use iced::widget::{Column, Space, column, container, row};
use iced::{Alignment, Element, Length, Widget as _};
use signex_i18n::{UserText, ids, t};
use signex_widgets::text;

use super::by_net::{PINS_W, expanded_fill, heading, more_row};
use super::{ComponentPinRow, ComponentRow, NetInspectorMsg, NetInspectorView, SP, TS, msg};
use crate::panels::{PanelContext, PanelMsg};
use crate::styles::{ChromeMetrics, ti, tokens::resolve};

const CM: ChromeMetrics = ChromeMetrics::DEFAULT;
/// Width of an expanded part's Pin column.
const PIN_W: f32 = TS.body * 3.0;
/// Width of an expanded part's Pin name column.
const PIN_NAME_W: f32 = TS.body * 6.0;

/// The heading row and the parts, or the no-match card.
pub(super) fn list<'a>(ctx: &'a PanelContext, view: &'a NetInspectorView) -> Element<'a, PanelMsg> {
    if view.components.is_empty() {
        return column![
            super::no_match(
                ctx,
                ids::PANELS_NET_INSPECTOR_EMPTY_PARTS,
                ids::PANELS_NET_INSPECTOR_EMPTY_PARTS_BODY,
                view.component_total,
            ),
            Space::new().width(Length::Fill).height(Length::Fill),
        ]
        .boxed();
    }
    let mut list: Column<Element<'a, PanelMsg>> = column![
        heading(
            ctx,
            &[
                (
                    ids::PANELS_NET_INSPECTOR_COLUMN_PART,
                    Length::FillPortion(1)
                ),
                (
                    ids::PANELS_NET_INSPECTOR_COLUMN_VALUE_FOOTPRINT,
                    Length::FillPortion(3)
                ),
                (ids::PANELS_NET_INSPECTOR_COLUMN_PINS, Length::Fixed(PINS_W)),
            ],
        ),
        super::rule(ctx),
    ]
    .width(Length::Fill);
    for part in &view.components {
        list = list.push(part_block(ctx, part));
    }
    super::scrolled(list.boxed())
}

/// A part row and, when expanded, its pin heading and pins.
fn part_block<'a>(ctx: &'a PanelContext, part: &'a ComponentRow) -> Element<'a, PanelMsg> {
    let mut block: Column<Element<'a, PanelMsg>> = column![part_row(ctx, part)];
    if part.expanded {
        block = block.push(pin_heading(ctx));
        for pin in &part.pin_rows {
            block = block.push(pin_row(ctx, pin));
        }
        if part.more > 0 {
            block = block.push(more_row(
                ctx,
                t!(ctx.i18n, ids::PANELS_NET_INSPECTOR_MORE_PINS, "count" => part.more),
                part.key.clone(),
            ));
        }
    }
    expanded_fill(ctx, block.boxed(), part.expanded)
}

/// A part: chevron and designator, value and footprint, pin count.
/// Clicking folds or unfolds it.
fn part_row<'a>(ctx: &'a PanelContext, part: &'a ComponentRow) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let tokens = &ctx.tokens;
    let (ink, muted) = (ti(tokens.text), resolve(tokens).ink3);
    let mono = crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT);
    let mut described = row![
        text(UserText::new(part.value.as_str()))
            .size(TS.body)
            .color(ink)
            .wrapping(iced::widget::text::Wrapping::None),
    ]
    .spacing(SP.xs)
    .align_y(Alignment::Center);
    if !part.footprint.is_empty() {
        described = described.push(
            text(t!(
                i18n,
                ids::PANELS_NET_INSPECTOR_FOOTPRINT_SUFFIX,
                "footprint" => part.footprint.as_str()
            ))
            .size(TS.body)
            .color(muted)
            .wrapping(iced::widget::text::Wrapping::None)
            .boxed(),
        );
    }
    let line = row![
        container(
            row![
                signex_widgets::chevron::chevron(part.expanded, TS.body, muted),
                text(UserText::new(part.designator.as_str()))
                    .size(TS.body)
                    .font(mono)
                    .color(ink)
                    .wrapping(iced::widget::text::Wrapping::None),
            ]
            .spacing(SP.s)
            .align_y(Alignment::Center),
        )
        .width(Length::FillPortion(1))
        .clip(true),
        container(described)
            .width(Length::FillPortion(3))
            .clip(true),
        container(
            text(t!(i18n, ids::PANELS_NET_INSPECTOR_PINS, "count" => part.pins))
                .size(TS.body)
                .font(mono)
                .color(ink),
        )
        .width(PINS_W)
        .align_x(iced::alignment::Horizontal::Right),
    ]
    .spacing(SP.s)
    .height(CM.field)
    .align_y(Alignment::Center);
    signex_widgets::button(line)
        .width(Length::Fill)
        .padding([0.0, SP.l])
        .on_press(msg(NetInspectorMsg::ComponentToggled(part.key.clone())))
        .style(crate::styles::menu_item(tokens))
        .boxed()
}

/// The muted Pin · Net · Pin name heading of an expanded part.
fn pin_heading<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let muted = resolve(&ctx.tokens).ink3;
    let cell = |id: signex_i18n::MessageId, width: Length| -> Element<'a, PanelMsg> {
        container(text(t!(ctx.i18n, id)).size(TS.caption).color(muted))
            .width(width)
            .boxed()
    };
    container(
        row![
            Space::new().width(SP.xl).height(Length::Shrink),
            cell(ids::PANELS_NET_INSPECTOR_COLUMN_PIN, Length::Fixed(PIN_W)),
            cell(ids::PANELS_NET_INSPECTOR_COLUMN_PIN_NET, Length::Fill),
            cell(
                ids::PANELS_NET_INSPECTOR_COLUMN_PIN_NAME,
                Length::Fixed(PIN_NAME_W)
            ),
        ]
        .spacing(SP.s)
        .align_y(Alignment::Center),
    )
    .padding([SP.xs, SP.l])
    .width(Length::Fill)
    .boxed()
}

/// A pin: its number, its net (or "— unconnected"), its name. A wired pin
/// goes to the pin on its sheet when clicked.
fn pin_row<'a>(ctx: &'a PanelContext, pin: &'a ComponentPinRow) -> Element<'a, PanelMsg> {
    let tokens = &ctx.tokens;
    let (ink, muted) = (ti(tokens.text), resolve(tokens).ink3);
    let mono = crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT);
    let net: Element<'a, PanelMsg> = match &pin.net {
        Some(net) => text(UserText::new(net.as_str()))
            .size(TS.body)
            .font(mono)
            .color(ink)
            .wrapping(iced::widget::text::Wrapping::None)
            .boxed(),
        None => text(t!(ctx.i18n, ids::PANELS_NET_INSPECTOR_UNCONNECTED))
            .size(TS.body)
            .color(muted)
            .wrapping(iced::widget::text::Wrapping::None)
            .boxed(),
    };
    let line = row![
        Space::new().width(SP.xl).height(Length::Shrink),
        container(
            text(UserText::new(pin.pin.as_str()))
                .size(TS.caption)
                .font(mono)
                .color(muted),
        )
        .width(PIN_W)
        .clip(true),
        container(net).width(Length::Fill).clip(true),
        container(
            text(UserText::new(pin.name.as_str()))
                .size(TS.caption)
                .font(mono)
                .color(muted)
                .wrapping(iced::widget::text::Wrapping::None),
        )
        .width(PIN_NAME_W)
        .clip(true),
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center);
    let Some(net) = &pin.net else {
        return container(line)
            .padding([SP.xs, SP.l])
            .width(Length::Fill)
            .boxed();
    };
    signex_widgets::button(line)
        .width(Length::Fill)
        .padding([SP.xs, SP.l])
        .on_press(msg(NetInspectorMsg::MemberClicked {
            net: net.clone(),
            anchor: pin.anchor.clone(),
        }))
        .style(crate::styles::menu_item(tokens))
        .boxed()
}
