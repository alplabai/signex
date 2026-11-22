//! Properties while a channel tab of a repeated sheet is shown (#1279): the
//! channel, its place among the channels, and the designator map (each
//! part's logical designator, value and designator in this channel). The
//! view is read-only. Per-channel variant state waits for the Variant
//! Manager tab and is not shown.

use iced::widget::{Column, column, container, row, scrollable};
use iced::{Alignment, Element, Length, Widget as _};
use signex_i18n::{UserText, ids, t};
use signex_widgets::text;

use super::super::{PanelContext, PanelMsg};
use crate::styles::{Spacing, TypeScale, ti, tokens::resolve};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Width of the Logical column: 60 px at the 12 px body.
const LOGICAL_W: f32 = TS.body * 60.0 / 12.0;

/// The channel Properties, or `None` when the active sheet shows Editor.
pub(in crate::panels) fn channel_view_properties<'a>(
    ctx: &'a PanelContext,
) -> Option<Element<'a, PanelMsg>> {
    let strip = ctx.compiled_design.strip.as_ref()?;
    let index = strip.active?;
    let tab = strip.tabs.iter().find(|t| t.channel.index == index)?;
    let i18n = &*ctx.i18n;
    let tokens = &ctx.tokens;
    let res = resolve(tokens);
    let mono = crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT);
    let header = column![
        text(t!(
            i18n,
            ids::PROPERTIES_CHANNEL_TITLE,
            "channel" => tab.channel.name.as_str()
        ))
        .size(TS.body)
        .color(ti(tokens.text))
        .font(iced::Font {
            weight: iced::font::Weight::Semibold,
            ..signex_widgets::ui_font::regular()
        }),
        text(t!(
            i18n,
            ids::PROPERTIES_CHANNEL_CAPTION,
            "file" => strip.file.as_str(),
            "index" => tab.channel.index,
            "count" => tab.channel.of
        ))
        .size(TS.caption)
        .color(res.ink3),
    ]
    .spacing(SP.xs);
    let mut map: Column<Element<'a, PanelMsg>> = column![row![
        text(t!(i18n, ids::PROPERTIES_CHANNEL_LOGICAL))
            .size(TS.section)
            .color(res.ink3)
            .width(Length::Fill),
        text(UserText::new(tab.channel.name.as_str()))
            .size(TS.section)
            .color(res.ink3),
    ]]
    .spacing(SP.s)
    .padding([SP.s, SP.m]);
    for (logical, value, channel) in &strip.designator_map {
        map = map.push(
            row![
                text(UserText::new(logical.as_str()))
                    .size(TS.body)
                    .font(mono)
                    .width(LOGICAL_W),
                text(UserText::new(value.as_str()))
                    .size(TS.caption)
                    .color(res.ink3)
                    .width(Length::Fill),
                text(UserText::new(channel.as_str()))
                    .size(TS.body)
                    .font(mono),
            ]
            .align_y(Alignment::Center)
            .boxed(),
        );
    }
    let section = ctx.collapsible(
        "channel_designator_map",
        ids::PROPERTIES_CHANNEL_DESIGNATOR_MAP,
        &ctx.collapsed_sections,
        move || column![map.boxed()],
    );
    Some(
        scrollable(
            column![
                container(header).padding([SP.m, SP.m]).width(Length::Fill),
                section,
            ]
            .width(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .boxed(),
    )
}
