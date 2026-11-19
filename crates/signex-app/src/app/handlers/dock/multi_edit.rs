//! The Properties form of a multi-selection (#1258): one edit, every object
//! of the group, one undo step.

use super::super::super::*;
use crate::panels::{MultiEditMsg, multi_edit_from_text};
use iced::Task;

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

impl Signex {
    /// The selection forms' own messages (#1699): view state is kept in the
    /// panel context; Browse… beside a sheet symbol's File asks for a file
    /// off the update thread. A No-ERC "All rules" edit also ends the
    /// directive's rule picking, and an emptied Line Width commits the default
    /// width; both then go on to their own handlers (`None`).
    pub(super) fn handle_dock_selection_form_message(
        &mut self,
        panel_msg: &crate::panels::PanelMsg,
    ) -> Option<Task<Message>> {
        use crate::panels::{NoErcRuleEdit, PanelMsg, SelectionFormMsg};
        // An emptied Line Width box sets the default width back (#46).
        if let PanelMsg::CommitChildSheetStrokeWidth(_) = panel_msg {
            let buf = &mut self.document_state.panel_ctx.child_sheet_stroke_width_buf;
            if buf.as_deref().is_some_and(|text| text.trim().is_empty()) {
                *buf = Some("0".to_string());
            }
            return None;
        }
        let form = &mut self.document_state.panel_ctx.selection_form;
        if let PanelMsg::EditNoErcRules(id, NoErcRuleEdit::AllRules) = panel_msg {
            if form.picking_rules_of == Some(*id) {
                form.picking_rules_of = None;
            }
            return None;
        }
        let PanelMsg::SelectionForm(msg) = panel_msg else {
            return None;
        };
        if form.update(msg) {
            return Some(Task::none());
        }
        Some(match msg {
            SelectionFormMsg::SheetFileBrowseClicked(id) => self.browse_child_sheet_file(*id),
            SelectionFormMsg::SheetFileBrowsed(id, picked) => {
                self.child_sheet_file_browsed(*id, picked.as_deref());
                Task::none()
            }
            // `SelectionForm::update` took every other message.
            _ => Task::none(),
        })
    }
}
