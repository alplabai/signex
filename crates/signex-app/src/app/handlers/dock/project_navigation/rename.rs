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
        // A native file keeps its extension: dropping or changing it
        // would orphan the file from every editor and project reference.
        let old_ext = state.target_path.extension().and_then(|e| e.to_str());
        let new_ext = std::path::Path::new(new_name)
            .extension()
            .and_then(|e| e.to_str());
        if old_ext != new_ext {
            let msg = match old_ext {
                Some(ext) => format!("The file extension must stay \".{ext}\"."),
                None => "This file has no extension; the new name cannot add one.".to_string(),
            };
            self.set_rename_error(&msg);
            return iced::Task::none();
        }
        if self.is_project_library_path(&state) {
            self.set_rename_error(
                "Libraries are referenced by path and cannot be renamed from the project tree.",
            );
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

        // Find every parent sheet that references this file BEFORE it
        // moves, while the hierarchy walk can still reach it.
        // Only a schematic sheet can be a child-sheet target; any other
        // file skips the walk so a malformed sheet cannot raise a notice.
        let is_sheet = state.target_path.extension().and_then(|e| e.to_str()) == Some("snxsch");
        let plan = state
            .tree_path
            .first()
            .filter(|_| is_sheet)
            .map(|&idx| self.plan_reference_rewrites(idx, &state.target_path, new_name))
            .unwrap_or_default();

        if let Err(e) = std::fs::rename(&state.target_path, &new_path) {
            self.set_rename_error(&format!("Rename failed: {e}"));
            return iced::Task::none();
        }

        // Update in-memory project data (page list + schematic root);
        // parent-sheet references are re-pointed through the engine
        // below. Nothing besides the rename itself is written to disk:
        // the touched documents are marked dirty for the user to save.
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
            let mut changed = false;
            for entry in loaded.data.sheets.iter_mut() {
                if &entry.filename == old {
                    entry.filename = new_name.to_string();
                    changed = true;
                }
            }
            if loaded.data.pcb_file.as_deref() == Some(old.as_str()) {
                loaded.data.pcb_file = Some(new_name.to_string());
                changed = true;
            }
            if loaded.data.schematic_root.as_deref() == Some(old.as_str()) {
                loaded.data.schematic_root = Some(new_name.to_string());
                changed = true;
            }
            // The sheet list changed in memory; without the dirty bit
            // `save_active_project_if_dirty` never writes the .snxprj and
            // a reopened project points at a file that no longer exists.
            // A rename that touched neither (a footprint, say) leaves the
            // project file as it was and must not dirty it.
            if changed {
                self.document_state.dirty_paths.insert(loaded.path.clone());
            }
        }

        // Update any open tabs + engine-map keys that pointed at the
        // old path. Do this before `refresh_panel_ctx` so the tree
        // rebuild sees the new filenames.
        self.rekey_open_document(&state.target_path, &new_path);

        let outcome = self.apply_reference_rewrites(&plan);
        if !outcome.is_clean() {
            self.document_state.error_notice = Some(crate::app::state::ErrorNotice::rename(
                super::sheet_references::rename_incomplete_detail(
                    &outcome.failed,
                    &outcome.unchecked,
                ),
            ));
        }

        self.ui_state.rename_dialog = None;
        self.refresh_panel_ctx();
        iced::Task::none()
    }

    /// True when the dialog target is one of its owning project's
    /// `libraries` entries (matched relative to the project dir or absolute).
    fn is_project_library_path(&self, state: &crate::app::RenameDialogState) -> bool {
        let Some(loaded) = state
            .tree_path
            .first()
            .and_then(|&idx| self.document_state.projects.get(idx))
        else {
            return false;
        };
        let dir = std::path::Path::new(&loaded.data.dir);
        loaded
            .data
            .libraries
            .iter()
            .any(|e| dir.join(&e.path) == state.target_path || e.path == state.target_path)
    }

    /// Move every path-keyed record of an open document from `old_path`
    /// to `new_path` after its file was renamed on disk: tab, engine map
    /// and the engine's own save target, active path, dirty marker, the
    /// symbol/footprint editor maps and any undocked window. Missing one
    /// leaves a key no engine backs, so closing the tab drops unsaved
    /// edits and Save All fails on the stale path.
    fn rekey_open_document(&mut self, old_path: &std::path::Path, new_path: &std::path::Path) {
        let new_title = new_path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(str::to_string);
        for tab in self.document_state.tabs.iter_mut() {
            if tab.path != old_path {
                continue;
            }
            tab.path = new_path.to_path_buf();
            if let Some(title) = &new_title {
                tab.title = title.clone();
            }
            match &mut tab.kind {
                crate::app::TabKind::SymbolEditor(p) | crate::app::TabKind::FootprintEditor(p) => {
                    *p = new_path.to_path_buf();
                }
                _ => {}
            }
        }
        if let Some(mut engine) = self.document_state.engines.remove(old_path) {
            // `Engine::save` writes to its own path; leaving the old one
            // would write the renamed sheet back under its old filename.
            engine.set_path(Some(new_path.to_path_buf()));
            self.document_state
                .engines
                .insert(new_path.to_path_buf(), engine);
        }
        if let Some(mut editor) = self.document_state.symbol_editors.remove(old_path) {
            editor.path = new_path.to_path_buf();
            self.document_state
                .symbol_editors
                .insert(new_path.to_path_buf(), editor);
        }
        if let Some(mut editor) = self.document_state.footprint_editors.remove(old_path) {
            editor.path = new_path.to_path_buf();
            self.document_state
                .footprint_editors
                .insert(new_path.to_path_buf(), editor);
        }
        if self.document_state.active_path.as_deref() == Some(old_path) {
            self.document_state.active_path = Some(new_path.to_path_buf());
        }
        if self.document_state.dirty_paths.remove(old_path) {
            self.document_state
                .dirty_paths
                .insert(new_path.to_path_buf());
        }
        self.rekey_undocked_windows(old_path, new_path);
    }

    /// Point undocked-tab windows at the renamed path so the window keeps
    /// rendering its sheet instead of going blank.
    pub(crate) fn rekey_undocked_windows(
        &mut self,
        old_path: &std::path::Path,
        new_path: &std::path::Path,
    ) {
        for kind in self.ui_state.windows.values_mut() {
            if let crate::app::state::WindowKind::UndockedTab { path, .. } = kind
                && path == old_path
            {
                *path = new_path.to_path_buf();
            }
        }
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
