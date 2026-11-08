//! Autosave and Restore for PCB tabs (#1185).
//!
//! A sidecar holds the bytes a normal save of the board would write
//! (`SnxPcb::write_string`), so the shared sidecar writer, cleanup and
//! recovery prompt in `autosave.rs` / `autosave_recovery.rs` apply
//! unchanged. A draft is stamped with a hash of its serialised bytes
//! ([`DocStamp::of_bytes`]): an unchanged board is not rewritten.

use std::path::{Path, PathBuf};

use iced::Task;

use super::autosave::write_task;
use crate::app::{Message, Signex, TabKind};
use crate::autosave::DocStamp;
use signex_types::pcb::PcbBoard;

impl Signex {
    /// Is a PCB open on `path`: a tab, or a board parked when its dirty tab
    /// closed (#2149)?
    pub(super) fn is_open_pcb(&self, path: &Path) -> bool {
        self.document_state.holds_pcb(path)
    }

    pub(super) fn open_pcb_paths(&self) -> Vec<PathBuf> {
        self.document_state.held_pcb_paths()
    }

    /// Does the open PCB at `path` hold edits that are not on disk? A save
    /// clears the flags when it lands, so a write in flight still counts.
    pub(super) fn is_dirty_pcb(&self, path: &Path) -> bool {
        let document = &self.document_state;
        document.holds_pcb(path)
            && (document.is_document_dirty(path) || document.document_saves.is_saving(path))
    }

    pub(super) fn dirty_pcb_paths(&self) -> Vec<PathBuf> {
        self.open_pcb_paths()
            .into_iter()
            .filter(|path| self.is_dirty_pcb(path))
            .collect()
    }

    /// The bytes a save of the PCB at `path` would write. `None` when it
    /// holds no board to protect: a board dropped by a failed History
    /// restore is stale, and one opened read-only (newer format) is never
    /// saved, so neither gets a copy.
    fn pcb_autosave_bytes(&self, path: &Path) -> Option<Result<Vec<u8>, String>> {
        let board = self.document_state.pcb_board(path)?;
        if board.unknown.newer_minor_source.is_some() {
            return None;
        }
        Some(
            signex_types::format::SnxPcb::new(board.clone())
                .write_string()
                .map(String::into_bytes)
                .map_err(|e| format!("{e:#}")),
        )
    }

    pub(super) fn autosave_pcb_snapshot_task(
        &mut self,
        dir: &Path,
        path: &Path,
    ) -> Option<Task<Message>> {
        let bytes = match self.pcb_autosave_bytes(path)? {
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

    /// Restore a PCB copy into the open tab as unsaved edits. Nothing is
    /// written to the real file; the tab is dirty so a save writes the copy.
    pub(super) fn restore_pcb_copy(&mut self, path: &Path, board: PcbBoard) -> bool {
        let Some(idx) = self
            .document_state
            .tabs
            .iter()
            .position(|tab| matches!(tab.kind, TabKind::Pcb) && tab.path == path)
        else {
            return false;
        };
        self.document_state.set_pcb_board(idx, board);
        self.document_state.mark_document_dirty(path);
        // Keep the camera: the user is looking at this board already.
        self.apply_loaded_pcb_document(false, false);
        true
    }
}
