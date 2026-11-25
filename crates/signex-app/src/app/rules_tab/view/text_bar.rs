//! The text view's bar (mockup row 91). The text holds every rule, so the
//! domain tabs give way to "All domains" and a "Text view" tag; then the
//! Table and Text switch, Constraint sets, Presets and Run DRC.

use iced::alignment::Vertical;
use iced::widget::{Space, row};
use iced::{Element, Length, Widget as _};
use signex_i18n::{ids, t};
use signex_widgets::text;

use super::grid_toolbar::{metrics, presets_button, quiet_pill, run_drc, sets_button, view_switch};
use super::{Frame, SP, TS};
use crate::app::Message;
use crate::styles::tokens::resolve;

pub(super) fn view<'a>(frame: &Frame<'a>) -> Element<'a, Message> {
    let i18n = frame.i18n;
    let scope = text(t!(i18n, ids::RULES_TEXT_ALL_DOMAINS))
        .size(TS.body)
        .color(resolve(frame.tokens).ink3);
    let tag = quiet_pill(
        frame,
        text(t!(i18n, ids::RULES_TEXT_VIEW_TAG))
            .size(metrics().badge_text)
            .boxed(),
    );
    row![
        scope,
        tag,
        Space::new().width(Length::Fill),
        view_switch(frame),
        sets_button(frame),
        presets_button(frame),
        run_drc(frame),
    ]
    .spacing(SP.s)
    .align_y(Vertical::Center)
    .boxed()
}
