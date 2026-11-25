//! The panel before a compile: the dashed "Not compiled yet" card with its
//! Compile project button, or a line while a compile runs or when no sheet
//! of a project is open.

use iced::widget::{column, container};
use iced::{Alignment, Border, Element, Length, Theme, Widget as _};
use signex_i18n::{ids, t};
use signex_widgets::text;

use super::{CompiledStatus, PanelContext, PanelMsg, SP, TS, msg};
use crate::app::CompiledDesignMsg;
use crate::styles::{Radii, ti, tokens::resolve};

pub(super) fn view<'a>(ctx: &'a PanelContext, status: CompiledStatus) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let tokens = &ctx.tokens;
    let res = resolve(tokens);
    let note = |id: signex_i18n::MessageId| -> Element<'a, PanelMsg> {
        container(text(t!(i18n, id)).size(TS.caption).color(res.ink3))
            .padding([SP.m, SP.l])
            .width(Length::Fill)
            .height(Length::Fill)
            .boxed()
    };
    match status {
        CompiledStatus::NoProject => return note(ids::PANELS_COMPILED_DESIGN_NO_PROJECT),
        CompiledStatus::Compiling => return note(ids::PANELS_COMPILED_DESIGN_COMPILING),
        CompiledStatus::NotCompiled | CompiledStatus::Ready => {}
    }
    let compile =
        signex_widgets::button(text(t!(i18n, ids::PANELS_COMPILED_DESIGN_COMPILE)).size(TS.body))
            .padding([SP.s, SP.l])
            .on_press(msg(CompiledDesignMsg::CompileRequested))
            .style(crate::styles::primary_button(tokens));
    let edge = res.line_strong;
    let card = container(
        column![
            text(t!(i18n, ids::PANELS_COMPILED_DESIGN_EMPTY_TITLE))
                .size(TS.body)
                .color(ti(tokens.text)),
            text(t!(i18n, ids::PANELS_COMPILED_DESIGN_EMPTY_BODY))
                .size(TS.caption)
                .color(res.ink3)
                .align_x(iced::alignment::Horizontal::Center),
            container(compile).padding(iced::Padding {
                top: SP.s,
                ..iced::Padding::ZERO
            }),
            text(t!(i18n, ids::PANELS_COMPILED_DESIGN_EMPTY_NOTE))
                .size(TS.caption)
                .color(res.ink3)
                .align_x(iced::alignment::Horizontal::Center),
        ]
        .spacing(SP.s)
        .align_x(Alignment::Center),
    )
    .padding([SP.xl, SP.l])
    .width(Length::Fill)
    .style(move |_: &Theme| container::Style {
        border: Border {
            color: edge,
            width: 1.0,
            radius: Radii::GRAPHITE.card.into(),
            ..Border::default()
        },
        ..container::Style::default()
    });
    container(card)
        .padding(SP.m)
        .width(Length::Fill)
        .height(Length::Fill)
        .boxed()
}
