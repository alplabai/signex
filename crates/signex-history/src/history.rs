//! One snapshot undo/redo history with its saved position (#661, #1854).
//!
//! The two stacks, the depth cap and the [`SavePoint`] bookkeeping every
//! snapshot-undo draft needs (a Component row, a symbol file, a footprint
//! file, the schematic engine). What a step holds is the caller's: this type
//! only knows stack lengths, so the save-point rules live in one place.

use crate::SavePoint;

/// How many undo steps every history keeps unless the `undo_depth` setting
/// says otherwise.
pub const DEFAULT_UNDO_DEPTH: usize = 100;

/// The fewest steps a history can keep.
pub const MIN_UNDO_DEPTH: usize = 1;

/// The largest accepted `undo_depth`: a deeper history only costs memory.
pub const MAX_UNDO_DEPTH: usize = 1000;

/// Is `depth` a usable `undo_depth` (at least one step, at most
/// [`MAX_UNDO_DEPTH`])?
pub const fn undo_depth_in_range(depth: usize) -> bool {
    depth >= MIN_UNDO_DEPTH && depth <= MAX_UNDO_DEPTH
}

/// Undo and redo stacks of whole-state snapshots plus the saved position.
#[derive(Debug, Clone)]
pub struct History<S> {
    /// Older states, newest last.
    undo: Vec<S>,
    /// States undone, the next redo last.
    redo: Vec<S>,
    point: SavePoint,
    cap: usize,
}

impl<S> History<S> {
    /// Content freshly read from disk: the empty history is the saved state.
    pub fn at_start(cap: usize) -> Self {
        Self::with_point(cap, SavePoint::at_start())
    }

    /// Content not known to be on disk: dirty until the next save.
    pub fn unknown(cap: usize) -> Self {
        Self::with_point(cap, SavePoint::unknown())
    }

    fn with_point(cap: usize, point: SavePoint) -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            point,
            // A history that keeps nothing could never undo what it records.
            cap: cap.max(1),
        }
    }

    /// How many undo steps are kept.
    pub fn cap(&self) -> usize {
        self.cap
    }

    /// Keep `cap` steps from now on. Lowering it drops the oldest undo steps
    /// (and the far end of the redo stack) at once; a save point that falls
    /// off the bottom is forgotten, so the document stays dirty until the
    /// next save.
    pub fn set_cap(&mut self, cap: usize) {
        if cap.max(1) == self.cap {
            return;
        }
        self.cap = cap.max(1);
        while self.undo.len() > self.cap {
            self.undo.remove(0);
            self.point.evicted_oldest();
        }
        if self.redo.len() > self.cap {
            self.redo.drain(..self.redo.len() - self.cap);
        }
        self.settle_point();
    }

    /// Position in the edit timeline: the steps applied now, counting those
    /// the cap evicted. An undo lowers it, a redo raises it, an edit raises
    /// it and cuts the redo branch.
    pub fn position(&self) -> usize {
        self.point.position(self.undo.len())
    }

    /// Drop the newest redo step, the one an undo just produced, so no redo
    /// can bring it back (a run that failed halfway and is reported as never
    /// having happened). A save point on the state it held is forgotten.
    pub fn discard_top(&mut self) -> Option<S> {
        let top = self.redo.pop()?;
        self.point.forget_if_at(self.undo.len() + 1);
        Some(top)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }

    pub fn redo_len(&self) -> usize {
        self.redo.len()
    }

    /// Older states, newest last.
    pub fn undo_steps(&self) -> &[S] {
        &self.undo
    }

    /// Undone states, the next redo last.
    pub fn redo_steps(&self) -> &[S] {
        &self.redo
    }

    /// Every state an undo or redo can restore.
    pub fn states(&self) -> impl Iterator<Item = &S> {
        self.undo.iter().chain(self.redo.iter())
    }

    /// [`Self::states`] for in-place rewrites (a cascade rebase).
    pub fn states_mut(&mut self) -> impl Iterator<Item = &mut S> {
        self.undo.iter_mut().chain(self.redo.iter_mut())
    }

    /// The undo stack for a caller that rewrites steps by hand. It owns the
    /// save-point consequence: [`Self::forget`] or [`Self::settle_point`].
    pub fn undo_steps_mut(&mut self) -> &mut Vec<S> {
        &mut self.undo
    }

    /// The redo stack, see [`Self::undo_steps_mut`].
    pub fn redo_steps_mut(&mut self) -> &mut Vec<S> {
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
    pub fn record(&mut self, before: S) -> Option<S> {
        self.point.redo_cleared(self.undo.len());
        self.redo.clear();
        self.push_undo(before)
    }

    /// A step that changes nothing in the state (a marker): one new undo
    /// step with the redo branch kept, so an edit undone before it is still
    /// redone after it. Returns the step pushed off a full stack.
    pub fn record_inert(&mut self, step: S) -> Option<S> {
        self.point.inserted_below(self.undo.len());
        self.push_undo(step)
    }

    /// First half of an Undo or Redo: take the state to show. The caller
    /// captures the current state, then hands it to [`Self::finish_step`].
    pub fn begin_step(&mut self, undo: bool) -> Option<S> {
        if undo {
            self.undo.pop()
        } else {
            self.redo.pop()
        }
    }

    /// Second half of an Undo or Redo: `current` goes to the opposite stack.
    pub fn finish_step(&mut self, current: S, undo: bool) {
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
    pub fn step(&mut self, current: S, undo: bool) -> Option<S> {
        let target = self.begin_step(undo)?;
        self.finish_step(current, undo);
        Some(target)
    }

    /// Is the state away from the saved position (or is that unknown)?
    pub fn is_dirty(&self) -> bool {
        !self.point.is_at(self.undo.len())
    }

    /// [`Self::is_dirty`], where the steps between the state and the saved
    /// position that `inert` holds for do not count (a marker that changes
    /// nothing in the state).
    pub fn is_dirty_past(&self, inert: impl Fn(&S) -> bool) -> bool {
        let Some(to_saved) = self.point.steps_to(self.undo.len()) else {
            return true;
        };
        let count = to_saved.unsigned_abs() as usize;
        let (stack, from) = if to_saved <= 0 {
            // The file is behind: the newest undo steps lie between.
            (&self.undo, self.undo.len().checked_sub(count))
        } else {
            // The file is ahead: the next redo steps lie between.
            (&self.redo, self.redo.len().checked_sub(count))
        };
        from.is_none_or(|from| !stack[from..].iter().all(inert))
    }

    /// The current state is what a save is writing.
    pub fn mark_saved(&mut self) {
        self.point.mark_saved(self.undo.len());
    }

    /// No state reproduces the saved copy: dirty until the next save.
    pub fn forget(&mut self) {
        self.point.forget();
    }

    /// The current state changed with no step recorded: a point at it no
    /// longer describes it.
    pub fn forget_if_current(&mut self) {
        self.point.forget_if_at(self.undo.len());
    }

    /// Forget a point the stacks can no longer reach, after a hand rewrite.
    pub fn settle_point(&mut self) {
        self.point.limit_to(self.undo.len(), self.redo.len());
    }

    /// The saved position, for a step that gives it back (a rejected edit).
    pub fn point(&self) -> SavePoint {
        self.point
    }

    pub fn set_point(&mut self, point: SavePoint) {
        self.point = point;
    }

    /// Drop every step; the current state is the saved one when `saved`.
    pub fn restart(&mut self, saved: bool) {
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

    /// Odd steps are inert (markers); even ones change the state.
    fn inert(step: &u32) -> bool {
        step % 2 == 1
    }

    #[test]
    fn inert_steps_between_the_state_and_the_save_do_not_dirty_it() {
        let mut h = history();
        h.record(1);
        assert!(
            h.is_dirty() && !h.is_dirty_past(inert),
            "behind by one marker"
        );
        h.record(2);
        assert!(h.is_dirty_past(inert), "a real step lies between");
        let s = h.begin_step(true).unwrap();
        h.finish_step(s, true);
        assert!(!h.is_dirty_past(inert), "only the marker is left between");
    }

    #[test]
    fn an_inert_gap_ahead_and_an_unknown_point() {
        let mut h = history();
        h.record(1);
        h.mark_saved();
        let s = h.begin_step(true).unwrap();
        h.finish_step(s, true);
        assert!(!h.is_dirty_past(inert), "the file is one marker ahead");
        h.forget();
        assert!(h.is_dirty_past(inert), "unknown is always dirty");
    }

    #[test]
    fn record_inert_keeps_the_redo_branch_and_shifts_a_point_on_it() {
        let mut h = history();
        h.record(2);
        h.mark_saved(); // saved after the real step
        let s = h.begin_step(true).unwrap();
        h.finish_step(s, true);
        assert_eq!(h.redo_len(), 1);

        h.record_inert(1);

        assert_eq!((h.undo_len(), h.redo_len()), (1, 1), "redo kept");
        assert!(h.is_dirty_past(inert), "the real step is still ahead");
        let s = h.begin_step(false).unwrap();
        h.finish_step(s, false);
        assert!(!h.is_dirty(), "redo lands exactly on the saved position");
    }

    #[test]
    fn record_inert_past_the_cap_evicts_the_oldest_and_keeps_counting() {
        let mut h = history();
        for step in [2, 4, 6] {
            h.record(step);
        }
        assert_eq!(h.record_inert(1), Some(2));
        assert_eq!(h.position(), 4);
        assert_eq!(h.undo_len(), CAP);
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
    fn stepping_back_caps_the_redo_stack_at_its_far_end() {
        let mut h = history();
        h.undo_steps_mut().push(0);
        h.redo_steps_mut().extend([10, 11, 12]);
        assert_eq!(h.step(99, true), Some(0));
        assert_eq!(h.redo_len(), CAP, "the redo stack stays within its cap");
        assert_eq!(h.redo_steps(), &[11, 12, 99], "the far end is dropped");
    }

    #[test]
    fn a_step_forgets_a_point_the_stacks_no_longer_reach() {
        let mut h = history();
        for i in 0..3 {
            h.record(i);
        }
        h.mark_saved(); // depth 3
        h.undo_steps_mut().truncate(1); // a hand rewrite lost the point
        h.step(9, true); // reach is now 1
        h.redo_steps_mut().extend([1, 2]);
        for i in 0..3 {
            h.step(i, false);
        }
        assert_eq!(h.undo_len(), 3);
        assert!(h.is_dirty(), "depth 3 is not the saved state any more");
    }

    #[test]
    fn an_edit_from_one_step_below_the_point_cuts_it() {
        let mut h = history();
        h.record(0);
        h.record(1);
        h.mark_saved(); // depth 2
        h.step(2, true); // depth 1, the point one step ahead
        h.record(9);
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

    #[test]
    fn position_counts_past_eviction_and_undo_lowers_it() {
        let mut h = history();
        for i in 0..CAP + 4 {
            h.record(i as u32);
            assert_eq!(h.position(), i + 1);
        }
        assert_eq!(h.undo_len(), CAP);
        h.step(99, true);
        assert_eq!(h.position(), CAP + 3);
        h.step(98, false);
        assert_eq!(h.position(), CAP + 4);
    }

    #[test]
    fn discard_top_drops_the_undone_step_and_the_point_on_it() {
        let mut h = history();
        h.record(0);
        h.record(1);
        h.mark_saved(); // depth 2
        h.step(2, true);
        assert_eq!(h.discard_top(), Some(2));
        assert!(!h.can_redo());
        assert!(h.is_dirty(), "the point sat on the discarded state");
        assert_eq!(h.discard_top(), None);
        h.record(7); // lands at the depth the point named: a different state
        assert!(h.is_dirty(), "the discarded state cannot come back");
    }

    #[test]
    fn a_point_on_the_oldest_kept_state_survives_eviction() {
        let mut h = History::at_start(2);
        h.record(0);
        h.record(1);
        h.mark_saved(); // depth 2
        h.record(2);
        h.record(3); // evicts two: the floor equals the point
        h.step(4, true);
        h.step(3, true);
        assert!(!h.is_dirty(), "depth 2 is the bottom of the stack");
    }

    #[test]
    fn discard_top_keeps_a_point_that_is_not_on_the_discarded_state() {
        let mut h = history();
        h.record(0);
        h.mark_saved(); // depth 1
        h.record(1);
        h.step(2, true); // depth 1 again
        h.record(5);
        h.step(6, true);
        assert_eq!(h.discard_top(), Some(6));
        assert!(!h.is_dirty(), "depth 1 is still the saved state");
    }

    #[test]
    fn lowering_the_cap_trims_the_oldest_and_keeps_a_reachable_point() {
        let mut h = History::at_start(10);
        for i in 0..6 {
            h.record(i);
        }
        h.mark_saved(); // depth 6
        h.record(6);
        h.set_cap(3);
        assert_eq!(h.undo_steps(), &[4, 5, 6], "the newest steps stay");
        assert_eq!(h.position(), 7, "the position does not move");
        h.step(7, true);
        assert!(!h.is_dirty(), "the saved depth is still reachable");
    }

    #[test]
    fn lowering_the_cap_past_the_point_leaves_it_dirty_for_good() {
        let mut h = History::at_start(10);
        h.record(0);
        h.mark_saved(); // depth 1
        for i in 1..6 {
            h.record(i);
        }
        h.set_cap(2); // evicts depths 0..4: the point is gone
        assert!(h.is_dirty());
        while h.can_undo() {
            h.step(0, true);
            assert!(h.is_dirty(), "undo cannot reach a trimmed point");
        }
        h.mark_saved();
        assert!(!h.is_dirty(), "a save re-anchors it");
    }

    #[test]
    fn lowering_the_cap_trims_the_far_end_of_the_redo_stack() {
        let mut h = History::at_start(10);
        for i in 0..5 {
            h.record(i);
        }
        for i in 0..5 {
            h.step(i, true);
        }
        h.set_cap(2);
        assert_eq!(h.redo_len(), 2);
        assert_eq!(h.undo_len(), 0);
    }

    #[test]
    fn depth_one_keeps_exactly_one_step_and_zero_is_one() {
        let mut h = History::<u32>::at_start(1);
        h.record(0);
        h.record(1);
        assert_eq!(h.undo_len(), 1);
        assert_eq!(History::<u32>::at_start(0).cap(), 1);
        h.set_cap(0);
        assert_eq!(h.cap(), 1);
    }

    #[test]
    fn the_depth_range_is_one_to_a_thousand() {
        assert!(!undo_depth_in_range(0));
        assert!(undo_depth_in_range(1));
        assert!(undo_depth_in_range(1000));
        assert!(!undo_depth_in_range(1001));
        assert_eq!((MIN_UNDO_DEPTH, MAX_UNDO_DEPTH), (1, 1000));
    }
}
