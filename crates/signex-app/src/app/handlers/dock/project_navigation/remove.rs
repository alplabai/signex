//! Remove-from-project / delete-file flow for the project-navigation dock.
//!
//! Extracted verbatim from the project-navigation dock handlers
//! (`handlers/dock/project_navigation`); pure code motion, zero
//! behaviour change.

use super::*;

impl Signex {
    pub(super) fn open_remove_dialog(&mut self, tree_path: Vec<usize>) {
        let Some(target_path) = self.tree_path_to_file_path(&tree_path) else {
            return;
        };
        let display_name = target_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        self.ui_state.remove_dialog = Some(crate::app::RemoveDialogState {
            target_path,
            tree_path,
            display_name,
        });
    }

    pub(crate) fn handle_remove_confirm(
        &mut self,
        choice: crate::app::RemoveChoice,
    ) -> iced::Task<Message> {
        use crate::app::RemoveChoice;

        let Some(state) = self.ui_state.remove_dialog.take() else {
            return iced::Task::none();
        };

        // Close any tab backed by this file before we touch disk or
        // drop it from the project's in-memory list — otherwise the
        // tab would keep referring to a file that no longer exists.
        let mut close_tasks: Vec<iced::Task<Message>> = Vec::new();
        while let Some(idx) = self
            .document_state
            .tabs
            .iter()
            .position(|t| t.path == state.target_path)
        {
            close_tasks.push(self.close_tab_now(idx));
        }

        if matches!(choice, RemoveChoice::DeleteFile) {
            // F23 — handle three cases:
            //   1. Regular file (sheets, pcbs, primitives) → remove_file.
            //   2. `.snxlib` directory package → remove_dir_all.
            //   3. Orphan entry (file/dir missing on disk) → no-op,
            //      Exclude semantic falls through naturally so the
            //      LibraryEntry / SheetEntry is still pruned below.
            let target = &state.target_path;
            if target.exists() {
                let result = if target.is_dir() {
                    std::fs::remove_dir_all(target)
                } else {
                    std::fs::remove_file(target)
                };
                if let Err(e) = result {
                    crate::diagnostics::log_error(
                        "Failed to delete project file",
                        &anyhow::anyhow!("{e}"),
                    );
                }
            }
            // Orphan target: silently fall through. The user's intent
            // ("get this row out of the project") is satisfied by the
            // in-memory entry pruning below.
        }

        // Drop the sheet from the *owning* project's in-memory data so
        // the tree rebuild reflects the removal. Resolves project via
        // the dialog's `tree_path[0]` so removing a sheet of project B
        // doesn't accidentally mutate active project A. Session-scoped:
        // reopening the project rescans disk and (for Exclude)
        // re-discovers the file, until proper project-write support
        // lands. (#54)
        let owner_idx = state.tree_path.first().copied();
        let mut mutated = false;
        let mut project_path: Option<std::path::PathBuf> = None;
        if let Some(filename) = state
            .target_path
            .file_name()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string())
            && let Some(idx) = owner_idx
            && let Some(loaded) = self.document_state.projects.get_mut(idx)
        {
            project_path = Some(loaded.path.clone());
            let before = loaded.data.sheets.len();
            loaded
                .data
                .sheets
                .retain(|entry| entry.filename != filename);
            if loaded.data.sheets.len() != before {
                mutated = true;
            }
            if loaded.data.schematic_root.as_deref() == Some(filename.as_str()) {
                loaded.data.schematic_root = None;
                mutated = true;
            }
            if loaded.data.pcb_file.as_deref() == Some(filename.as_str()) {
                loaded.data.pcb_file = None;
                mutated = true;
            }
            // F23 — also prune library entries. Library file_name on
            // the entry is the `.snxlib` filename for both
            // ProjectLocal (relative path stored on the entry) and
            // Shared (absolute path stored), so the same file_name
            // match works for both kinds.
            let lib_before = loaded.data.libraries.len();
            loaded.data.libraries.retain(|entry| {
                entry
                    .path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .map(|n| n != filename)
                    .unwrap_or(true)
            });
            if loaded.data.libraries.len() != lib_before {
                mutated = true;
            }
            // Drop any pending registration too, in case the user
            // saved-then-removed within a single session before
            // materialise ran (rare but possible if materialise failed
            // and the entry stayed pending).
            let pending_before = loaded.pending_libraries.len();
            loaded.pending_libraries.retain(|_, spec| {
                spec.lib_path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .map(|n| n != filename)
                    .unwrap_or(true)
            });
            if loaded.pending_libraries.len() != pending_before {
                mutated = true;
            }
        }
        if mutated && let Some(p) = project_path {
            // Match Add Existing — Remove from Project also dirties the
            // .snxprj so the project root row gets the red dot until
            // the user saves the metadata change. The on-disk file
            // delete already happened above when DeleteFile was picked;
            // the dirty bit only tracks the project's *list* of
            // children, which lives in the .snxprj.
            self.document_state.dirty_paths.insert(p);
        }

        self.refresh_panel_ctx();
        if close_tasks.is_empty() {
            iced::Task::none()
        } else {
            iced::Task::batch(close_tasks)
        }
    }
}
