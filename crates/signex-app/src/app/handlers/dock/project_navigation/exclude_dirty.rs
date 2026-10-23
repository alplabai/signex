//! Exclude-from-project on a sheet with unsaved edits (#738).
//!
//! The Remove modal's second stage: Save / Discard / Cancel. Cancel is
//! `RemoveMsg::Close`, so Esc and the close button need no extra wiring.

use super::*;
use crate::app::state::scope::{CaseFold, path_key_with};

impl Signex {
    /// Does `target` still hold unsaved edits (a dirty engine or editor)?
    pub(super) fn has_unsaved_edits(&mut self, target: &std::path::Path) -> bool {
        // Footprint edits only reach `dirty_paths` on a sync; make sure a
        // dirty footprint draft is seen here too.
        self.sync_footprint_dirty_paths();
        let fold = CaseFold::host_default();
        let key = path_key_with(target, fold);
        self.document_state
            .dirty_paths
            .iter()
            .any(|p| path_key_with(p, fold) == key)
    }

    /// Resolve the Save / Discard choice of the unsaved-edits stage.
    pub(crate) fn handle_exclude_dirty_choice(
        &mut self,
        choice: crate::app::ExcludeDirtyChoice,
    ) -> iced::Task<Message> {
        use crate::app::{ExcludeDirtyChoice, RemoveChoice};

        let Some(state) = self.ui_state.remove_dialog.take() else {
            return iced::Task::none();
        };
        if !state.unsaved_prompt {
            self.ui_state.remove_dialog = Some(state);
            return iced::Task::none();
        }
        match choice {
            ExcludeDirtyChoice::Save => {
                if let Err(err) = self.try_save_dirty_path(&state.target_path) {
                    crate::diagnostics::log_error(
                        &format!(
                            "Exclude from Project: could not save {}",
                            state.display_name
                        ),
                        &err,
                    );
                    self.document_state.error_notice =
                        Some(crate::app::state::ErrorNotice::save(format!(
                            "Could not save \"{}\", so it was not excluded: {err:#}",
                            state.display_name
                        )));
                    // Keep the prompt so the user can retry or cancel.
                    self.ui_state.remove_dialog = Some(state);
                    self.refresh_panel_ctx();
                    return iced::Task::none();
                }
                self.apply_remove(&state, RemoveChoice::ExcludeFromProject)
            }
            ExcludeDirtyChoice::Discard => {
                let task = self.apply_remove(&state, RemoveChoice::ExcludeFromProject);
                self.discard_unsaved_edits(&state.target_path, &state.display_name);
                // The discard was logged after `apply_remove` refreshed the panels.
                self.refresh_panel_ctx();
                task
            }
        }
    }

    /// Drop the engine / editor and dirty marker held for `target`, so a
    /// later Save All writes nothing for it.
    fn discard_unsaved_edits(&mut self, target: &std::path::Path, display_name: &str) {
        let fold = CaseFold::host_default();
        let key = path_key_with(target, fold);
        let doc = &mut self.document_state;
        let is_target = |p: &std::path::PathBuf| path_key_with(p, fold) == key;
        doc.engines.retain(|p, _| !is_target(p));
        doc.symbol_editors.retain(|p, _| !is_target(p));
        doc.footprint_editors.retain(|p, _| !is_target(p));
        doc.dirty_paths.retain(|p| !is_target(p));
        crate::diagnostics::log_warning(format!(
            "Excluded {display_name} from the project: discarded its unsaved edits"
        ));
    }
}
