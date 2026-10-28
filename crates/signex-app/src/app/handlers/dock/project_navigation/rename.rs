//! Rename flows for the project-navigation dock — file rename and project rename.

use super::*;

impl Signex {
    pub(super) fn open_rename_dialog(&mut self, tree_path: Vec<usize>) {
        let Some(target_path) = self.tree_path_to_file_path(&tree_path) else {
            crate::diagnostics::log_warning(
                "Rename cancelled: the selected project-tree row no longer resolves to a file.",
            );
            return;
        };
        let filename = target_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        let project_id = self.project_id_at_tree_path(&tree_path);
        self.ui_state.rename_dialog = Some(crate::app::RenameDialogState {
            target_path,
            tree_path,
            project_id,
            buffer: filename,
            error: None,
            is_project_rename: false,
        });
    }

    /// Open the rename modal seeded with the project name (the `.snxprj`
    /// file stem). On submit, [`handle_rename_submit`] sees
    /// `is_project_rename = true` and renames the project (see `project_rename`).
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
            project_id: Some(project.id),
            buffer: stem,
            error: None,
            is_project_rename: true,
        });
    }

    pub(crate) fn handle_rename_submit(&mut self) -> iced::Task<Message> {
        let Some(mut state) = self.ui_state.rename_dialog.clone() else {
            return iced::Task::none();
        };
        if !self.repoint_dialog_tree_path(state.project_id, &mut state.tree_path) {
            crate::diagnostics::log_warning(format!(
                "Rename of {} cancelled: its project was closed while the dialog was open.",
                state.target_path.display()
            ));
            self.set_rename_error("The project was closed while this dialog was open.");
            return iced::Task::none();
        }

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
        // An open still reading this file would land on the old path after
        // the move, and a save of that tab would recreate it (#1096).
        if let Some(opening) = self
            .pending_open_under(&state.target_path)
            .or_else(|| self.recovery_check_under(&state.target_path))
        {
            crate::diagnostics::log_warning(format!(
                "Rename of {} refused: {} is still opening.",
                state.target_path.display(),
                opening.display()
            ));
            self.set_rename_error("This file is still opening; rename it once it has opened.");
            return iced::Task::none();
        }
        // A restore still running would rewrite the old path after the move,
        // recreating the file the user just renamed.
        if let Some(restoring) = self
            .document_state
            .pending_restores
            .find_under(&state.target_path)
        {
            let label = restoring.file_name().map_or_else(
                || restoring.display().to_string(),
                |name| name.to_string_lossy().into_owned(),
            );
            crate::diagnostics::log_warning(format!(
                "Rename refused: a History restore of {label} is still running."
            ));
            self.set_rename_error(
                "A History restore of this file is still running; rename it once the restore has finished.",
            );
            return iced::Task::none();
        }
        // A project library moves its entry, mount and open documents
        // together once the file rename lands (#739).
        if self.project_library_kind(&state).is_some() {
            return self.begin_library_rename(&state, new_path);
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

        // A write still running for this sheet would recreate the old file
        // (or land after the move); let it finish first. Same bounded wait
        // as Save All's `try_save_dirty_path`.
        self.document_state.document_saves.barrier().wait();
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
        self.autosave_flush_moves()
    }

    /// Move every path-keyed record of an open document from `old_path`
    /// to `new_path` after its file was renamed on disk: tab, engine map
    /// and the engine's own save target, active path, dirty marker, the
    /// symbol/footprint editor maps and any undocked window. Missing one
    /// leaves a key no engine backs, so closing the tab drops unsaved
    /// edits and Save All fails on the stale path.
    pub(crate) fn rekey_open_document(
        &mut self,
        old_path: &std::path::Path,
        new_path: &std::path::Path,
    ) {
        let new_title = new_path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(str::to_string);
        for tab in self.document_state.tabs.iter_mut() {
            if tab.path != old_path {
                continue;
            }
            tab.path = new_path.to_path_buf();
            if tab.kind.as_project().is_some() {
                tab.title = crate::app::project_tab::project_tab_title(new_path);
            } else if let Some(title) = &new_title {
                tab.title = title.clone();
            }
            match &mut tab.kind {
                crate::app::TabKind::SymbolEditor(p) | crate::app::TabKind::FootprintEditor(p) => {
                    *p = new_path.to_path_buf();
                }
                _ => {}
            }
        }
        if let Some(mut engine) = self.document_state.remove_engine(old_path) {
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
        // A failure or queued save names the file by its old path.
        self.document_state
            .document_saves
            .forget_where(|p| p == old_path);
        // A PCB's save revision is keyed by path (#1040); without the move
        // every later save would compare against a missing key.
        if let Some(revision) = self.document_state.pcb_revisions.remove(old_path) {
            self.document_state
                .pcb_revisions
                .insert(new_path.to_path_buf(), revision);
        }
        self.document_state.autosave.rekey(old_path, new_path);
        self.rekey_page_setup(old_path, new_path);
        // Renaming does not rewrite the parents' sheet symbols, so the
        // placement no longer reaches this file.
        self.document_state.tab_placements.forget(old_path);
        self.rekey_undocked_windows(old_path, new_path);
    }

    /// Carry the remembered PDF page setup (and an open preview's source)
    /// over to the document's new path.
    pub(crate) fn rekey_page_setup(
        &mut self,
        old_path: &std::path::Path,
        new_path: &std::path::Path,
    ) {
        crate::app::state::rekey_page_setup(
            &mut self.document_state.page_setup_memory,
            self.document_state.preview.as_mut(),
            old_path,
            new_path,
        );
    }

    /// Point undocked-tab windows at the renamed path so the window keeps
    /// rendering its sheet instead of going blank.
    pub(crate) fn rekey_undocked_windows(
        &mut self,
        old_path: &std::path::Path,
        new_path: &std::path::Path,
    ) {
        // A part on the cursor follows its sheet to the new path.
        if let Some(session) = self.interaction_state.placement.as_mut()
            && session.target_sheet == old_path
        {
            session.target_sheet = new_path.to_path_buf();
        }
        for kind in self.ui_state.windows.values_mut() {
            if let crate::app::state::WindowKind::UndockedTab { path, .. } = kind
                && path == old_path
            {
                *path = new_path.to_path_buf();
            }
        }
    }

    /// A file at or under `target` whose recovery check has not reported:
    /// renaming it now would lose the copy the check finds (#1323).
    fn recovery_check_under(&self, target: &std::path::Path) -> Option<std::path::PathBuf> {
        use crate::app::state::scope::path_is_under;
        let ds = &self.document_state;
        let owner = ds
            .projects
            .iter()
            .find(|p| path_is_under(target, p.dir(), p.case_fold));
        let fold = crate::app::state::project_case::fold_of(owner);
        ds.recovery_check_due
            .iter()
            .chain(ds.autosave.checking_paths())
            .find(|p| path_is_under(p, target, fold))
            .cloned()
    }

    pub(super) fn set_rename_error(&mut self, msg: &str) {
        if let Some(d) = self.ui_state.rename_dialog.as_mut() {
            d.error = Some(msg.to_string());
        }
    }

    /// Refuse a rename that would race an off-thread `.snxprj` write: shown
    /// in the dialog and logged to the Messages panel.
    pub(super) fn refuse_rename_while_project_saving(&mut self) {
        const REASON: &str = "The project file is still being saved; try again in a moment.";
        self.set_rename_error(REASON);
        crate::diagnostics::log_warning(format!("[rename] refused: {REASON}"));
    }
}
