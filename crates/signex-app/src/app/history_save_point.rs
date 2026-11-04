//! Where a snapshot-undo history last equalled the file on disk (#1854).
//!
//! A draft's undo stack is a line of states: `undo_len` steps behind the
//! current one, any number of redo steps ahead. [`SavePoint`] remembers the
//! depth the draft held when its content last equalled its file, so an Undo
//! back to that depth reads clean and a Redo away from it reads dirty.
//!
//! Depth is counted from the oldest step ever recorded (`evicted + undo_len`)
//! so a full stack that drops its oldest step keeps every depth meaning the
//! same state. The point is **forgotten** (unknown, so the draft stays dirty
//! until the next save) whenever no history step can reproduce the file any
//! more: its step was evicted or cut off the redo branch, an edit amended the
//! state at the point without a step, or a history rewrite removed it.
//! Unknown is the default: content not known to be on disk is never clean.
//!
//! Domain-neutral: it knows stack lengths, not what a step holds.

/// The saved depth of one undo history, or unknown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SavePoint {
    /// Steps dropped off the bottom of the undo stack so far.
    evicted: u64,
    /// Absolute depth the content equalled the file at; `None` = unknown.
    saved: Option<u64>,
}

impl SavePoint {
    /// Content freshly read from disk: the empty history is the saved state.
    pub(crate) const fn at_start() -> Self {
        Self {
            evicted: 0,
            saved: Some(0),
        }
    }

    /// Content not known to be on disk (a repaired load, a recovery copy).
    pub(crate) const fn unknown() -> Self {
        Self {
            evicted: 0,
            saved: None,
        }
    }

    fn depth(&self, undo_len: usize) -> u64 {
        self.evicted + undo_len as u64
    }

    /// Does the state `undo_len` steps deep equal the file?
    pub(crate) fn is_at(&self, undo_len: usize) -> bool {
        self.saved == Some(self.depth(undo_len))
    }

    /// The current state is what a save is writing.
    pub(crate) fn mark_saved(&mut self, undo_len: usize) {
        self.saved = Some(self.depth(undo_len));
    }

    /// No history step reproduces the file any more.
    pub(crate) fn forget(&mut self) {
        self.saved = None;
    }

    /// The state at the point changed with no step recorded (an unrecorded
    /// edit, or one amended into the newest step): the point no longer
    /// describes it. A point behind or ahead of the state is unaffected.
    pub(crate) fn forget_if_at(&mut self, undo_len: usize) {
        if self.is_at(undo_len) {
            self.forget();
        }
    }

    /// A new step is about to be recorded on an undo stack of `undo_len`
    /// steps and the redo stack cleared: a point on the redo branch is cut.
    pub(crate) fn redo_cleared(&mut self, undo_len: usize) {
        if self.saved.is_some_and(|saved| saved > self.depth(undo_len)) {
            self.forget();
        }
    }

    /// The oldest undo step was dropped to keep the stack within its cap.
    pub(crate) fn evicted_oldest(&mut self) {
        self.evicted += 1;
        if self.saved.is_some_and(|saved| saved < self.evicted) {
            self.forget();
        }
    }

    /// The stacks hold `undo_len` and `redo_len` steps: a point outside the
    /// reachable range (a capped redo stack lost its far end) is forgotten.
    pub(crate) fn limit_to(&mut self, undo_len: usize, redo_len: usize) {
        let reach = self.depth(undo_len) + redo_len as u64;
        if self
            .saved
            .is_some_and(|saved| saved < self.evicted || saved > reach)
        {
            self.forget();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_load_is_saved_and_unknown_is_never() {
        assert!(SavePoint::at_start().is_at(0));
        assert!(!SavePoint::unknown().is_at(0));
    }

    #[test]
    fn undo_back_to_the_point_is_at_it_and_redo_is_not() {
        let mut point = SavePoint::at_start();
        point.redo_cleared(0); // an edit: 1 step behind the state
        assert!(!point.is_at(1));
        assert!(point.is_at(0), "undo returns to the saved depth");
    }

    #[test]
    fn an_edit_after_undoing_below_the_point_forgets_it() {
        let mut point = SavePoint::unknown();
        point.mark_saved(3);
        point.redo_cleared(2); // undone twice below, then a new edit
        assert!(!point.is_at(3) && !point.is_at(2));
        assert_eq!(point, SavePoint::unknown());
    }

    #[test]
    fn an_edit_from_the_point_keeps_it() {
        let mut point = SavePoint::unknown();
        point.mark_saved(3);
        point.redo_cleared(3);
        assert!(point.is_at(3));
    }

    #[test]
    fn eviction_shifts_depths_and_forgets_an_evicted_point() {
        let mut point = SavePoint::unknown();
        point.mark_saved(5);
        point.evicted_oldest(); // stack of 5 + new step, oldest dropped
        assert!(point.is_at(4), "the same state, one slot lower");
        let mut at_bottom = SavePoint::at_start();
        at_bottom.evicted_oldest();
        assert!(!at_bottom.is_at(0), "its step fell off the stack");
    }

    #[test]
    fn an_in_place_edit_at_the_point_forgets_it_but_not_elsewhere() {
        let mut point = SavePoint::unknown();
        point.mark_saved(2);
        point.forget_if_at(3);
        assert!(point.is_at(2));
        point.forget_if_at(2);
        assert!(!point.is_at(2));
    }

    #[test]
    fn a_redo_stack_losing_its_far_end_forgets_a_point_there() {
        let mut point = SavePoint::unknown();
        point.mark_saved(5);
        point.limit_to(2, 3); // reach 5
        assert!(point.is_at(5));
        point.limit_to(2, 2); // reach 4
        assert!(!point.is_at(5));
    }
}
