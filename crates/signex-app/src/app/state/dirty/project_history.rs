//! The `.snxprj` saved cursor (#1854).
//!
//! The Project tab's General settings, the stored ERC rules and the net-class
//! table each keep their own undo history, so no single stack position names
//! "the saved state". The one cursor is the bytes the file holds: the project
//! as `encode_project` writes it at the last save, or at the last moment it
//! was known clean. After any history step the project reads clean exactly
//! when it encodes to those bytes and nothing outside the histories marked it.
//! That covers every way a history can lose its position (eviction, a
//! coalesced typing run, a cut redo branch) with nothing to forget by hand.
//!
//! Unknown is the default: with no cursor the project stays dirty until the
//! next save. The cursor is dropped when no known state equals the file (a
//! failed write, a discard, a re-key). Edits outside the histories (a
//! library add, a rename, a sheet list change) go through
//! `mark_document_dirty` and sit in the `external` set, so only a save
//! clears them.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use signex_types::project::encode_project;

use super::super::{DocumentState, ProjectId};

/// The cursors, keyed by project file.
pub(super) type ProjectSavedBytes = HashMap<PathBuf, Vec<u8>>;

impl DocumentState {
    /// Call before a history-recorded edit to project `id`: if the project is
    /// clean right now, its current encoding is the file's content, so that
    /// is the cursor to come back to. A project already dirty keeps whatever
    /// cursor it has (none means it stays dirty until saved).
    pub(crate) fn note_project_clean_state(&mut self, id: ProjectId) {
        let Some(project) = self.project_by_id(id) else {
            return;
        };
        let path = project.path.clone();
        if self.is_document_dirty(&path) {
            return;
        }
        self.record_project_saved_state(&path);
    }

    /// A history step (edit, undo, redo) changed project `id`: it is dirty
    /// unless it encodes to the cursor, nothing outside the histories marked
    /// it, and no write of it is in flight (the completion settles that).
    /// `affects_erc` is false for a display-only change (a net class).
    pub(crate) fn sync_project_history_dirty(&mut self, id: ProjectId, affects_erc: bool) {
        let Some(project) = self.project_by_id(id) else {
            return;
        };
        let path = project.path.clone();
        let at_cursor = self
            .dirty_docs
            .project_saved
            .get(&path)
            .is_some_and(|saved| encode_project(&project.data).is_ok_and(|now| now == *saved));
        let clean = at_cursor
            && !self.dirty_docs.external.contains(&path)
            && !self.document_saves.is_saving(&path);
        if clean {
            self.dirty_docs.paths.remove(&path);
        } else {
            self.dirty_docs.paths.insert(path);
        }
        // The epoch tracks the content changing, not the dirty mark: a step
        // that lands back on the saved rules still outdates ERC results.
        if affects_erc {
            self.note_sheet_edit();
        }
    }

    /// A write of `bytes` to the project file landed but did not leave the
    /// project clean (edited meanwhile): the file holds `bytes`, so an undo
    /// back to them is clean. `newest` is false when a later write is still
    /// to land, which makes the file's content unknown until it does. Callers
    /// that pass a literal `true` (path ops) rely on the path-op guards
    /// keeping them from overlapping a project save.
    pub(crate) fn note_project_bytes_on_disk(&mut self, path: &Path, bytes: Vec<u8>, newest: bool) {
        if newest && self.projects.iter().any(|p| p.path == path) {
            self.dirty_docs
                .project_saved
                .insert(path.to_path_buf(), bytes);
        } else {
            self.dirty_docs.project_saved.remove(path);
        }
    }

    /// The file's content is no longer known to equal any state the project
    /// can return to (a failed write): stay dirty until a save.
    pub(crate) fn forget_project_saved_state(&mut self, path: &Path) {
        self.dirty_docs.project_saved.remove(path);
    }

    /// A save landed for the project at `path` and the project encodes to
    /// what was written: that is the cursor.
    pub(super) fn record_project_saved_state(&mut self, path: &Path) {
        let Some(project) = self.projects.iter().find(|p| p.path == path) else {
            return;
        };
        match encode_project(&project.data) {
            Ok(bytes) => {
                self.dirty_docs
                    .project_saved
                    .insert(path.to_path_buf(), bytes);
            }
            Err(_) => {
                self.dirty_docs.project_saved.remove(path);
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn has_project_saved_state(&self, path: &Path) -> bool {
        self.dirty_docs.project_saved.contains_key(path)
    }
}
