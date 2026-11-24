//! The Unions panel's foot for the picked union (mockup row 158): its name as
//! an input (a rename is one undo step), the member count, member pills and
//! the wire count, and Add selection / Remove selection / Break union, which
//! wrap on a narrow dock. A hidden union shows that it is hidden, with Show,
//! instead of member editing.

use iced::widget::{Row, Space, column, container, row};
use iced::{Alignment, Element, Length, Widget as _};
use signex_i18n::{UserText, ids, t};
use signex_widgets::{text, text_input};

use super::{PanelContext, PanelMsg, SP, TS, icon, um};
use crate::app::unions::{UnionFoot, UnionsMsg, tag_pill};
use crate::styles::{DialogButtonKind, dialog_button, ti, tokens::resolve};

pub(super) fn view(ctx: &PanelContext) -> Option<Element<'_, PanelMsg>> {
    let foot = ctx.unions.foot.as_ref()?;
    let body = if foot.hidden {
        hidden(ctx, foot)
    } else {
        members(ctx, foot)
    };
    Some(
        container(body)
            .padding([SP.s, SP.m])
            .width(Length::Fill)
            .boxed(),
    )
}

fn hidden<'a>(ctx: &'a PanelContext, foot: &'a UnionFoot) -> Element<'a, PanelMsg> {
    let tokens = &ctx.tokens;
    let r = resolve(tokens);
    let show = dialog_button(
        t!(ctx.i18n, ids::UNIONS_PANEL_SHOW),
        DialogButtonKind::Secondary,
        Some(um(UnionsMsg::HideToggled {
            sheet: foot.sheet.clone(),
            union: foot.uuid,
        })),
        tokens,
    );
    let line = row![
        icon(crate::panels::pcb_layers::eye_handle(true), r.ink3),
        text(t!(ctx.i18n, ids::UNIONS_FOOT_HIDDEN, "name" => foot.name.as_str()))
            .size(TS.body)
            .color(ti(tokens.text))
            .width(Length::Fill),
        show,
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center);
    column![
        line,
        text(t!(ctx.i18n, ids::UNIONS_FOOT_HIDDEN_NOTE))
            .size(TS.caption)
            .color(r.ink3),
    ]
    .spacing(SP.s)
    .boxed()
}

fn members<'a>(ctx: &'a PanelContext, foot: &'a UnionFoot) -> Element<'a, PanelMsg> {
    let tokens = &ctx.tokens;
    let r = resolve(tokens);
    let name = row![
        text_input(UserText::new(String::new()), &foot.name)
            .on_input_maybe((!foot.locked).then_some(|s: String| um(UnionsMsg::RenameEdited(s))))
            .size(TS.body)
            .width(Length::Fill),
        text(t!(ctx.i18n, ids::UNIONS_FOOT_MEMBERS, "count" => foot.members as i64))
            .size(TS.caption)
            .color(r.ink3),
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center);
    let mut pills: Row<Element<'a, PanelMsg>> =
        Row::new().spacing(SP.xs).align_y(Alignment::Center);
    for pill in &foot.pills {
        pills = pills.push(tag_pill(pill.clone(), None, &ctx.i18n, tokens));
    }
    if foot.wires > 0 {
        pills = pills.push(
            text(t!(ctx.i18n, ids::UNIONS_FOOT_WIRES, "count" => foot.wires as i64))
                .size(TS.caption)
                .color(r.ink3)
                .boxed(),
        );
    }
    if foot.others > 0 {
        pills = pills.push(
            text(t!(ctx.i18n, ids::UNIONS_FOOT_OTHERS, "count" => foot.others as i64))
                .size(TS.caption)
                .color(r.ink3)
                .boxed(),
        );
    }
    let buttons: Row<Element<'a, PanelMsg>> = Row::new()
        .spacing(SP.s)
        .align_y(Alignment::Center)
        .push(dialog_button(
            t!(ctx.i18n, ids::UNIONS_FOOT_ADD),
            DialogButtonKind::Secondary,
            foot.can_add.then_some(um(UnionsMsg::AddSelectionRequested)),
            tokens,
        ))
        .push(dialog_button(
            t!(ctx.i18n, ids::UNIONS_FOOT_REMOVE),
            DialogButtonKind::Secondary,
            foot.can_remove
                .then_some(um(UnionsMsg::RemoveSelectionRequested)),
            tokens,
        ))
        .push(Space::new().width(Length::Fill).boxed())
        .push(dialog_button(
            t!(ctx.i18n, ids::UNIONS_FOOT_BREAK),
            DialogButtonKind::Ghost,
            (!foot.locked).then_some(um(UnionsMsg::BreakRequested)),
            tokens,
        ));
    column![
        name,
        pills.wrap().vertical_spacing(SP.xs),
        buttons.wrap().vertical_spacing(SP.xs),
    ]
    .spacing(SP.s)
    .boxed()
}
