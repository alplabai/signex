//! Board revisions that tell a PCB save completion whether its bytes are
//! still the board (#1040).
//!
//! The counter is global and never reused: a tab closed mid-write and the same
//! file reopened gets a fresh stamp, so the old write cannot mark the new tab
//! clean. The map is keyed by path and survives a tab close for that reason.

use std::path::Path;

use super::DocumentState;
use crate::app::TabDocument;
use signex_types::pcb::PcbBoard;

impl DocumentState {
    /// Stamp `path` with a fresh, never-reused revision (the first is 1).
    pub(crate) fn stamp_pcb_revision(&mut self, path: &Path) -> u64 {
        self.next_pcb_revision += 1;
        self.pcb_revisions
            .insert(path.to_path_buf(), self.next_pcb_revision);
        self.next_pcb_revision
    }

    /// The one write path for a tab's cached board after it opened.
    pub(crate) fn set_pcb_board(&mut self, tab_idx: usize, board: PcbBoard) {
        self.replace_pcb_document(tab_idx, Some(TabDocument::Pcb(board)));
    }

    /// Drop a tab's cached board (a failed History restore); that is a change
    /// a save in flight must not mark clean either.
    pub(crate) fn drop_pcb_board(&mut self, tab_idx: usize) {
        self.replace_pcb_document(tab_idx, None);
    }

    fn replace_pcb_document(&mut self, tab_idx: usize, document: Option<TabDocument>) {
        let Some(tab) = self.tabs.get_mut(tab_idx) else {
            return;
        };
        tab.cached_document = document;
        let path = tab.path.clone();
        self.stamp_pcb_revision(&path);
    }
}
