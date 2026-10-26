//! Autosave and Restore for the symbol and footprint editors (#1185).
//!
//! A sidecar holds the bytes a normal save of the container would write
//! (`SymbolFile` / `FootprintFile` TOML), so the shared sidecar writer,
//! cleanup and recovery prompt in `autosave.rs` / `autosave_recovery.rs`
//! apply unchanged. These editors have no revision counter, so a draft is
//! stamped with a hash of its serialised bytes ([`DocStamp::of_bytes`]):
//! an unchanged draft is not rewritten.
//!
//! A footprint copy can be one drag behind the editor: the solve a drag
//! still owes is settled only when the editor is saved
//! (`settle_pending_solve`), not when a copy is taken.

use std::path::{Path, PathBuf};

use iced::Task;

use super::autosave::write_task;
use crate::app::{FootprintEditorState, Message, Signex, SymbolEditorState};
use crate::autosave::DocStamp;

impl Signex {
    /// Does an open symbol or footprint editor at `path` hold edits that
    /// are not on disk? A save clears `dirty` when it starts, not when it
    /// lands, so a write still in flight counts: if it fails the draft is
    /// dirty again and its recovery copy must still be there.
    pub(super) fn is_dirty_primitive_editor(&self, path: &Path) -> bool {
        let document = &self.document_state;
        let open_dirty = document
            .symbol_editors
            .get(path)
            .map(|editor| editor.dirty)
            .or_else(|| {
                document
                    .footprint_editors
                    .get(path)
                    .map(|editor| editor.dirty)
            });
        open_dirty.is_some_and(|dirty| dirty || document.document_saves.is_saving(path))
    }

    pub(super) fn dirty_primitive_editor_paths(&self) -> Vec<PathBuf> {
        let document = &self.document_state;
        let symbols = document
            .symbol_editors
            .iter()
            .filter(|(_, editor)| editor.dirty)
            .map(|(path, _)| path);
        let footprints = document
            .footprint_editors
            .iter()
            .filter(|(_, editor)| editor.dirty)
            .map(|(path, _)| path);
        symbols.chain(footprints).cloned().collect()
    }

    /// The bytes a save of the editor at `path` would write, taken without
    /// touching the editor (a save stamps timestamps and clears `dirty`;
    /// a recovery copy must do neither). `None` when no editor backs `path`.
    fn primitive_autosave_bytes(&self, path: &Path) -> Option<Result<Vec<u8>, String>> {
        if let Some(editor) = self.document_state.symbol_editors.get(path) {
            return Some(
                editor
                    .file
                    .to_toml_string()
                    .map(String::into_bytes)
                    .map_err(|e| e.to_string()),
            );
        }
        let editor = self.document_state.footprint_editors.get(path)?;
        // `state.pads` is authoritative on the editor side; sync it into a
        // copy, as a save does into the editor itself.
        let mut file = editor.file.clone();
        if let Some(footprint) = file.footprints.get_mut(editor.active_idx) {
            crate::library::editor::footprint::state::FootprintEditorState::sync_pads_to_primitive(
                &editor.state,
                footprint,
            );
        }
        Some(
            file.to_toml_string()
                .map(String::into_bytes)
                .map_err(|e| e.to_string()),
        )
    }

    pub(super) fn autosave_primitive_snapshot_task(
        &mut self,
        dir: &Path,
        path: &Path,
    ) -> Option<Task<Message>> {
        let bytes = match self.primitive_autosave_bytes(path)? {
            Ok(bytes) => bytes,
            Err(error) => {
                let reason = format!("could not encode {} for autosave: {error}", path.display());
                self.report_autosave_failure(&reason);
                return None;
            }
        };
        let stamp = DocStamp::of_bytes(&bytes);
        if !self.document_state.autosave.needs_write(path, stamp) {
            return None;
        }
        let superseded = self.document_state.autosave.begin(path);
        let guard = self.document_state.autosave.track_job();
        Some(write_task(
            dir.to_path_buf(),
            path.to_path_buf(),
            stamp,
            bytes,
            superseded,
            guard,
        ))
    }

    /// Restore a symbol copy into the open editor as unsaved edits. The
    /// baseline stays the file on disk, so a save still writes exactly the
    /// symbols the copy changed.
    pub(super) fn restore_symbol_copy(
        &mut self,
        path: &Path,
        file: signex_library::SymbolFile,
    ) -> bool {
        let Some(old) = self.document_state.symbol_editors.get(path) else {
            return false;
        };
        let (baseline, generation, zoom_step) =
            (old.baseline.clone(), old.save_generation, old.zoom_step);
        let mut editor = SymbolEditorState::new(path.to_path_buf(), file);
        editor.baseline = baseline;
        editor.save_generation = generation;
        editor.zoom_step = zoom_step;
        editor.dirty = true;
        self.document_state
            .symbol_editors
            .insert(path.to_path_buf(), editor);
        true
    }

    /// Footprint counterpart of [`Self::restore_symbol_copy`].
    pub(super) fn restore_footprint_copy(
        &mut self,
        path: &Path,
        file: signex_library::FootprintFile,
    ) -> bool {
        let Some(old) = self.document_state.footprint_editors.get(path) else {
            return false;
        };
        let (baseline, generation, zoom_step) =
            (old.baseline.clone(), old.save_generation, old.zoom_step);
        let mut editor = FootprintEditorState::reloaded(path.to_path_buf(), file)
            .with_global_snap_disabled(!self.ui_state.snap_enabled);
        editor.baseline = baseline;
        editor.save_generation = generation;
        editor.zoom_step = zoom_step;
        editor.dirty = true;
        self.document_state
            .footprint_editors
            .insert(path.to_path_buf(), editor);
        true
    }
}
