//! Undo / redo of a project's stored rule set (#1085).
//!
//! The rule set is a few hundred bytes, so each step keeps the whole prior
//! [`RuleState`] rather than an inverse command: `rules: None` is "the
//! project stores no rules", which makes the store action undoable too.
//! The steps live in the shared snapshot [`History`] (depth capped).

use signex_history::History;

pub use crate::app::core::rule_state::RuleState;

/// Depth cap of the undo and redo stacks; a few hundred bytes a step.
const HISTORY_DEPTH: usize = signex_history::DEFAULT_UNDO_DEPTH;

/// One project's rule-edit history.
///
/// Its save point is never read: the `.snxprj` is clean when it encodes to
/// the bytes on disk (`state/dirty/project_history.rs`), so no `is_dirty`
/// is exposed here.
#[derive(Debug)]
pub struct RulesHistory {
    steps: History<RuleState>,
    /// Per undo step, oldest first: the project's General history position
    /// when the step was recorded, which orders the two histories.
    undo_marks: Vec<usize>,
    /// The same for the redo steps, the next redo last.
    redo_marks: Vec<usize>,
}

impl Default for RulesHistory {
    fn default() -> Self {
        Self {
            steps: History::at_start(HISTORY_DEPTH),
            undo_marks: Vec::new(),
            redo_marks: Vec::new(),
        }
    }
}

impl RulesHistory {
    /// Keep `depth` undo steps from now on; lowering it drops the oldest.
    pub fn set_undo_depth(&mut self, depth: usize) {
        self.steps.set_cap(depth);
        self.trim_marks();
    }

    /// How many undo steps are kept.
    pub fn undo_depth(&self) -> usize {
        self.steps.cap()
    }

    /// Remember `before` as the state to return to, made with the General
    /// history at `general_position`; a new edit drops redo.
    pub fn record_at(&mut self, before: impl Into<RuleState>, general_position: usize) {
        self.steps.record(before.into());
        self.undo_marks.push(general_position);
        self.redo_marks.clear();
        self.trim_marks();
    }

    /// [`Self::record_at`] with the General history at its start.
    #[cfg(test)]
    pub fn record(&mut self, before: impl Into<RuleState>) {
        self.record_at(before, 0);
    }

    /// Step back from `current`; returns the state to restore.
    pub fn undo(&mut self, current: RuleState) -> Option<RuleState> {
        let restored = self.steps.step(current, true)?;
        self.redo_marks.extend(self.undo_marks.pop());
        self.trim_marks();
        Some(restored)
    }

    /// Step forward from `current`; returns the state to restore.
    pub fn redo(&mut self, current: RuleState) -> Option<RuleState> {
        let restored = self.steps.step(current, false)?;
        self.undo_marks.extend(self.redo_marks.pop());
        self.trim_marks();
        Some(restored)
    }

    pub fn can_undo(&self) -> bool {
        self.steps.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.steps.can_redo()
    }

    /// Whether this history's next step (`undo`, else redo) comes before the
    /// General history's, which stands at `general_position`.
    pub fn steps_before_general(&self, undo: bool, general_position: usize) -> bool {
        if undo {
            self.undo_marks
                .last()
                .is_some_and(|mark| *mark >= general_position)
        } else {
            self.redo_marks
                .last()
                .is_some_and(|mark| *mark <= general_position)
        }
    }

    /// Drop the marks of steps the depth cap evicted.
    fn trim_marks(&mut self) {
        let undo = self.undo_marks.len().saturating_sub(self.steps.undo_len());
        self.undo_marks.drain(..undo);
        let redo = self.redo_marks.len().saturating_sub(self.steps.redo_len());
        self.redo_marks.drain(..redo);
    }

    #[cfg(test)]
    pub(crate) fn undo_len(&self) -> usize {
        self.steps.undo_len()
    }
}

#[cfg(test)]
mod order_tests {
    use super::*;

    fn state() -> RuleState {
        RuleState {
            rules: None,
            router_tool_diameter: None,
        }
    }

    /// General step, rule step, General step: undo and redo walk them back
    /// and forth in the order they were made.
    #[test]
    fn a_rule_step_keeps_its_place_between_general_steps() {
        let mut history = RulesHistory::default();
        history.record_at(state(), 1);
        assert!(!history.steps_before_general(true, 2), "the later edit");
        assert!(history.steps_before_general(true, 1), "then the rule step");
        history.undo(state()).expect("a step");
        assert!(!history.steps_before_general(true, 1), "none left");
        assert!(!history.steps_before_general(false, 0), "General redo first");
        assert!(history.steps_before_general(false, 1), "then the rule step");
        history.redo(state()).expect("a step");
        assert!(!history.steps_before_general(false, 1), "none left");
    }

    #[test]
    fn the_marks_follow_the_depth_cap() {
        let mut history = RulesHistory::default();
        history.record_at(state(), 0);
        history.record_at(state(), 5);
        history.set_undo_depth(1);
        history.undo(state()).expect("the newest step");
        assert!(history.undo(state()).is_none());
        assert!(!history.steps_before_general(true, 0), "its mark went too");
    }
}
