//! A dirty PCB tab that closes parks its board engine, history included;
//! reopening the file restores it (#2149). The board store is `state/pcb_board.rs`.

use std::path::PathBuf;

use crate::app::{Signex, TabDocument, TabInfo, TabKind};

impl Signex {
    /// The PCB tab at `idx` is closing. A close the user asked for keeps a
    /// dirty board parked, so quit, Close Project and Save All still see it;
    /// a discard, a clean board or one a failed History restore dropped
    /// leaves nothing to keep, and its marker goes with the tab.
    pub(crate) fn park_or_drop_closing_pcb(&mut self, idx: usize, park_dirty: bool) {
        let Some(tab) = self.document_state.tabs.get(idx) else {
            return;
        };
        let path = tab.path.clone();
        // Its sketch mode ends with the tab (#2234). A close the user asked
        // for applied the sketch already (`close_tab`); a discard drops it.
        self.end_board_sketch_of(&path);
        let keep = park_dirty && self.document_state.is_document_dirty(&path);
        if keep
            && let Some(TabDocument::Pcb(engine)) =
                self.document_state.tabs[idx].cached_document.take()
        {
            self.document_state.park_pcb_board(path, engine);
            return;
        }
        self.document_state.mark_document_clean(&path);
    }

    /// Reopen `path` from the board parked when its dirty tab closed. The
    /// tab keeps the dirty mark and the board revision, so a save of the
    /// parked board still in flight completes against it. `false` when no
    /// board is parked for `path`.
    pub(crate) fn attach_parked_pcb_tab(&mut self, path: PathBuf, title: String) -> bool {
        let Some(engine) = self.document_state.take_parked_pcb_board(&path) else {
            return false;
        };
        self.park_active_schematic_session();
        let project_id = self.document_state.project_for_path(&path).map(|p| p.id);
        self.document_state.tabs.push(TabInfo {
            title,
            path,
            cached_document: Some(TabDocument::Pcb(engine)),
            project_id,
            kind: TabKind::Pcb,
        });
        self.document_state.active_tab = self.document_state.tabs.len() - 1;
        self.apply_loaded_pcb_document(true, true);
        true
    }
}
