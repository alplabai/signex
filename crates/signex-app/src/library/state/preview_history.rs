//! Undo / redo of a Component Preview's row, with a saved position (#1854).
//!
//! The row's edits form a line of whole-row snapshots. The depth the line
//! held when the row last equalled its stored copy is a [`SavePoint`], so
//! undoing every edit since the save reads clean again and a redo reads
//! dirty again. A point that no step can reproduce any more is forgotten and
//! the row stays dirty until the next save: when in doubt, dirty.

use signex_library::ComponentRow;

use super::ComponentPreviewState;
use super::preview::PinMapInlineState;
use crate::app::History;

/// Depth cap of the undo and redo stacks.
const HISTORY_DEPTH: usize = signex_history::DEFAULT_UNDO_DEPTH;

/// The snapshot history and saved position of one preview's row.
#[derive(Debug)]
pub struct RowHistory(History<ComponentRow>);

impl RowHistory {
    /// A row read from the library: the empty history is the saved state.
    pub fn at_start() -> Self {
        Self(History::at_start(HISTORY_DEPTH))
    }

    /// Keep `depth` undo steps from now on; lowering it drops the oldest.
    pub fn set_undo_depth(&mut self, depth: usize) {
        self.0.set_cap(depth);
    }

    /// How many undo steps are kept.
    pub fn undo_depth(&self) -> usize {
        self.0.cap()
    }

    /// Is the row away from the state the library stores (or is that state
    /// unknown)?
    pub fn is_dirty(&self) -> bool {
        self.0.is_dirty()
    }

    /// Every row an undo or redo can restore.
    pub fn rows(&self) -> impl Iterator<Item = &ComponentRow> {
        self.0.states()
    }

    /// Is there an edit to undo?
    pub fn can_undo(&self) -> bool {
        self.0.can_undo()
    }

    /// Is there an undone edit to redo?
    pub fn can_redo(&self) -> bool {
        self.0.can_redo()
    }

    /// Carry a cascade's pins and version into every state an undo or redo
    /// can restore, so stepping to one does not bring back a pre-cascade pin
    /// (#1840).
    pub fn rebase_onto_cascade(&mut self, stored: &ComponentRow) {
        for row in self.0.states_mut() {
            crate::library::preview_rebase::rebase_onto_cascade(row, stored);
        }
    }

    /// The current row is what a save is writing.
    pub fn mark_saved(&mut self) {
        self.0.mark_saved();
    }

    /// No state reproduces the stored copy: dirty until the next save.
    pub fn forget(&mut self) {
        self.0.forget();
    }

    /// Drop every step and forget the saved position: the row changed in a
    /// way a snapshot cannot undo (a bound primitive).
    pub fn drop_steps_stay_dirty(&mut self) {
        self.restart(false);
    }

    /// Drop every step; the current row is the saved state when `saved`.
    pub fn restart(&mut self, saved: bool) {
        self.0.restart(saved);
    }

    /// An edit turned `before` into the current row.
    fn record(&mut self, before: ComponentRow) {
        self.0.record(before);
    }

    /// Move one step: the state to show, the `current` one going to the
    /// opposite stack.
    fn step(&mut self, current: ComponentRow, undo: bool) -> Option<ComponentRow> {
        self.0.step(current, undo)
    }
}

/// What an inline edit started from, taken by
/// [`ComponentPreviewState::begin_edit`].
pub struct EditScope {
    before: ComponentRow,
    marks: u64,
}

impl ComponentPreviewState {
    /// Capture the row before an inline edit; settle it with
    /// [`Self::finish_edit`].
    pub fn begin_edit(&self) -> EditScope {
        EditScope {
            before: self.row.clone(),
            marks: self.dirty_marks,
        }
    }

    /// Record what an inline edit did. An edit that changed the row is one
    /// undo step; one that dirtied the preview without changing the row (the
    /// simulation model, which the row snapshots do not hold) or re-bound a
    /// primitive (whose resolved copy a snapshot does not hold) leaves no
    /// step that reproduces the stored copy, so the preview stays dirty
    /// until saved.
    pub fn finish_edit(&mut self, scope: EditScope) {
        if self.dirty_marks == scope.marks {
            return;
        }
        let before = scope.before;
        if self.row == before {
            self.history.forget();
        } else if self.row.symbol_ref != before.symbol_ref
            || self.row.footprint_ref != before.footprint_ref
            || self.row.sim_ref != before.sim_ref
        {
            self.history.drop_steps_stay_dirty();
        } else {
            self.history.record(before);
        }
        self.dirty = true;
        // A replaced pick is spent unless the row, the copy being saved or an
        // undo / redo step pins it.
        self.prune_staged_datasheets();
    }

    /// The row changed outside an inline edit and no snapshot can undo it
    /// (a primitive bound in the library): start a new history, still dirty
    /// exactly when the row was.
    pub fn restart_history(&mut self) {
        let saved = !self.dirty && !self.history.is_dirty();
        self.history.restart(saved);
        // The steps that pinned a PDF are gone.
        self.prune_staged_datasheets();
    }

    /// The row is being written: its current state is the saved position.
    pub fn begin_save(&mut self) {
        self.saving_row = Some(self.row.clone());
        self.history.mark_saved();
    }

    /// The save did not reach the library: the position it named never
    /// reached disk.
    pub fn abort_save(&mut self) {
        self.saving_row = None;
        self.history.forget();
    }

    /// The flag after the history moved: dirty away from the saved position,
    /// and a dirty row stays dirty while its save is in flight.
    pub fn sync_dirty(&mut self) {
        self.dirty = self.history.is_dirty() || (self.saving_row.is_some() && self.dirty);
    }

    /// Rebase the row and every history state onto the row a cascade
    /// rewrote (#1840).
    pub fn rebase_onto_cascade(&mut self, stored: &ComponentRow) {
        crate::library::preview_rebase::rebase_onto_cascade(&mut self.row, stored);
        self.history.rebase_onto_cascade(stored);
    }

    fn clear_edit_buffers(&mut self) {
        self.params_edit_buf.clear();
        self.params_prefix.clear();
        self.params_slot.clear();
        self.params_error.clear();
        self.params_note.clear();
        self.pin_map_state = PinMapInlineState::default();
    }

    /// Whether Undo has something to do: an edit, or half-typed text to drop.
    pub fn can_undo_step(&self) -> bool {
        self.history.can_undo() || !self.params_edit_buf.is_empty()
    }

    /// Whether Redo has something to do: never with half-typed text.
    pub fn can_redo_step(&self) -> bool {
        self.history.can_redo() && self.params_edit_buf.is_empty()
    }

    /// Undo or redo one edit. `false` when there is nothing to step to.
    /// Versions, pins and the base revision stay the current row's: only the
    /// edits move.
    pub fn step_history(&mut self, undo: bool) -> bool {
        // Half-typed text is not an edit yet: an undo only drops it, and a
        // redo (which re-applies an undone step) leaves it alone.
        if !self.params_edit_buf.is_empty() {
            if !undo {
                return false;
            }
            self.clear_edit_buffers();
            self.note_edit();
            return true;
        }
        let Some(mut target) = self.history.step(self.row.clone(), undo) else {
            return false;
        };
        crate::library::preview_rebase::rebase_onto_cascade(&mut target, &self.row);
        self.row = target;
        // The buffers hold text typed against the row that was just replaced.
        self.clear_edit_buffers();
        self.note_edit();
        self.sync_dirty();
        true
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use signex_library::LifecycleState;

    use super::*;
    use crate::library::component_preview::apply_inline_edit;
    use crate::library::component_preview::test_row;
    use crate::library::messages::EditorMsg;

    fn preview() -> ComponentPreviewState {
        ComponentPreviewState::from_row(PathBuf::from("/tmp/t.snxlib"), "t".into(), test_row())
    }

    fn set_state(state: &mut ComponentPreviewState, lifecycle: LifecycleState) {
        apply_inline_edit(state, EditorMsg::SetLifecycle(lifecycle));
    }

    #[test]
    fn undo_to_the_opened_row_reads_clean_and_redo_dirty() {
        let mut state = preview();
        set_state(&mut state, LifecycleState::Released);
        assert!(state.dirty);
        assert!(state.step_history(true));
        assert!(
            !state.dirty && !state.has_unsaved_edits(),
            "back at the file"
        );
        assert!(state.step_history(false));
        assert!(state.dirty, "redo leaves the file again");
    }

    #[test]
    fn undo_restores_the_edit_and_keeps_the_base_revision() {
        let mut state = preview();
        let before = state.row.state;
        state.row.server_revision = Some(3);
        set_state(&mut state, LifecycleState::Deprecated);
        state.row.server_revision = Some(9);
        assert!(state.step_history(true));
        assert_eq!(state.row.state, before);
        assert_eq!(state.row.server_revision, Some(9));
    }

    #[test]
    fn saved_position_is_where_the_save_snapshot_was_taken() {
        let mut state = preview();
        set_state(&mut state, LifecycleState::Released);
        state.begin_save();
        state.saving_row = None; // the write landed
        state.sync_dirty();
        assert!(!state.dirty);
        set_state(&mut state, LifecycleState::Deprecated);
        assert!(state.dirty);
        assert!(state.step_history(true));
        assert!(!state.dirty, "undo returns to what was saved");
        assert!(state.step_history(true));
        assert!(state.dirty, "undoing past the save leaves the file");
    }

    #[test]
    fn a_dirty_row_stays_dirty_while_its_save_is_in_flight() {
        let mut state = preview();
        set_state(&mut state, LifecycleState::Released);
        state.begin_save();
        assert!(state.step_history(true));
        assert!(state.step_history(false));
        assert!(state.dirty, "the write has not landed");
    }

    #[test]
    fn a_failed_save_forgets_the_position() {
        let mut state = preview();
        set_state(&mut state, LifecycleState::Released);
        state.begin_save();
        state.abort_save();
        set_state(&mut state, LifecycleState::Deprecated);
        assert!(state.step_history(true));
        assert!(state.dirty, "that state never reached the file");
    }

    #[test]
    fn a_dirtying_edit_that_leaves_the_row_alone_cannot_be_undone_clean() {
        let mut state = preview();
        set_state(&mut state, LifecycleState::Released);
        let scope = state.begin_edit();
        state.mark_dirty(); // e.g. the simulation model, not in the row
        state.finish_edit(scope);
        assert!(state.step_history(true));
        assert!(state.dirty);
    }

    #[test]
    fn a_half_typed_value_still_counts_as_unsaved_at_the_saved_position() {
        let mut state = preview();
        state.params_edit_buf.insert("v".into(), "1.".into());
        assert!(!state.dirty && state.has_unsaved_edits());
    }

    #[test]
    fn an_edit_after_undoing_below_the_saved_position_forgets_it() {
        let mut state = preview();
        set_state(&mut state, LifecycleState::Released);
        state.begin_save();
        state.saving_row = None;
        assert!(state.step_history(true));
        set_state(&mut state, LifecycleState::Deprecated);
        assert!(state.step_history(true));
        assert!(state.dirty, "the saved state is gone from the history");
    }

    #[test]
    fn the_opened_step_evicted_at_the_depth_cap_stays_dirty() {
        let mut state = preview();
        for i in 0..=HISTORY_DEPTH {
            let next = if i % 2 == 0 {
                LifecycleState::Released
            } else {
                LifecycleState::Deprecated
            };
            set_state(&mut state, next);
        }
        for _ in 0..HISTORY_DEPTH {
            assert!(state.step_history(true));
        }
        assert!(state.dirty, "the opened state fell off the stack");
    }

    #[test]
    fn undo_after_a_cascade_keeps_the_cascade_pin_of_a_restored_alternate() {
        use signex_library::{AlternateSymbol, PrimitiveRef};
        let mut row = crate::library::component_preview::test_row();
        let alt = PrimitiveRef::new(uuid::Uuid::now_v7(), uuid::Uuid::now_v7());
        row.alternate_symbols
            .push(AlternateSymbol::new(alt, "", "IEC"));
        row.alternate_symbols[0].symbol_version = "0.0.1".into();
        let mut state = ComponentPreviewState::from_row(
            PathBuf::from("/tmp/t.snxlib"),
            "t".into(),
            row.clone(),
        );
        apply_inline_edit(&mut state, EditorMsg::AlternateSymbolRemoved { idx: 0 });
        let mut stored = row;
        stored.alternate_symbols[0].symbol_version = "0.0.2".into();
        stored.server_revision = Some(9);
        state.rebase_onto_cascade(&stored); // what Rebased does
        assert!(state.step_history(true));
        assert_eq!(state.row.alternate_symbols[0].symbol_version, "0.0.2");
    }

    #[test]
    fn the_first_undo_with_a_half_typed_value_only_drops_the_text() {
        let mut state = preview();
        set_state(&mut state, LifecycleState::Released);
        state.params_edit_buf.insert("v".into(), "1.".into());
        assert!(state.step_history(true));
        assert!(state.params_edit_buf.is_empty());
        assert_eq!(state.row.state, LifecycleState::Released, "no step yet");
        assert!(state.step_history(true));
        assert!(!state.dirty);
    }

    #[test]
    fn a_redo_with_a_half_typed_value_does_nothing_and_keeps_the_text() {
        let mut state = preview();
        set_state(&mut state, LifecycleState::Released);
        assert!(state.step_history(true));
        state.params_edit_buf.insert("v".into(), "1.".into());
        assert!(!state.can_redo_step());
        assert!(!state.step_history(false));
        assert_eq!(state.params_edit_buf.len(), 1, "the text stays");
        assert!(state.history.can_redo(), "the undone step is still there");
    }
}
