//! The Variant Manager of a project with Base only (#1293): Base's card and
//! a hint that says what a variant records, with New variant from Base (the
//! same add as `+`).

use iced::widget::{column, container, row};
use iced::{Alignment, Element, Length, Widget as _};
use signex_i18n::{ids, t};
use signex_widgets::{button, text};

use super::super::ProjectTabMsg;
use super::super::general::tab_message;
use super::super::table::{SP, TS, note, problem};
use super::{GLYPH_PLUS, Manager, Parts, glyph, semibold, summary};
use crate::app::Message;
use crate::styles::ti;
use crate::styles::tokens::resolve;

/// Widest Base's card grows, in `body`-size characters.
const CARD_EMS: f32 = 220.0 / 12.0;
/// Widest the hint grows, in `body`-size characters.
const HINT_EMS: f32 = 440.0 / 12.0;

pub(super) fn view<'a>(m: &Manager<'_>) -> Element<'a, Message> {
    let r = resolve(m.tokens);
    let new_variant = button(
        row![
            glyph(GLYPH_PLUS, TS.body, r.primary_ink),
            text(t!(m.i18n, ids::PROJECT_VARIANTS_NEW_FROM_BASE)).size(TS.body),
        ]
        .spacing(SP.s)
        .align_y(Alignment::Center),
    )
    .padding([SP.s, SP.l])
    .style(crate::styles::primary_button(m.tokens))
    .on_press(tab_message(ProjectTabMsg::NewVariantRequested {
        project_id: m.project_id,
    }));
    let hint = column![
        text(t!(m.i18n, ids::PROJECT_VARIANTS_EMPTY_TITLE))
            .size(TS.title)
            .font(semibold())
            .color(ti(m.tokens.text)),
        text(t!(m.i18n, ids::PROJECT_VARIANTS_EMPTY_BODY))
            .size(TS.body)
            .color(r.ink3)
            .align_x(iced::alignment::Horizontal::Center),
        new_variant,
    ]
    .spacing(SP.m)
    .align_x(Alignment::Center)
    .width(Length::Fill.max(TS.body * HINT_EMS));
    let mut col = column![summary::card(m, None, Length::Fill.max(TS.body * CARD_EMS))]
        .spacing(SP.xl)
        .padding(SP.l)
        .width(Length::Fill)
        .height(Length::Fill);
    match m.parts {
        Parts::Reading => {
            col = col.push(note(t!(m.i18n, ids::PROJECT_VARIANTS_READING), m.tokens));
        }
        Parts::Failed(why) => {
            col = col.push(problem(
                t!(m.i18n, ids::PROJECT_VARIANTS_READ_FAILED, "reason" => why),
                m.tokens,
            ));
        }
        Parts::Read(_) => {}
    }
    col.push(container(hint).center_x(Length::Fill).boxed())
        .boxed()
}
