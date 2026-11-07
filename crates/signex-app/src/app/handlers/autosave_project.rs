//! Autosave and Restore for a loaded project (#2141).
//!
//! A recovery copy holds the `.snxprj` bytes a save of the project would
//! write (`encode_project`), so the shared sidecar writer, cleanup and
//! recovery prompt in `autosave.rs` / `autosave_recovery.rs` apply
//! unchanged. A project is the document with no tab of its own: it is
//! offered when it loads and restored without activating anything.
//! Libraries still pending creation are not in the bytes; they are written
//! by a save and are lost with a crash.

use std::path::Path;

use signex_types::project::{ProjectData, encode_project};

use crate::app::Signex;

impl Signex {
    /// Is a project loaded from the `.snxprj` at `path`?
    pub(super) fn is_loaded_project(&self, path: &Path) -> bool {
        self.document_state.projects.iter().any(|p| p.path == path)
    }

    /// Does the project at `path` hold edits that are not on disk? A save
    /// clears the mark when it lands, so a write in flight still counts.
    pub(super) fn is_dirty_project(&self, path: &Path) -> bool {
        let document = &self.document_state;
        self.is_loaded_project(path)
            && (document.is_document_dirty(path) || document.document_saves.is_saving(path))
    }

    /// The bytes a save of the project at `path` would write. `None` when
    /// no loaded project has that path.
    pub(super) fn project_autosave_bytes(&self, path: &Path) -> Option<Result<Vec<u8>, String>> {
        let project = self
            .document_state
            .projects
            .iter()
            .find(|p| p.path == path)?;
        Some(encode_project(&project.data).map_err(|e| e.to_string()))
    }

    /// Restore a project copy as unsaved edits. Nothing is written to the
    /// real file; the project is marked modified so a save writes the copy.
    pub(super) fn restore_project_copy(&mut self, path: &Path, data: ProjectData) -> bool {
        let document = &mut self.document_state;
        let Some(project) = document.projects.iter_mut().find(|p| p.path == path) else {
            return false;
        };
        let id = project.id;
        project.data = data;
        // The undo steps were recorded against the data just replaced.
        document.project_tabs.entry(id).and_modify(|tab| {
            tab.general_history = Default::default();
            tab.default_drafts.clear();
        });
        document.rules_history.remove(&id);
        document.net_class_history.remove(&id);
        document.mark_document_dirty(path);
        self.clear_schematic_content_caches();
        self.refresh_panel_ctx();
        true
    }
}
