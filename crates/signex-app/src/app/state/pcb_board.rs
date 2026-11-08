//! Where a PCB's board lives: in its tab, or parked after a dirty tab closed
//! (#2149).
//!
//! Closing a dirty PCB tab parks its board here, like a dirty schematic
//! engine, so quit, Close Project and Save All still find and write it, and
//! reopening the file restores the unsaved edits. A parked board is held
//! only while its dirty mark is: [`DocumentState::clear_document_dirty`]
//! drops it, so a save that lands or a discard never leaves a stale one.

use std::path::{Path, PathBuf};

use super::DocumentState;
use crate::app::TabKind;
use signex_types::pcb::PcbBoard;

impl DocumentState {
    /// Does a PCB tab, or a board parked without one, hold `path`?
    pub(crate) fn holds_pcb(&self, path: &Path) -> bool {
        self.has_pcb_tab(path) || self.parked_pcb_boards.contains_key(path)
    }

    /// The board of the PCB at `path`: its tab's, else the parked one.
    /// `None` for a tab whose board a failed History restore dropped.
    pub(crate) fn pcb_board(&self, path: &Path) -> Option<&PcbBoard> {
        if self.has_pcb_tab(path) {
            return self
                .tabs
                .iter()
                .find(|tab| matches!(tab.kind, TabKind::Pcb) && tab.path == path)
                .and_then(|tab| tab.cached_document.as_ref())
                .and_then(|document| document.as_pcb());
        }
        self.parked_pcb_boards.get(path)
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

    /// Keep the board of a dirty PCB tab that is closing.
    pub(crate) fn park_pcb_board(&mut self, path: PathBuf, board: PcbBoard) {
        self.note_documents_changed();
        self.parked_pcb_boards.insert(path, board);
    }

    /// Take the parked board of `path` back into a reopened tab.
    pub(crate) fn take_parked_pcb_board(&mut self, path: &Path) -> Option<PcbBoard> {
        self.parked_pcb_boards.remove(path)
    }

    /// Move a parked board to the path its file was renamed to.
    pub(crate) fn rekey_parked_pcb_board(&mut self, from: &Path, to: &Path) {
        if let Some(board) = self.parked_pcb_boards.remove(from) {
            self.parked_pcb_boards.insert(to.to_path_buf(), board);
        }
    }

    fn has_pcb_tab(&self, path: &Path) -> bool {
        self.tabs
            .iter()
            .any(|tab| matches!(tab.kind, TabKind::Pcb) && tab.path == path)
    }
}
