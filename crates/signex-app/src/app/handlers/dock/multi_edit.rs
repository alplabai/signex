//! The Properties form of a multi-selection (#1258): one edit, every object
//! of the group, one undo step.

use super::super::super::*;
use crate::panels::{MultiEditMsg, multi_edit_from_text};

impl Signex {
    pub(super) fn handle_dock_multi_edit_message(
        &mut self,
        panel_msg: &crate::panels::PanelMsg,
    ) -> bool {
        let crate::panels::PanelMsg::MultiEdit(msg) = panel_msg else {
            return false;
        };
        match msg {
            MultiEditMsg::DraftChanged(field, text) => {
                self.document_state.panel_ctx.multi_draft = Some((field.clone(), text.clone()));
            }
            MultiEditMsg::DraftSubmitted(field) => {
                let Some((_, typed)) = self
                    .document_state
                    .panel_ctx
                    .multi_draft
                    .clone()
                    .filter(|(f, _)| f == field)
                else {
                    return true;
                };
                match multi_edit_from_text(field, &typed) {
                    Ok(edit) => self.apply_multi_edit(&edit),
                    // The draft stays, so the user can fix the text.
                    Err(why) => crate::diagnostics::log_warning(why),
                }
            }
            MultiEditMsg::Picked(edit) => self.apply_multi_edit(edit),
        }
        true
    }

    /// `edit` on every selected object of its kind, as one history entry.
    pub(crate) fn apply_multi_edit(&mut self, edit: &signex_engine::MultiEdit) {
        let items = self.interaction_state.active_canvas().selected.clone();
        let Some(engine) = self.document_state.active_engine() else {
            return;
        };
        let commands = signex_engine::multi_edit_commands(engine.document(), &items, edit);
        if commands.is_empty() {
            crate::diagnostics::log_warning(
                "Properties: no selected object has that field, nothing was changed",
            );
            return;
        }
        // A refused batch is logged by the gateway.
        self.apply_engine_commands(commands, true, true);
    }
}
