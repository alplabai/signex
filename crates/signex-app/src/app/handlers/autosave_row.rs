//! Autosave and Restore for a Component Preview row (#2141).
//!
//! A recovery copy holds the JSON of the row's working copy, keyed by the
//! row's synthetic tab path (`EditorAddress::synthetic_tab_path`), so the
//! shared sidecar writer, cleanup and recovery prompt apply unchanged. A row
//! lives in a library, not in a file, so the open row is the baseline a copy
//! is judged against ([`Signex::row_baseline_bytes`]). Half-typed parameter
//! text and PDFs staged for a pinned datasheet are not in the snapshot.

use std::path::{Path, PathBuf};

use signex_library::ComponentRow;

use crate::app::Signex;
use crate::library::state::{ComponentPreviewState, EditorAddress};

/// The bytes of `row` as a recovery copy holds them.
pub(crate) fn row_snapshot_bytes(row: &ComponentRow) -> Result<Vec<u8>, String> {
    serde_json::to_vec_pretty(row).map_err(|e| e.to_string())
}

impl Signex {
    /// The open Component Preview whose tab path is `path`.
    pub(super) fn row_editor_at(&self, path: &Path) -> Option<&ComponentPreviewState> {
        if !EditorAddress::is_synthetic_tab_path(path) {
            return None;
        }
        self.library
            .editors
            .iter()
            .find(|(address, _)| address.synthetic_tab_path() == path)
            .map(|(_, editor)| editor)
    }

    /// Does the row editor at `path` hold edits that are not in the library?
    /// A save in flight still counts: if it fails the row stays dirty.
    pub(super) fn is_dirty_row(&self, path: &Path) -> bool {
        self.row_editor_at(path)
            .is_some_and(|editor| editor.dirty || editor.saving_row.is_some())
    }

    pub(super) fn dirty_row_paths(&self) -> Vec<PathBuf> {
        self.library
            .editors
            .iter()
            .filter(|(_, editor)| editor.dirty || editor.saving_row.is_some())
            .map(|(address, _)| address.synthetic_tab_path())
            .collect()
    }

    /// The snapshot of the row editor at `path`. `None` when none is open.
    pub(super) fn row_autosave_bytes(&self, path: &Path) -> Option<Result<Vec<u8>, String>> {
        Some(row_snapshot_bytes(&self.row_editor_at(path)?.row))
    }

    /// The snapshot a clean row editor at `path` has, to judge an earlier
    /// run's copy against. `None` when no editor is open or it already holds
    /// edits (the copy is then left alone).
    pub(super) fn row_baseline_bytes(&self, path: &Path) -> Option<Result<Vec<u8>, String>> {
        let editor = self.row_editor_at(path)?;
        (!editor.dirty && editor.saving_row.is_none()).then(|| row_snapshot_bytes(&editor.row))
    }

    /// Restore a row copy into the open editor as unsaved edits, as one undo
    /// step. The copy keeps its own versions and base revision: if the stored
    /// row moved since the copy was written, the next save is refused as
    /// stale (or rebased when only this session's cascade moved it) instead
    /// of writing over the newer row.
    pub(super) fn restore_row_copy(&mut self, path: &Path, row: ComponentRow) -> bool {
        let Some(address) = self
            .library
            .editors
            .keys()
            .find(|address| address.synthetic_tab_path() == path)
            .cloned()
        else {
            return false;
        };
        let Some(editor) = self.library.editors.get_mut(&address) else {
            return false;
        };
        let scope = editor.begin_edit();
        editor.row = row;
        editor.mark_dirty();
        editor.finish_edit(scope);
        true
    }
}
