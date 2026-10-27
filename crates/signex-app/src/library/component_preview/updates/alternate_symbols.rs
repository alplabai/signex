//! Alternate-symbol edits for a Component Preview row: remove, relabel and
//! reorder. Adding goes through the symbol picker (it must read both
//! symbols), see `app/dispatch/library/alternate_symbols.rs`.

use crate::library::ComponentPreviewState;

/// Drop the alternate at `idx`.
pub(super) fn remove(state: &mut ComponentPreviewState, idx: usize) {
    if signex_library::remove_alternate_symbol(&mut state.row, idx) {
        state.alternate_error = None;
        state.dirty = true;
    }
}

/// Set the label of the alternate at `idx`.
pub(super) fn relabel(state: &mut ComponentPreviewState, idx: usize, label: String) {
    if signex_library::relabel_alternate_symbol(&mut state.row, idx, label) {
        state.dirty = true;
    }
}

/// Move the alternate at `from` to position `to`.
pub(super) fn move_to(state: &mut ComponentPreviewState, from: usize, to: usize) {
    if signex_library::move_alternate_symbol(&mut state.row, from, to) {
        state.dirty = true;
    }
}
