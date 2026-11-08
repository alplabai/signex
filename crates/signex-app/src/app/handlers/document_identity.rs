//! The one re-key gate (#661): when a document's path changes, every record
//! keyed by the old path moves to the new one here, so no record is orphaned.
//! Rename goes through [`Signex::rekey_document`]; Save As has its own flow
//! steps. Net colours and net-class pick markers live on the sheet's engine,
//! so they need no re-key.

use crate::app::Signex;

impl Signex {
    /// Move every path-keyed record of an open document from `old_path`
    /// to `new_path` after its file was renamed on disk: tab, engine map
    /// and the engine's own save target, active path, dirty marker, the
    /// symbol/footprint editor maps and any undocked window. Missing one
    /// leaves a key no engine backs, so closing the tab drops unsaved
    /// edits and Save All fails on the stale path.
    pub(crate) fn rekey_document(
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
        // Pending drafts belong to the sheet, not to the name it had.
        self.document_state.drafts.rekey(old_path, new_path);
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
        self.document_state.rekey_document_dirty(old_path, new_path);
        // A queued save names the file by its old path; a recorded failure
        // follows the document so its Save Failed card is not lost.
        let failure = self
            .document_state
            .document_saves
            .failure(old_path)
            .map(str::to_owned);
        self.document_state
            .document_saves
            .forget_where(|p| p == old_path);
        if let Some(reason) = failure {
            self.document_state
                .document_saves
                .record_failure(new_path.to_path_buf(), reason);
        }
        self.document_state.document_saves.rekey_disk_records(
            old_path,
            new_path,
            std::fs::canonicalize(new_path).ok(),
        );
        // A PCB's save revision is keyed by path (#1040); without the move
        // every later save would compare against a missing key.
        if let Some(revision) = self.document_state.pcb_revisions.remove(old_path) {
            self.document_state
                .pcb_revisions
                .insert(new_path.to_path_buf(), revision);
        }
        self.document_state
            .rekey_parked_pcb_board(old_path, new_path);
        self.document_state.autosave.rekey(old_path, new_path);
        self.rekey_page_setup(old_path, new_path);
        // Renaming does not rewrite the parents' sheet symbols, so the
        // placement no longer reaches this file.
        self.document_state.tab_placements.forget(old_path);
        self.rekey_undocked_windows(old_path, new_path);
    }
}
