//! The typed Visible Grid and Snap Grid of Document Options (mockup row 40):
//! the text waits in the panel context until Enter; refused text stays in
//! the box, marked, and the pitch does not change.

use iced::Task;

use super::super::super::*;
use crate::dock::DockMessage;
use crate::panels::{DocGridField, DocGridMsg, PanelMsg, typed_pitch_mm};

impl Signex {
    pub(super) fn handle_doc_grid(&mut self, msg: &DocGridMsg) -> Task<Message> {
        match msg {
            DocGridMsg::DraftChanged(field, text) => {
                *self.doc_grid_draft_mut(*field) = Some(text.clone());
                Task::none()
            }
            DocGridMsg::DraftSubmitted(field) => self.submit_doc_grid(*field),
        }
    }

    fn submit_doc_grid(&mut self, field: DocGridField) -> Task<Message> {
        let Some(typed) = self.doc_grid_draft_mut(field).clone() else {
            return Task::none();
        };
        match typed_pitch_mm(&typed, self.ui_state.unit) {
            Ok(mm) => {
                let set: PanelMsg = field.set_message(mm);
                self.update(Message::Dock(DockMessage::Panel(set)))
            }
            Err(why) => {
                crate::diagnostics::log_warning(format!("{} refused: {why}", field.name()));
                Task::none()
            }
        }
    }

    fn doc_grid_draft_mut(&mut self, field: DocGridField) -> &mut Option<String> {
        let ctx = &mut self.document_state.panel_ctx;
        match field {
            DocGridField::Visible => &mut ctx.visible_grid_draft,
            DocGridField::Snap => &mut ctx.snap_grid_draft,
        }
    }
}
