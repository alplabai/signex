//! Find Similar Objects on a board (#2403): the seed is the object under
//! the cursor, else the one selected object; the dialog lists its
//! properties, and OK selects every object of the board that matches the
//! ones set to Same (`signex_engine::pcb::similar`).

use signex_engine::pcb::similar;
use signex_i18n::{ids, t};

use crate::app::Signex;
use crate::find_similar::FindSimilarState;

impl Signex {
    /// Open the dialog for the board seed, or say in the Messages panel
    /// why there is none.
    pub(crate) fn open_board_find_similar(&mut self) {
        let seed = self
            .with_pcb_selection(|sel, _| {
                sel.hover().or_else(|| {
                    let mut picked = sel.selection().items();
                    match (picked.next(), picked.next()) {
                        (Some(only), None) => Some(only),
                        _ => None,
                    }
                })
            })
            .flatten();
        let i18n = self.ui_state.i18n.clone();
        let Some(seed) = seed else {
            crate::diagnostics::log_warning(t!(i18n, ids::BOARD_SIMILAR_NO_SEED).into_string());
            return;
        };
        let Some(board) = self.active_pcb() else {
            return;
        };
        let values = similar::properties_of(board, seed);
        if values.is_empty() {
            crate::diagnostics::log_warning(t!(i18n, ids::BOARD_SIMILAR_GONE).into_string());
            return;
        }
        let label = similar::describe(board, seed);
        let origin = self.active_tab_path();
        self.ui_state.find_similar = Some(FindSimilarState::new(label, values, origin).on_board());
    }

    /// OK on a board: select what the criteria match and close the dialog.
    pub(crate) fn select_similar_on_board(&mut self) {
        let Some(state) = self.ui_state.find_similar.take() else {
            return;
        };
        let Some(items) = self
            .active_pcb()
            .map(|board| similar::find_similar(board, &state.criteria()))
        else {
            return;
        };
        let i18n = self.ui_state.i18n.clone();
        if items.is_empty() {
            crate::diagnostics::log_warning(t!(i18n, ids::BOARD_SIMILAR_NONE).into_string());
            return;
        }
        self.select_pcb_items(&items);
        let line = t!(i18n, ids::BOARD_SIMILAR_SELECTED, "count" => items.len());
        crate::diagnostics::log_info(line.into_string());
    }
}
