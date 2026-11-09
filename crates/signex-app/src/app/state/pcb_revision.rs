//! Board revisions that tell a PCB save completion whether its bytes are
//! still the board (#1040).
//!
//! The counter is global and never reused, and an engine's own edits raise it
//! too (`note_pcb_revision`): a tab closed mid-write and the same
//! file reopened gets a fresh stamp, so the old write cannot mark the new tab
//! clean. The map is keyed by path and survives a tab close for that reason.

use std::path::Path;

use super::DocumentState;
use crate::app::TabDocument;
use signex_engine::pcb::BoardEngine;
use signex_types::pcb::PcbBoard;

impl DocumentState {
    /// Stamp `path` with a fresh, never-reused revision (the first is 1).
    pub(crate) fn stamp_pcb_revision(&mut self, path: &Path) -> u64 {
        self.next_pcb_revision += 1;
        self.pcb_revisions
            .insert(path.to_path_buf(), self.next_pcb_revision);
        self.next_pcb_revision
    }

    /// Note that `path`'s engine reached `revision`: the map keeps it past
    /// a tab close, and no later engine for any path starts at or below it.
    pub(super) fn note_pcb_revision(&mut self, path: &Path, revision: u64) {
        self.pcb_revisions.insert(path.to_path_buf(), revision);
        self.next_pcb_revision = self.next_pcb_revision.max(revision);
    }

    /// A board engine for `path` read from disk (or equal to it), starting
    /// above every revision the path reached before.
    pub(crate) fn new_board_engine(&mut self, path: &Path, board: PcbBoard) -> BoardEngine {
        BoardEngine::new(board, self.stamp_pcb_revision(path))
    }

    /// The one write path for a tab's whole board after it opened: a new
    /// engine with an empty history, equal to the file.
    pub(crate) fn set_pcb_board(&mut self, tab_idx: usize, board: PcbBoard) {
        self.replace_pcb_engine(tab_idx, |revision| Some(BoardEngine::new(board, revision)));
    }

    /// [`Self::set_pcb_board`] for a board that is not known to equal the
    /// file (a recovery copy): dirty until the next save.
    pub(crate) fn set_unsaved_pcb_board(&mut self, tab_idx: usize, board: PcbBoard) {
        self.replace_pcb_engine(tab_idx, |revision| {
            Some(BoardEngine::unsaved(board, revision))
        });
    }

    /// Drop a tab's cached board (a failed History restore); that is a change
    /// a save in flight must not mark clean either.
    pub(crate) fn drop_pcb_board(&mut self, tab_idx: usize) {
        self.replace_pcb_engine(tab_idx, |_| None);
    }

    fn replace_pcb_engine(
        &mut self,
        tab_idx: usize,
        make: impl FnOnce(u64) -> Option<BoardEngine>,
    ) {
        let Some(path) = self.tabs.get(tab_idx).map(|tab| tab.path.clone()) else {
            return;
        };
        let revision = self.stamp_pcb_revision(&path);
        self.tabs[tab_idx].cached_document = make(revision).map(TabDocument::Pcb);
    }
}
