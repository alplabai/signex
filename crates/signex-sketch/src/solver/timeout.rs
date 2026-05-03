//! Auto-pause hysteresis for the live solver.
//!
//! The UI runs `Solver::solve(...)` on every sketch edit so geometry
//! updates feel direct. If the solver consistently misses its
//! per-edit budget (e.g. the user has constructed a very large
//! sketch), continuing to attempt live solves will burn cycles and
//! starve UI repaint. This module models a small hysteresis state
//! that pauses live-solve after `consecutive_overruns` overruns and
//! requires explicit `unpause()` to resume — typical UI behaviour
//! is to surface a "Solver paused (sketch too large)" toast and
//! resume on the next manual edit-finalise.
//!
//! The threshold is `2` consecutive overruns by default — single
//! glitches don't pause; sustained budget-misses do.

/// Number of consecutive budget-overruns that must occur before
/// `paused` flips to `true`. Tuned to ignore single glitches caused
/// by GC pauses, OS scheduling, etc., while still pausing quickly
/// on a genuinely overweight sketch.
pub const PAUSE_THRESHOLD: u32 = 2;

/// Hysteresis state for live-solve auto-pause.
#[derive(Clone, Debug, Default)]
pub struct AutoPauseState {
    consecutive_overruns: u32,
    paused: bool,
}

impl AutoPauseState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Observe a single solve attempt's wall-clock duration. If
    /// `elapsed_ms > budget_ms` the overrun counter increments;
    /// after [`PAUSE_THRESHOLD`] consecutive overruns the state
    /// flips to paused. A successful in-budget observation resets
    /// the counter.
    pub fn observe(&mut self, elapsed_ms: u64, budget_ms: u64) {
        if elapsed_ms > budget_ms {
            self.consecutive_overruns += 1;
            if self.consecutive_overruns >= PAUSE_THRESHOLD {
                self.paused = true;
            }
        } else {
            self.consecutive_overruns = 0;
        }
    }

    /// Whether live-solve is currently auto-paused.
    pub fn paused(&self) -> bool {
        self.paused
    }

    /// Manually resume live-solve. Resets the overrun counter so
    /// the threshold has to be hit fresh from zero.
    pub fn unpause(&mut self) {
        self.paused = false;
        self.consecutive_overruns = 0;
    }

    /// Current consecutive-overrun count. Mostly for debugging /
    /// status-bar display.
    pub fn consecutive_overruns(&self) -> u32 {
        self.consecutive_overruns
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_overrun_does_not_pause() {
        let mut s = AutoPauseState::new();
        s.observe(60, 50);
        assert!(!s.paused());
        assert_eq!(s.consecutive_overruns(), 1);
    }

    #[test]
    fn two_consecutive_overruns_pause() {
        let mut s = AutoPauseState::new();
        s.observe(60, 50);
        s.observe(70, 50);
        assert!(s.paused());
    }

    #[test]
    fn good_observation_resets_counter() {
        let mut s = AutoPauseState::new();
        s.observe(60, 50); // overrun, count=1
        s.observe(40, 50); // good, reset
        s.observe(60, 50); // overrun again, count=1
        assert!(!s.paused());
    }

    #[test]
    fn unpause_clears_state() {
        let mut s = AutoPauseState::new();
        s.observe(60, 50);
        s.observe(60, 50);
        assert!(s.paused());
        s.unpause();
        assert!(!s.paused());
        assert_eq!(s.consecutive_overruns(), 0);
    }

    #[test]
    fn equal_to_budget_is_not_an_overrun() {
        let mut s = AutoPauseState::new();
        s.observe(50, 50);
        s.observe(50, 50);
        assert!(!s.paused());
    }
}
