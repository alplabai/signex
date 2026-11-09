//! Where a PCB's board lives: in its tab, or parked after a dirty tab closed
//! (#2149).
//!
//! Closing a dirty PCB tab parks its board engine here, like a dirty
//! schematic engine, so quit, Close Project and Save All still find and write
//! it, and reopening the file restores the unsaved edits and their undo
//! history. A parked engine is held only while its dirty mark is:
//! [`DocumentState::clear_document_dirty`] drops it, so a save that lands or
//! a discard never leaves a stale one.

use std::path::{Path, PathBuf};

use super::DocumentState;
use crate::app::{TabDocument, TabKind};
use signex_engine::pcb::BoardEngine;
use signex_types::pcb::PcbBoard;

/// The engines of closed dirty PCB tabs, by path.
pub(crate) type ParkedBoards = std::collections::HashMap<PathBuf, BoardEngine>;

impl DocumentState {
    /// Does a PCB tab, or a board parked without one, hold `path`?
    pub(crate) fn holds_pcb(&self, path: &Path) -> bool {
        self.has_pcb_tab(path) || self.parked_pcb_boards.contains_key(path)
    }

    /// The board of the PCB at `path`: its tab's, else the parked one.
    /// `None` for a tab whose board a failed History restore dropped.
    pub(crate) fn pcb_board(&self, path: &Path) -> Option<&PcbBoard> {
        self.board_engine(path).map(BoardEngine::board)
    }

    /// The board engine of the PCB at `path`: its tab's, else the parked one.
    pub(crate) fn board_engine(&self, path: &Path) -> Option<&BoardEngine> {
        if self.has_pcb_tab(path) {
            return self
                .tabs
                .iter()
                .find(|tab| matches!(tab.kind, TabKind::Pcb) && tab.path == path)
                .and_then(|tab| tab.cached_document.as_ref())
                .and_then(TabDocument::as_pcb_engine);
        }
        self.parked_pcb_boards.get(path)
    }

    /// The engine of the PCB tab at `path`, for a write. A parked engine is
    /// read-only: nothing may step it while its save can be in flight, or the
    /// landing save would clean a board that moved after the snapshot.
    pub(crate) fn board_engine_mut(&mut self, path: &Path) -> Option<&mut BoardEngine> {
        self.tabs
            .iter_mut()
            .find(|tab| matches!(tab.kind, TabKind::Pcb) && tab.path == path)
            .and_then(|tab| tab.cached_document.as_mut())
            .and_then(TabDocument::as_pcb_engine_mut)
    }

    /// A landed save matches the open board at `path`: its current state is
    /// the save point. Bookkeeping only, never an edit. (A parked board is
    /// dropped by `clear_document_dirty` before this runs.)
    pub(crate) fn mark_board_saved(&mut self, path: &Path) {
        if let Some(engine) = self.board_bookkeeping_mut(path) {
            engine.mark_saved();
        }
    }

    /// The file at `path` holds a snapshot no step of the board reproduces,
    /// open or parked: it reads unsaved until the next save.
    pub(crate) fn forget_board_save_point(&mut self, path: &Path) {
        if let Some(engine) = self.board_bookkeeping_mut(path) {
            engine.forget_save_point();
        }
    }

    /// The engine at `path`, tab or parked, for the save-point calls above.
    /// Kept private so an edit cannot reach a parked engine through it.
    fn board_bookkeeping_mut(&mut self, path: &Path) -> Option<&mut BoardEngine> {
        if self.has_pcb_tab(path) {
            return self.board_engine_mut(path);
        }
        self.parked_pcb_boards.get_mut(path)
    }

    /// Every board engine a tab or a parked board holds.
    pub(crate) fn board_engines_mut(&mut self) -> impl Iterator<Item = &mut BoardEngine> {
        let open = self
            .tabs
            .iter_mut()
            .filter_map(|tab| tab.cached_document.as_mut())
            .filter_map(TabDocument::as_pcb_engine_mut);
        open.chain(self.parked_pcb_boards.values_mut())
    }

    /// Every PCB path a tab or a parked board holds, one entry per path.
    pub(crate) fn held_pcb_paths(&self) -> Vec<PathBuf> {
        let mut paths: Vec<PathBuf> = self
            .tabs
            .iter()
            .filter(|tab| matches!(tab.kind, TabKind::Pcb))
            .map(|tab| tab.path.clone())
            .chain(self.parked_pcb_boards.keys().cloned())
            .collect();
        paths.sort();
        paths.dedup();
        paths
    }

    /// Keep the engine of a dirty PCB tab that is closing.
    pub(crate) fn park_pcb_board(&mut self, path: PathBuf, engine: BoardEngine) {
        self.note_documents_changed();
        self.parked_pcb_boards.insert(path, engine);
    }

    /// Take the parked engine of `path` back into a reopened tab.
    pub(crate) fn take_parked_pcb_board(&mut self, path: &Path) -> Option<BoardEngine> {
        self.parked_pcb_boards.remove(path)
    }

    /// Move a parked board to the path its file was renamed to.
    pub(crate) fn rekey_parked_pcb_board(&mut self, from: &Path, to: &Path) {
        if let Some(board) = self.parked_pcb_boards.remove(from) {
            self.parked_pcb_boards.insert(to.to_path_buf(), board);
        }
    }

    pub(crate) fn has_pcb_tab(&self, path: &Path) -> bool {
        self.tabs
            .iter()
            .any(|tab| matches!(tab.kind, TabKind::Pcb) && tab.path == path)
    }
}
