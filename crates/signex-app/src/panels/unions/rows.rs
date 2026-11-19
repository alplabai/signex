//! The Unions panel's list (mockup `.urow`, 42 px rows): the union icon,
//! name and member count, the #tag pills, and the Select members, Lock /
//! Unlock and Hide / Show actions. The picked row is accent-soft with a
//! 2 px accent bar; a hidden union's row is dimmed. In Project scope the
//! rows are grouped under their sheet's name.

use std::sync::OnceLock;

use iced::widget::{Column, Row, Space, column, container, mouse_area, row, svg};
use iced::{Alignment, Background, Element, Length, Theme, Widget as _};
use signex_i18n::{UserText, ids, t};
use signex_widgets::text;

use super::{PanelContext, PanelMsg, SP, TS, icon, um};
use crate::app::unions::{UnionRow, UnionsMsg, tag_pill};
use crate::styles::{chrome_px, ti, tokens::resolve};

/// Row height (mockup: 42 px).
const ROW_H: f32 = chrome_px(42.0 * 11.0 / 12.0);
/// Width of the picked row's accent bar.
const BAR_W: f32 = 2.0;

const UNION: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 14 14"><rect x="1.5" y="1.5" width="11" height="11" rx="2" fill="none" stroke="currentColor" stroke-width="1.2" stroke-dasharray="2.5 1.5"/><rect x="4" y="4" width="6" height="6" rx="1" fill="currentColor" opacity="0.5"/></svg>"#;
const SELECT: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 14 14"><path d="M3 2l7.5 5-3.2.8L9 11.5l-1.6.7-1.7-3.7L3 10.5z" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linejoin="round"/></svg>"#;
pub(super) const LOCK: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 14 14"><rect x="3" y="6.5" width="8" height="6" rx="1" fill="currentColor"/><path d="M4.8 6.5V4.6a2.2 2.2 0 0 1 4.4 0v1.9" fill="none" stroke="currentColor" stroke-width="1.3"/></svg>"#;
pub(super) const UNLOCK: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 14 14"><rect x="3" y="6.5" width="8" height="6" rx="1" fill="currentColor"/><path d="M4.8 6.5V4.1a2.2 2.2 0 0 1 4.4 0" fill="none" stroke="currentColor" stroke-width="1.3"/></svg>"#;

pub(super) fn lock_handle(locked: bool) -> svg::Handle {
    static SHUT: OnceLock<svg::Handle> = OnceLock::new();
    static OPEN: OnceLock<svg::Handle> = OnceLock::new();
    let (cell, bytes) = if locked {
        (&SHUT, LOCK)
    } else {
        (&OPEN, UNLOCK)
    };
    cell.get_or_init(|| svg::Handle::from_memory(bytes)).clone()
}

fn handle(cell: &'static OnceLock<svg::Handle>, bytes: &'static [u8]) -> svg::Handle {
    cell.get_or_init(|| svg::Handle::from_memory(bytes)).clone()
}

/// Every group of rows; an empty list says so.
pub(super) fn list(ctx: &PanelContext) -> Element<'_, PanelMsg> {
    let v = &ctx.unions;
    let r = resolve(&ctx.tokens);
    let mut col: Column<Element<'_, PanelMsg>> = Column::new().width(Length::Fill);
    let mut any = false;
    for group in &v.groups {
        if let Some(title) = &group.title {
            col = col.push(
                container(
                    text(UserText::new(title.clone()))
                        .size(TS.caption)
                        .color(r.ink3),
                )
                .padding([SP.s, SP.m]),
            );
        }
        for union in &group.rows {
            any = true;
            col = col.push(union_row(ctx, union));
        }
    }
    if !any {
        col = col.push(
            container(
                text(t!(ctx.i18n, ids::UNIONS_PANEL_NONE))
                    .size(TS.body)
                    .color(r.ink3),
            )
            .padding(SP.m),
        );
    }
    col.boxed()
}

/// One row action: a quiet 14 px icon button with its tooltip.
fn action<'a>(
    ctx: &'a PanelContext,
    handle: svg::Handle,
    ink: iced::Color,
    tip: signex_i18n::Localized,
    msg: PanelMsg,
) -> Element<'a, PanelMsg> {
    let pressed = signex_widgets::button(icon(handle, ink))
        .padding(SP.xs)
        .style(crate::styles::ghost_button(&ctx.tokens))
        .on_press(msg)
        .boxed();
    crate::panels::graphite::hint_tooltip(
        pressed,
        tip,
        &ctx.tokens,
        iced::widget::tooltip::Position::Bottom,
    )
}

fn union_row<'a>(ctx: &'a PanelContext, u: &'a UnionRow) -> Element<'a, PanelMsg> {
    static UNION_ICON: OnceLock<svg::Handle> = OnceLock::new();
    static SELECT_ICON: OnceLock<svg::Handle> = OnceLock::new();
    let tokens = &ctx.tokens;
    let r = resolve(tokens);
    let (name_ink, caption_ink) = if u.hidden {
        (r.ink4, r.ink4)
    } else {
        (ti(tokens.text), r.ink3)
    };
    let name = row![
        icon(handle(&UNION_ICON, UNION), r.ink3),
        text(UserText::new(u.name.clone()))
            .size(TS.body)
            .color(name_ink)
            .font(iced::Font {
                weight: iced::font::Weight::Medium,
                ..iced::Font::DEFAULT
            }),
        text(UserText::new(ctx.i18n.count(u.members as i64)))
            .size(TS.caption)
            .color(caption_ink),
    ]
    .spacing(SP.xs)
    .align_y(Alignment::Center);
    let mut tags: Row<Element<'a, PanelMsg>> = Row::new().spacing(SP.xs);
    for tag in &u.tags {
        tags = tags.push(tag_pill(format!("#{tag}"), None, &ctx.i18n, tokens));
    }
    let left = column![name, tags].spacing(SP.xs).width(Length::Fill);
    let sel = || (u.sheet.clone(), u.uuid);
    let (sheet, union) = sel();
    let members = action(
        ctx,
        handle(&SELECT_ICON, SELECT),
        ti(tokens.text_secondary),
        t!(ctx.i18n, ids::UNIONS_PANEL_SELECT_MEMBERS),
        um(UnionsMsg::MembersSelected { sheet, union }),
    );
    let (sheet, union) = sel();
    let lock_tip = if u.locked {
        ids::UNIONS_PANEL_UNLOCK
    } else {
        ids::UNIONS_PANEL_LOCK
    };
    let lock_ink = if u.locked { ti(tokens.text) } else { r.ink3 };
    let lock = action(
        ctx,
        lock_handle(u.locked),
        lock_ink,
        t!(ctx.i18n, lock_tip),
        um(UnionsMsg::LockToggled { sheet, union }),
    );
    let (sheet, union) = sel();
    let hide_tip = if u.hidden {
        ids::UNIONS_PANEL_SHOW
    } else {
        ids::UNIONS_PANEL_HIDE
    };
    let hide = action(
        ctx,
        crate::panels::pcb_layers::eye_handle(u.hidden),
        if u.hidden {
            r.ink4
        } else {
            ti(tokens.text_secondary)
        },
        t!(ctx.i18n, hide_tip),
        um(UnionsMsg::HideToggled { sheet, union }),
    );
    let actions = row![members, lock, hide]
        .spacing(SP.xs)
        .align_y(Alignment::Center);
    let body = row![left, actions]
        .spacing(SP.s)
        .align_y(Alignment::Center)
        .height(Length::Fixed(ROW_H));
    let (fill, bar) = if u.picked {
        (Some(r.accent_soft), Some(ti(tokens.accent)))
    } else {
        (None, None)
    };
    let accent_bar = container(Space::new().width(BAR_W).height(Length::Fill))
        .width(BAR_W)
        .height(Length::Fixed(ROW_H))
        .style(move |_: &Theme| container::Style {
            background: bar.map(Background::Color),
            ..container::Style::default()
        });
    let line = row![
        accent_bar,
        container(body).padding([0.0, SP.m]).width(Length::Fill)
    ]
    .width(Length::Fill);
    let (sheet, union) = sel();
    mouse_area(
        container(line)
            .width(Length::Fill)
            .style(move |_: &Theme| container::Style {
                background: fill.map(Background::Color),
                ..container::Style::default()
            }),
    )
    .on_press(um(UnionsMsg::RowPicked { sheet, union }))
    .boxed()
}
