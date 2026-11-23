//! The Nets mode: every compiled net with the number of sheets it crosses;
//! power nets in the schematic power colour. A click goes to the net.

use iced::widget::{Column, column, container, row, scrollable, svg};
use iced::{Alignment, Background, Element, Length, Theme, Widget as _};
use signex_i18n::{UserText, ids, t};
use signex_widgets::text;

use super::tree::{COUNT_W, ROW_H, heading};
use super::{CompiledPanelView, PanelContext, PanelMsg, SP, TS, msg, rule};
use crate::app::CompiledDesignMsg;
use crate::styles::{ti, tokens::resolve};

/// Size of the net icon.
const ICON: f32 = TS.body * 14.0 / 12.0;

pub(super) fn view<'a>(
    ctx: &'a PanelContext,
    view: &'a CompiledPanelView,
) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let tokens = &ctx.tokens;
    let muted = resolve(tokens).ink3;
    let mono = crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT);
    let (info, power, ink) = (
        resolve(tokens).info,
        ti(ctx.canvas_colors.power),
        ti(tokens.text),
    );
    let hover = ti(tokens.hover);
    let mut list: Column<Element<'a, PanelMsg>> = column![
        heading(
            ctx,
            t!(i18n, ids::PANELS_COMPILED_DESIGN_COLUMN_NET),
            t!(i18n, ids::PANELS_COMPILED_DESIGN_COLUMN_SHEETS),
            None,
        ),
        rule(ctx),
    ]
    .width(Length::Fill);
    if view.nets.is_empty() {
        list = list.push(
            container(
                text(t!(i18n, ids::PANELS_COMPILED_DESIGN_NO_MATCH))
                    .size(TS.body)
                    .color(muted),
            )
            .padding([SP.m, SP.l])
            .boxed(),
        );
    }
    for net in &view.nets {
        let tint = if net.is_power { power } else { info };
        let name_ink = if net.is_power { power } else { ink };
        let icon = svg(crate::icons::icon_dd_net_label(ctx.theme_id))
            .width(ICON)
            .height(ICON)
            .style(move |_: &iced::Theme, _| svg::Style {
                color: Some(tint),
                ..Default::default()
            });
        let line = row![
            icon,
            container(
                text(UserText::new(net.name.as_str()))
                    .size(TS.body)
                    .font(mono)
                    .color(name_ink)
                    .wrapping(iced::widget::text::Wrapping::None),
            )
            .width(Length::Fill)
            .clip(true),
            text(UserText::new(net.sheets.to_string()))
                .size(TS.caption)
                .font(mono)
                .color(ink)
                .width(COUNT_W)
                .align_x(iced::alignment::Horizontal::Right),
            iced::widget::Space::new()
                .width(COUNT_W)
                .height(Length::Shrink),
        ]
        .spacing(SP.s)
        .height(ROW_H)
        .align_y(Alignment::Center);
        let mut button = signex_widgets::button(line)
            .width(Length::Fill)
            .padding([0.0, SP.l])
            .style(move |_: &Theme, status: iced::widget::button::Status| {
                iced::widget::button::Style {
                    background: matches!(status, iced::widget::button::Status::Hovered)
                        .then_some(Background::Color(hover)),
                    ..iced::widget::button::Style::default()
                }
            });
        if let Some(target) = &net.target {
            button = button.on_press(msg(CompiledDesignMsg::TargetClicked(target.clone())));
        }
        list = list.push(button.boxed());
    }
    scrollable(list)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(super::super::form_look::scrollbar_style)
        .boxed()
}
