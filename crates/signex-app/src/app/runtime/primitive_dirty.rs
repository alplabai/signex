//! Mirror footprint- and symbol-editor dirtiness into the workspace
//! dirty set.
//!
//! `dirty_paths` is what the quit gate, the project-close gate and the
//! project tree read; the footprint and symbol editors only flip their
//! own `dirty` flag. Instead of every edit site (well over fifty across
//! the dock handlers and the editors' update modules) remembering a
//! second write, `refresh_panel_ctx` calls this one mirror, so a new
//! edit site that refreshes the panel is covered without knowing
//! `dirty_paths` exists (#666). Undo after a Save re-dirties an editor
//! the same way, so it is covered too.

use super::*;

impl Signex {
    /// Does the symbol/footprint draft at `path` hold unsaved edits?
    ///
    /// Reads the editors themselves as well as `dirty_paths`: editor edits
    /// only reach it through `sync_editor_dirty_paths`, so `dirty_paths`
    /// alone can lag a real edit (#110).
    pub(crate) fn primitive_draft_is_dirty(&self, path: &std::path::Path) -> bool {
        self.document_state.dirty_paths.contains(path)
            || self
                .document_state
                .symbol_editors
                .get(path)
                .is_some_and(|e| e.dirty)
            || self
                .document_state
                .footprint_editors
                .get(path)
                .is_some_and(|e| e.dirty)
    }

    /// Insert every dirty footprint or symbol editor's path into
    /// `dirty_paths` and light its tab's dirty dot. One-way: clean-up
    /// happens at the save and discard sites, which clear the editor
    /// flag and the path together.
    pub(crate) fn sync_editor_dirty_paths(&mut self) {
        let footprints = self
            .document_state
            .footprint_editors
            .iter()
            .filter(|(_, editor)| editor.dirty)
            .map(|(path, _)| path.clone());
        let symbols = self
            .document_state
            .symbol_editors
            .iter()
            .filter(|(_, editor)| editor.dirty)
            .map(|(path, _)| path.clone());
        let dirty: Vec<std::path::PathBuf> = footprints.chain(symbols).collect();
        for path in dirty {
            if let Some(tab) = self.document_state.tabs.iter_mut().find(|t| t.path == path) {
                tab.dirty = true;
            }
            self.document_state.dirty_paths.insert(path);
        }
    }
}
