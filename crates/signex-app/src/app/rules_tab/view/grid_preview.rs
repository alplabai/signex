//! The left column of the detail strip: the selected cell's live drawing
//! (`rules_tab::drawing`), its values editable on the drawing, or a note
//! when nothing is selected or the column has no drawing yet.

use iced::widget::column;
use iced::{Element, Widget as _};
use signex_i18n::{ids, t};
use signex_widgets::text;

use super::detail::section;
use super::{Frame, SP, TS, up};
use crate::app::Message;
use crate::app::rules_tab::RulesTabMsg;
use crate::app::rules_tab::drawing;
use crate::styles::tokens;

pub(super) fn view<'a>(frame: &Frame<'a>) -> Element<'a, Message> {
    let state = frame.state;
    let id = frame.id;
    if let Some(target) = state.drawing_target.as_ref() {
        return drawing::view(&state.drawing, target, frame.i18n, frame.tokens)
            .map(move |msg| {
                up(RulesTabMsg::PreviewActed {
                    project_id: id,
                    msg,
                })
            })
            .boxed();
    }
    let note = if state.grid.view.selected().is_some() {
        ids::RULES_GRID_PREVIEW_NONE
    } else {
        ids::RULES_GRID_PREVIEW_SELECT
    };
    column![
        section(frame, ids::RULES_PREVIEW_HEADING),
        text(t!(frame.i18n, note))
            .size(TS.body)
            .color(tokens::muted_text(frame.tokens)),
    ]
    .spacing(SP.s)
    .boxed()
}
