//! One snapshot undo/redo history with its saved position (#661, #1854).
//!
//! The two stacks, the depth cap and the [`SavePoint`] bookkeeping every
//! snapshot-undo draft needs (a Component row, a symbol file, a footprint
//! file). What a step holds is the caller's: this type only knows stack
//! lengths, so the save-point rules live in one place.

use super::SavePoint;

/// Undo and redo stacks of whole-state snapshots plus the saved position.
#[derive(Debug)]
pub(crate) struct History<S> {
    /// Older states, newest last.
    undo: Vec<S>,
    /// States undone, the next redo last.
    redo: Vec<S>,
    point: SavePoint,
    cap: usize,
}

impl<S> History<S> {
    /// Content freshly read from disk: the empty history is the saved state.
    pub(crate) fn at_start(cap: usize) -> Self {
        Self::with_point(cap, SavePoint::at_start())
    }

    /// Content not known to be on disk: dirty until the next save.
    pub(crate) fn unknown(cap: usize) -> Self {
        Self::with_point(cap, SavePoint::unknown())
    }

    fn with_point(cap: usize, point: SavePoint) -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            point,
            cap,
        }
    }

    pub(crate) fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub(crate) fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub(crate) fn undo_len(&self) -> usize {
        self.undo.len()
    }

    pub(crate) fn redo_len(&self) -> usize {
        self.redo.len()
    }

    /// Older states, newest last.
    pub(crate) fn undo_steps(&self) -> &[S] {
        &self.undo
    }

    /// Undone states, the next redo last.
    pub(crate) fn redo_steps(&self) -> &[S] {
        &self.redo
    }

    /// Every state an undo or redo can restore.
    pub(crate) fn states(&self) -> impl Iterator<Item = &S> {
        self.undo.iter().chain(self.redo.iter())
    }

    /// [`Self::states`] for in-place rewrites (a cascade rebase).
    pub(crate) fn states_mut(&mut self) -> impl Iterator<Item = &mut S> {
        self.undo.iter_mut().chain(self.redo.iter_mut())
    }

    /// The undo stack for a caller that rewrites steps by hand. It owns the
    /// save-point consequence: [`Self::forget`] or [`Self::settle_point`].
    pub(crate) fn undo_steps_mut(&mut self) -> &mut Vec<S> {
        &mut self.undo
    }

    /// The redo stack, see [`Self::undo_steps_mut`].
    pub(crate) fn redo_steps_mut(&mut self) -> &mut Vec<S> {
        &mut self.redo
    }

    /// Push onto the undo stack, dropping the oldest step past the cap.
    /// Returns the dropped step.
    fn push_undo(&mut self, state: S) -> Option<S> {
        self.undo.push(state);
        let dropped = (self.undo.len() > self.cap).then(|| self.undo.remove(0));
        if dropped.is_some() {
            self.point.evicted_oldest();
        }
        dropped
    }

    /// An edit turned `before` into the current state: one new undo step,
    /// the redo branch cut. Returns the step pushed off a full stack.
    pub(crate) fn record(&mut self, before: S) -> Option<S> {
        self.point.redo_cleared(self.undo.len());
        self.redo.clear();
        self.push_undo(before)
    }

    /// First half of an Undo or Redo: take the state to show. The caller
    /// captures the current state, then hands it to [`Self::finish_step`].
    pub(crate) fn begin_step(&mut self, undo: bool) -> Option<S> {
        if undo {
            self.undo.pop()
        } else {
            self.redo.pop()
        }
    }

    /// Second half of an Undo or Redo: `current` goes to the opposite stack.
    pub(crate) fn finish_step(&mut self, current: S, undo: bool) {
        if undo {
            self.redo.push(current);
            if self.redo.len() > self.cap {
                self.redo.remove(0);
            }
        } else {
            self.push_undo(current);
        }
        self.settle_point();
    }

    /// Move one step: the state to show, `current` going to the other stack.
    pub(crate) fn step(&mut self, current: S, undo: bool) -> Option<S> {
        let target = self.begin_step(undo)?;
        self.finish_step(current, undo);
        Some(target)
    }

    /// Is the state away from the saved position (or is that unknown)?
    pub(crate) fn is_dirty(&self) -> bool {
        !self.point.is_at(self.undo.len())
    }

    /// The current state is what a save is writing.
    pub(crate) fn mark_saved(&mut self) {
        self.point.mark_saved(self.undo.len());
    }

    /// No state reproduces the saved copy: dirty until the next save.
    pub(crate) fn forget(&mut self) {
        self.point.forget();
    }

    /// The current state changed with no step recorded: a point at it no
    /// longer describes it.
    pub(crate) fn forget_if_current(&mut self) {
        self.point.forget_if_at(self.undo.len());
    }

    /// Forget a point the stacks can no longer reach, after a hand rewrite.
    pub(crate) fn settle_point(&mut self) {
        self.point.limit_to(self.undo.len(), self.redo.len());
    }

    /// The saved position, for a step that gives it back (a rejected edit).
    pub(crate) fn point(&self) -> SavePoint {
        self.point
    }

    pub(crate) fn set_point(&mut self, point: SavePoint) {
        self.point = point;
    }

    /// Drop every step; the current state is the saved one when `saved`.
    pub(crate) fn restart(&mut self, saved: bool) {
        self.undo.clear();
        self.redo.clear();
        self.point = if saved {
            SavePoint::at_start()
        } else {
            SavePoint::unknown()
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CAP: usize = 3;

    fn history() -> History<u32> {
        History::at_start(CAP)
    }

    #[test]
    fn record_undo_redo_round_trip() {
        let mut h = history();
        h.record(0);
        h.record(1);
        assert_eq!(h.step(2, true), Some(1));
        assert_eq!(h.step(1, true), Some(0));
        assert_eq!(h.step(0, true), None);
        assert_eq!(h.step(0, false), Some(1));
        assert_eq!((h.undo_len(), h.redo_len()), (1, 1));
        assert!(h.can_undo() && h.can_redo());
    }

    #[test]
    fn undo_back_to_the_saved_depth_is_clean_and_redo_dirty() {
        let mut h = history();
        assert!(!h.is_dirty());
        h.record(0);
        assert!(h.is_dirty());
        h.step(1, true);
        assert!(!h.is_dirty());
        h.step(0, false);
        assert!(h.is_dirty());
    }

    #[test]
    fn eviction_keeps_the_saved_state_meaning_or_forgets_it() {
        let mut h = history();
        h.record(0);
        h.mark_saved(); // saved at depth 1
        h.record(1);
        h.record(2);
        assert_eq!(h.record(3), Some(0), "the oldest step is dropped");
        assert!(h.is_dirty(), "the step the point sat on is gone");

        let mut h = history();
        for i in 0..2 {
            h.record(i);
        }
        h.mark_saved(); // depth 2
        h.record(2);
        h.record(3); // evicts step 0, point shifts with the depths
        h.step(4, true);
        h.step(3, true);
        assert!(!h.is_dirty(), "two undos land on the saved state");
    }

    #[test]
    fn a_new_edit_cuts_a_point_on_the_redo_branch() {
        let mut h = history();
        h.record(0);
        h.record(1);
        h.mark_saved(); // depth 2
        h.step(2, true);
        h.step(1, true); // depth 0, the point is on the redo branch
        h.record(9);
        assert!(h.is_dirty());
        h.step(10, true);
        assert!(h.is_dirty(), "the saved state is not reachable any more");
    }

    #[test]
    fn a_capped_redo_stack_forgets_a_point_off_its_far_end() {
        let mut h = history();
        for i in 0..3 {
            h.record(i);
        }
        h.mark_saved(); // depth 3
        for i in 0..3 {
            h.step(i, true);
        }
        assert_eq!(h.redo_len(), 3);
        h.record(7); // new branch
        assert_eq!(h.redo_len(), 0);
        assert!(h.is_dirty());
    }

    #[test]
    fn restart_and_forget_variants() {
        let mut h = history();
        h.record(0);
        h.restart(true);
        assert!(!h.is_dirty() && !h.can_undo());
        h.forget();
        assert!(h.is_dirty());
        h.restart(false);
        assert!(h.is_dirty());
        let mut h = History::<u32>::unknown(CAP);
        h.mark_saved();
        h.forget_if_current();
        assert!(h.is_dirty());
    }
}
