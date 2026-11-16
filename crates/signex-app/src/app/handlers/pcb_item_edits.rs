//! Locks of tracks and vias (#2407) and the steps the agent commands share
//! with the Properties rows: the lock, the via span and per-layer copper
//! sizes (#2401). Each is one board undo step through `run_board_step`.

use std::path::Path;

use pcb_geom::ItemRef;
use signex_engine::pcb::primitive_edit::{PadEdit, ViaEdit, pad_edit_command, via_edit_command};
use signex_engine::pcb::{BoardCommand, BoardItemId};
use signex_i18n::{MessageId, ids, t};
use signex_types::pcb::PcbBoard;
use uuid::Uuid;

use crate::app::Signex;

/// The tracks and vias of a board selection: what a lock acts on.
pub(crate) fn lockable_items(selection: &pcb_geom::Selection) -> Vec<BoardItemId> {
    lock_targets(&selection.items().collect::<Vec<_>>())
}

/// The tracks and vias among `items`.
pub(crate) fn lock_targets(items: &[ItemRef]) -> Vec<BoardItemId> {
    items
        .iter()
        .filter_map(|item| match *item {
            ItemRef::Track(id) => Some(BoardItemId::Segment(id)),
            ItemRef::Via(id) => Some(BoardItemId::Via(id)),
            _ => None,
        })
        .collect()
}

/// The track or via with uuid `id` on `board`.
pub(crate) fn track_or_via(board: &PcbBoard, id: Uuid) -> Option<BoardItemId> {
    if board.segments.iter().any(|s| s.uuid == id) {
        Some(BoardItemId::Segment(id))
    } else if board.vias.iter().any(|v| v.uuid == id) {
        Some(BoardItemId::Via(id))
    } else {
        None
    }
}

impl Signex {
    /// `lock_board_items` / `unlock_board_items` from a menu, a key or the
    /// palette: the tracks and vias selected on the board in view. Says why
    /// in Messages when there are none or the board refused.
    pub(crate) fn lock_pcb_selection(&mut self, locked: bool) {
        let Some(path) = self.active_tab_path() else {
            return;
        };
        let items = self
            .with_pcb_selection(|state, _| lockable_items(state.selection()))
            .unwrap_or_default();
        if items.is_empty() {
            let line = t!(self.ui_state.i18n, ids::PCB_PRIM_LOCK_NOTHING);
            crate::diagnostics::log_info(line.into_string());
            return;
        }
        if let Err(why) = self.set_items_locked(&path, &items, locked) {
            self.refuse_item_edit(ids::PCB_PRIM_LOCK, why);
        }
        self.sync_pcb_board_summary();
    }

    /// Lock or unlock `items` (tracks and vias) on the board at `path` as one
    /// undo step. Items already so are left out; with none left nothing is
    /// done.
    pub(crate) fn set_items_locked(
        &mut self,
        path: &Path,
        items: &[BoardItemId],
        locked: bool,
    ) -> Result<(), String> {
        let board = self
            .document_state
            .pcb_board(path)
            .ok_or_else(|| self.board_gone(ids::PCB_PRIM_LOCK))?;
        let changes: Vec<(BoardItemId, bool)> = items
            .iter()
            .filter(|id| signex_engine::pcb::lock::is_locked(board, **id) != locked)
            .map(|id| (*id, locked))
            .collect();
        if changes.is_empty() {
            return Ok(());
        }
        let label = t!(self.ui_state.i18n, ids::PCB_PRIM_LOCK).into_string();
        let command = BoardCommand::SetItemsLocked { items: changes };
        self.run_board_step(path, vec![command], &label)
    }

    /// Run a via edit on `vias` of the board at `path` as one step labelled
    /// `what`. Nothing to change is no step.
    pub(crate) fn edit_vias(
        &mut self,
        path: &Path,
        vias: &[Uuid],
        edit: ViaEdit,
        what: MessageId,
    ) -> Result<(), String> {
        let board = self
            .document_state
            .pcb_board(path)
            .ok_or_else(|| self.board_gone(what))?;
        let command = via_edit_command(board, vias, edit).map_err(|r| r.to_string())?;
        let Some(command) = command else {
            return Ok(());
        };
        let label = t!(self.ui_state.i18n, what).into_string();
        self.run_board_step(path, vec![command], &label)
    }

    /// Run a pad edit on `pads` (footprint, pad) of the board at `path` as
    /// one step labelled `what`.
    pub(crate) fn edit_pads(
        &mut self,
        path: &Path,
        pads: &[(Uuid, Uuid)],
        edit: &PadEdit,
        what: MessageId,
    ) -> Result<(), String> {
        let board = self
            .document_state
            .pcb_board(path)
            .ok_or_else(|| self.board_gone(what))?;
        let command = pad_edit_command(board, pads, edit).map_err(|r| r.to_string())?;
        let Some(command) = command else {
            return Ok(());
        };
        let label = t!(self.ui_state.i18n, what).into_string();
        self.run_board_step(path, vec![command], &label)
    }

    fn board_gone(&self, what: MessageId) -> String {
        let name = t!(self.ui_state.i18n, what).into_string();
        t!(self.ui_state.i18n, ids::PCB_PRIM_BOARD_GONE, "what" => name).into_string()
    }

    /// Say in Messages that `what` was not changed and why.
    pub(crate) fn refuse_item_edit(&self, what: MessageId, why: String) {
        let name = t!(self.ui_state.i18n, what).into_string();
        let line = t!(
            self.ui_state.i18n,
            ids::PCB_PRIM_REFUSED,
            "what" => name,
            "why" => why
        );
        crate::diagnostics::log_warning(line.into_string());
    }
}

#[cfg(test)]
#[path = "pcb_item_edits_tests.rs"]
mod tests;
