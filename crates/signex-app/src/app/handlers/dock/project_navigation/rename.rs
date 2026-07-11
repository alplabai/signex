//! Rename flows for the project-navigation dock — file rename and
//!
//! Extracted verbatim from the project-navigation dock handlers
//! (`handlers/dock/project_navigation`); pure code motion, zero
//! behaviour change.

use super::*;

impl Signex {
    pub(super) fn open_rename_dialog(&mut self, tree_path: Vec<usize>) {
        let Some(target_path) = self.tree_path_to_file_path(&tree_path) else {
            return;
        };
        let filename = target_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        self.ui_state.rename_dialog = Some(crate::app::RenameDialogState {
            target_path,
            tree_path,
            buffer: filename,
            error: None,
            is_project_rename: false,
        });
    }

    /// Open the rename modal seeded with the project name (the `.snxprj`
    /// file stem). On submit, [`handle_rename_submit`] sees
    /// `is_project_rename = true` and renames the trio
    /// `<old>.snxprj` / `<old>.snxsch` / `<old>.snxpcb` together.
    pub(crate) fn open_project_rename_dialog(&mut self, tree_path: Vec<usize>) {
        let Some(&project_idx) = tree_path.first() else {
            return;
        };
        let Some(project) = self.document_state.projects.get(project_idx) else {
            return;
        };
        let target_path = project.path.clone();
        let stem = target_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        self.ui_state.rename_dialog = Some(crate::app::RenameDialogState {
            target_path,
            tree_path,
            buffer: stem,
            error: None,
            is_project_rename: true,
        });
    }

    pub(crate) fn handle_rename_submit(&mut self) -> iced::Task<Message> {
        let Some(state) = self.ui_state.rename_dialog.clone() else {
            return iced::Task::none();
        };

        if state.is_project_rename {
            return self.handle_project_rename_submit(&state);
        }

        let new_name = state.buffer.trim();
        if new_name.is_empty() {
            self.set_rename_error("Name cannot be empty.");
            return iced::Task::none();
        }
        // Reject path separators so users can't escape the project dir
        // and so a fs::rename never crosses filesystem boundaries.
        if new_name.contains(['/', '\\']) {
            self.set_rename_error("Name cannot contain path separators.");
            return iced::Task::none();
        }
        let parent = match state.target_path.parent() {
            Some(p) => p,
            None => {
                self.set_rename_error("Target file has no parent directory.");
                return iced::Task::none();
            }
        };
        let new_path = parent.join(new_name);
        if new_path == state.target_path {
            // No-op rename — close the dialog silently.
            self.ui_state.rename_dialog = None;
            return iced::Task::none();
        }
        if new_path.exists() {
            self.set_rename_error("A file with that name already exists.");
            return iced::Task::none();
        }

        if let Err(e) = std::fs::rename(&state.target_path, &new_path) {
            self.set_rename_error(&format!("Rename failed: {e}"));
            return iced::Task::none();
        }

        // Update in-memory project data. NOTE: this does not rewrite
        // parent sheets that reference this child sheet — if the
        // renamed file is referenced from a parent `sheet` S-expr,
        // the parent still points to the old name. Recorded as a
        // known limitation; proper rewrite lands with project-write
        // support (v0.9). For now, sheets at the root level (no
        // parent reference) rename cleanly.
        let old_filename = state
            .target_path
            .file_name()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string());
        // Mutate the *owning* project's data via the dialog's
        // tree_path[0], not the active project — renaming a sheet of
        // project B while project A is active should still touch B.
        // (#54)
        let owner_idx = state.tree_path.first().copied();
        if let (Some(old), Some(idx)) = (old_filename.as_ref(), owner_idx)
            && let Some(loaded) = self.document_state.projects.get_mut(idx)
        {
            for entry in loaded.data.sheets.iter_mut() {
                if &entry.filename == old {
                    entry.filename = new_name.to_string();
                }
            }
            if loaded.data.schematic_root.as_deref() == Some(old.as_str()) {
                loaded.data.schematic_root = Some(new_name.to_string());
            }
        }

        // Update any open tabs + engine-map keys that pointed at the
        // old path. Do this before `refresh_panel_ctx` so the tree
        // rebuild sees the new filenames.
        let old_path = state.target_path.clone();
        for tab in self.document_state.tabs.iter_mut() {
            if tab.path == old_path {
                tab.path = new_path.clone();
                if let Some(stem) = new_path.file_stem().and_then(|s| s.to_str()) {
                    tab.title = stem.to_string();
                }
            }
        }
        if let Some(engine) = self.document_state.engines.remove(&old_path) {
            self.document_state.engines.insert(new_path.clone(), engine);
        }
        if self.document_state.active_path.as_ref() == Some(&old_path) {
            self.document_state.active_path = Some(new_path.clone());
        }

        self.ui_state.rename_dialog = None;
        self.refresh_panel_ctx();
        iced::Task::none()
    }

    fn set_rename_error(&mut self, msg: &str) {
        if let Some(d) = self.ui_state.rename_dialog.as_mut() {
            d.error = Some(msg.to_string());
        }
    }

    /// Project-root rename — `state.target_path` is the project's
    /// `.snxprj`; the buffer is the new *stem*. We rename ONLY the
    /// `.snxprj` file and update the project's in-memory `name`.
    /// Companion schematic / pcb files keep their existing filenames —
    /// they're independent entities, possibly shared across workflows
    /// or referenced from version control with their original names.
    /// The `.snxprj`'s `schematic_root` / `pcb_file` are filename
    /// strings that continue to point at the unchanged sheet files.
    fn handle_project_rename_submit(
        &mut self,
        state: &crate::app::RenameDialogState,
    ) -> iced::Task<Message> {
        let new_stem = state.buffer.trim();
        if new_stem.is_empty() {
            self.set_rename_error("Name cannot be empty.");
            return iced::Task::none();
        }
        if new_stem.contains(['/', '\\', '.']) {
            self.set_rename_error("Name cannot contain '/', '\\', or '.'.");
            return iced::Task::none();
        }
        let dir = match state.target_path.parent() {
            Some(d) => d.to_path_buf(),
            None => {
                self.set_rename_error("Project file has no parent directory.");
                return iced::Task::none();
            }
        };
        let old_stem = match state.target_path.file_stem().and_then(|s| s.to_str()) {
            Some(s) => s.to_string(),
            None => {
                self.set_rename_error("Project file has no stem.");
                return iced::Task::none();
            }
        };
        if new_stem == old_stem {
            self.ui_state.rename_dialog = None;
            return iced::Task::none();
        }
        let new_prj = dir.join(format!("{new_stem}.snxprj"));
        if new_prj.exists() {
            self.set_rename_error("A project with that name already exists.");
            return iced::Task::none();
        }

        // Rename only the .snxprj. Schematic / PCB files keep their
        // original filenames; the project's `schematic_root` /
        // `pcb_file` references continue to point at them.
        if let Err(e) = std::fs::rename(&state.target_path, &new_prj) {
            self.set_rename_error(&format!("Rename failed: {e}"));
            return iced::Task::none();
        }

        // Update the in-memory project record + any tab / engine that
        // referenced the .snxprj path itself.
        let owner_idx = state.tree_path.first().copied();
        if let Some(idx) = owner_idx
            && let Some(loaded) = self.document_state.projects.get_mut(idx)
        {
            loaded.path = new_prj.clone();
            loaded.data.name = new_stem.to_string();
        }

        let from = state.target_path.clone();
        let to = new_prj.clone();
        for tab in self.document_state.tabs.iter_mut() {
            if tab.path == from {
                tab.path = to.clone();
                if let Some(stem) = to.file_stem().and_then(|s| s.to_str()) {
                    tab.title = stem.to_string();
                }
            }
        }
        if let Some(engine) = self.document_state.engines.remove(&from) {
            self.document_state.engines.insert(to.clone(), engine);
        }
        if self.document_state.active_path.as_ref() == Some(&from) {
            self.document_state.active_path = Some(to.clone());
        }
        if self.document_state.dirty_paths.remove(&from) {
            self.document_state.dirty_paths.insert(to.clone());
        }

        self.ui_state.rename_dialog = None;
        self.refresh_panel_ctx();
        iced::Task::none()
    }
}
