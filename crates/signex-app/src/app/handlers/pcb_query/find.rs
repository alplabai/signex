//! Find and Replace on a board (#2403). With a board in view the dialog
//! searches it (`signex_engine::pcb::find`): designators, values, footprint
//! names and nets. A result selects what it names and frames it; Replace
//! writes designators and values as one board undo step and leaves net and
//! footprint results alone, saying how many.

use signex_engine::pcb::find::{replace_commands, scan_board};
use signex_i18n::{ids, t};

use crate::app::Signex;
use crate::find_replace::BoardFind;

impl Signex {
    /// Re-run the search on the board in view. Whether a board is in view:
    /// a sheet's search runs instead when not.
    pub(crate) fn refresh_board_find(&mut self) -> bool {
        let path = self.active_tab_path().filter(|_| self.has_active_pcb());
        let Some(path) = path else {
            self.ui_state.find_replace.board = None;
            return false;
        };
        let query = self.ui_state.find_replace.query.trim().to_string();
        let hits = self
            .active_pcb()
            .map(|board| scan_board(board, &query))
            .unwrap_or_default();
        let find = &mut self.ui_state.find_replace;
        find.matches.clear();
        find.scanning = false;
        let previous = find.selected_index.unwrap_or(0);
        find.selected_index = (!hits.is_empty()).then(|| previous.min(hits.len() - 1));
        // The current result is selected, without moving the view, as on a
        // sheet.
        let current = find
            .selected_index
            .and_then(|index| hits.get(index))
            .map(|hit| hit.items.clone());
        find.board = Some(BoardFind { path, hits });
        if let Some(items) = current {
            self.select_pcb_items(&items);
        }
        true
    }

    /// The board search is current for the board in view.
    fn board_find_in_view(&self) -> bool {
        let board = self.ui_state.find_replace.board.as_ref();
        board.is_some_and(|b| self.active_tab_path().as_deref() == Some(b.path.as_path()))
    }

    /// The dialog lists a board that is no longer in view (another tab
    /// was opened since): its results must not be acted on.
    pub(crate) fn board_find_is_stale(&self) -> bool {
        self.ui_state.find_replace.board.is_some() && !self.board_find_in_view()
    }

    /// Go to board result `index`: select what it names and frame it.
    /// `false` when the dialog's results are not a board's.
    pub(crate) fn focus_board_find_result(&mut self, index: usize) -> bool {
        if self.ui_state.find_replace.board.is_none() {
            return false;
        }
        let Some((path, items)) = self
            .ui_state
            .find_replace
            .board
            .as_ref()
            .filter(|_| self.board_find_in_view())
            .and_then(|b| Some((b.path.clone(), b.hits.get(index)?.items.clone())))
        else {
            return true;
        };
        self.ui_state.find_replace.selected_index = Some(index);
        self.select_pcb_items(&items);
        let window = self.interaction_state.acting_window;
        self.fit_board_items(window, &path, &items);
        true
    }

    /// Find next or previous on a board: re-read the board, then step,
    /// wrapping at the ends. `false` when the dialog's results are not a
    /// board's.
    pub(crate) fn step_board_find_result(&mut self, forward: bool) -> bool {
        if !self.refresh_board_find() {
            return false;
        }
        let count = self.ui_state.find_replace.result_count();
        if count == 0 {
            return true;
        }
        let next = match (self.ui_state.find_replace.selected_index, forward) {
            (None, true) => 0,
            (None, false) => count - 1,
            (Some(i), true) => (i + 1) % count,
            (Some(i), false) => (i + count - 1) % count,
        };
        self.focus_board_find_result(next)
    }

    /// Replace on a board: the selected result (`only`) or all of them, as
    /// one undo step. `false` when the dialog's results are not a board's.
    pub(crate) fn replace_on_board(&mut self, only: Option<usize>) -> bool {
        if !self.board_find_in_view() {
            return self.ui_state.find_replace.board.is_some();
        }
        let find = &self.ui_state.find_replace;
        let Some(board_find) = find.board.as_ref() else {
            return false;
        };
        let hits: Vec<_> = match only {
            Some(index) => board_find.hits.get(index).cloned().into_iter().collect(),
            None => board_find.hits.clone(),
        };
        let (query, replacement) = (find.query.clone(), find.replacement.clone());
        let kept = hits.iter().filter(|h| !h.kind.is_replaceable()).count();
        let Some(commands) = self
            .active_pcb()
            .map(|board| replace_commands(board, &hits, &query, &replacement))
        else {
            return true;
        };
        let i18n = self.ui_state.i18n.clone();
        if kept > 0 {
            let line = t!(i18n, ids::BOARD_FIND_KEPT, "count" => kept);
            crate::diagnostics::log_info(line.into_string());
        }
        let changed = commands.len();
        if changed == 0 {
            crate::diagnostics::log_info(t!(i18n, ids::BOARD_FIND_NOTHING_REPLACED).into_string());
        } else {
            let what = t!(i18n, ids::BOARD_FIND_REPLACED, "count" => changed).into_string();
            if self.run_placement(&what, Ok(commands)) {
                crate::diagnostics::log_info(what);
            }
        }
        self.refresh_board_find();
        true
    }
}
