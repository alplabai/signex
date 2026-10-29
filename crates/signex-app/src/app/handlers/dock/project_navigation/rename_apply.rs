//! Completion of a file or project rename (#859): the worker moved the file
//! (or failed), and this re-keys everything that knew the old path in one
//! `update`, so no state ever points at a path that does not exist.
//!
//! A project closed, a tab closed or a document edited while the worker ran
//! is handled by keying on what is open *now*: tabs, engines and dirty markers
//! are moved by path, so edits travel with the document; the project record
//! is found by id, and a project that is gone is reported instead of guessed.

use super::*;
use crate::app::path_ops::{
    FileRenameDone, FileRenameKind, FileRenameRequest, ParentSheets, ReferencePlan, RenameIoError,
};
use crate::app::state::library_entry::{entry_resolves_to, with_new_leaf};

/// Point the project's page list, PCB and schematic root at `new_name`.
/// Entries match by resolved path and keep their directory prefix
/// (`sub/Power.snxsch` becomes `sub/Power2.snxsch`, #1367). Returns whether
/// anything changed.
fn repoint_project_entries(
    loaded: &mut crate::app::state::LoadedProject,
    from: &std::path::Path,
    new_name: &str,
) -> bool {
    let fold = loaded.case_fold;
    let dir = loaded.dir().to_path_buf();
    let renamed_from = |entry: &str| {
        entry_resolves_to(&dir, entry, from, fold).then(|| with_new_leaf(entry, new_name))
    };
    let mut changed = false;
    for entry in loaded.data.sheets.iter_mut() {
        if let Some(new_entry) = renamed_from(&entry.filename) {
            entry.filename = new_entry;
            changed = true;
        }
    }
    if let Some(new_entry) = loaded.data.pcb_file.as_deref().and_then(renamed_from) {
        loaded.data.pcb_file = Some(new_entry);
        changed = true;
    }
    if let Some(new_entry) = loaded.data.schematic_root.as_deref().and_then(renamed_from) {
        loaded.data.schematic_root = Some(new_entry);
        changed = true;
    }
    changed
}

impl Signex {
    pub(crate) fn handle_file_renamed(&mut self, done: FileRenameDone) -> iced::Task<Message> {
        let FileRenameDone {
            request,
            result,
            parents,
        } = done;
        self.document_state.pending_path_ops.finish(request.id);
        if let Err(error) = result {
            self.report_file_rename_failure(&request, &error);
            return iced::Task::none();
        }
        match &request.kind {
            FileRenameKind::Document(plan) => self.apply_document_rename(&request, plan, parents),
        }
        if self.dialog_targets(&request.from) {
            self.ui_state.rename_dialog = None;
        }
        self.refresh_panel_ctx();
        self.autosave_flush_moves()
    }

    /// Does the open rename dialog still name `path`? It may have been closed
    /// or reopened for another file while the worker ran.
    fn dialog_targets(&self, path: &std::path::Path) -> bool {
        self.ui_state
            .rename_dialog
            .as_ref()
            .is_some_and(|d| d.target_path == path)
    }

    /// The move did not happen: nothing in memory changed. The dialog shows
    /// the reason when it is still open for this file; the Messages panel
    /// always gets it, reason first.
    fn report_file_rename_failure(&mut self, request: &FileRenameRequest, error: &RenameIoError) {
        let (shown, reason) = match (error, &request.kind) {
            (RenameIoError::TargetExists, FileRenameKind::Document(_)) => (
                "A file with that name already exists.".to_string(),
                "A file with that name already exists".to_string(),
            ),
            (RenameIoError::Os(e), _) => {
                (format!("Rename failed: {e}"), format!("Rename failed: {e}"))
            }
        };
        let name = request
            .from
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        crate::diagnostics::log_error("Rename failed", &anyhow::anyhow!("{reason} ({name})"));
        if self.dialog_targets(&request.from) {
            self.set_rename_error(&shown);
        } else {
            self.document_state.error_notice =
                Some(crate::app::state::ErrorNotice::rename_failed(shown));
        }
    }

    fn owner_index(&self, request: &FileRenameRequest) -> Option<usize> {
        let id = request.owner?;
        self.document_state.projects.iter().position(|p| p.id == id)
    }

    /// The file moved but its project was closed meanwhile, so the project
    /// record and the parent-sheet references cannot be updated.
    fn warn_project_closed_during_rename(&self, request: &FileRenameRequest) {
        let name = |p: &std::path::Path| {
            p.file_name()
                .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
        };
        crate::diagnostics::log_warning(format!(
            "Its project was closed while {} was renamed to {}; open documents follow the \
             new name, but sheets that reference it were not updated.",
            name(&request.from),
            name(&request.to)
        ));
    }

    fn apply_document_rename(
        &mut self,
        request: &FileRenameRequest,
        plan: &ReferencePlan,
        parents: ParentSheets,
    ) {
        let new_name = request.new_name.as_str();
        let owner_idx = self.owner_index(request);
        if request.owner.is_some() && owner_idx.is_none() {
            self.warn_project_closed_during_rename(request);
        }
        // Update in-memory project data (page list + schematic root);
        // parent-sheet references are re-pointed through the engine
        // below. Nothing besides the rename itself is written to disk:
        // the touched documents are marked dirty for the user to save.
        // Mutate the *owning* project's data, not the active project:
        // renaming a sheet of project B while project A is active should
        // still touch B. (#54)
        if let Some(idx) = owner_idx
            && let Some(loaded) = self.document_state.projects.get_mut(idx)
            && repoint_project_entries(loaded, &request.from, new_name)
        {
            // The sheet list changed in memory; without the dirty bit
            // `save_active_project_if_dirty` never writes the .snxprj and
            // a reopened project points at a file that no longer exists.
            // A rename that touched neither (a footprint, say) leaves the
            // project file as it was and must not dirty it.
            self.document_state.dirty_paths.insert(loaded.path.clone());
        }

        // Update any open tabs + engine-map keys that pointed at the
        // old path. Do this before `refresh_panel_ctx` so the tree
        // rebuild sees the new filenames.
        self.rekey_open_document(&request.from, &request.to);

        if owner_idx.is_none() {
            return;
        }
        let outcome = self.apply_reference_rewrites(plan, parents);
        if !outcome.is_clean() {
            self.document_state.error_notice = Some(crate::app::state::ErrorNotice::rename(
                super::sheet_references::rename_incomplete_detail(
                    &outcome.failed,
                    &outcome.unchecked,
                ),
            ));
        }
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
                .insert_engine(new_path.to_path_buf(), engine);
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
        if let Some(count) = self.ui_state.net_color_revision.remove(old_path) {
            self.ui_state
                .net_color_revision
                .insert(new_path.to_path_buf(), count);
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
}
