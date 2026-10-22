//! Mirror footprint-editor dirtiness into the workspace dirty set.
//!
//! `dirty_paths` is what the quit gate, the project-close gate and the
//! project tree read; the footprint editors only flip their own
//! `FootprintEditorState::dirty`. Instead of every edit site (well over
//! fifty across the dock handlers and the editor's update modules)
//! remembering a second write, `refresh_panel_ctx` calls this one
//! mirror, so a new edit site that refreshes the panel is covered
//! without knowing `dirty_paths` exists (#666).

use super::*;

impl Signex {
    /// Insert every dirty footprint editor's path into `dirty_paths`
    /// and light its tab's dirty dot. One-way: clean-up happens at the
    /// save and discard sites, which clear the editor flag and the
    /// path together.
    pub(crate) fn sync_footprint_dirty_paths(&mut self) {
        let dirty: Vec<std::path::PathBuf> = self
            .document_state
            .footprint_editors
            .iter()
            .filter(|(_, editor)| editor.dirty)
            .map(|(path, _)| path.clone())
            .collect();
        for path in dirty {
            if let Some(tab) = self.document_state.tabs.iter_mut().find(|t| t.path == path) {
                tab.dirty = true;
            }
            self.document_state.dirty_paths.insert(path);
        }
    }
}
