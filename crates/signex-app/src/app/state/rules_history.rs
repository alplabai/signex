//! Undo / redo of a project's stored rule set (#1085).
//!
//! The rule set is a few hundred bytes, so each step keeps the whole prior
//! `Option<ProjectRules>` rather than an inverse command: `None` is "the
//! project stores no rules", which makes the store action undoable too.
//! The steps live in the shared snapshot [`History`] (depth capped).

use signex_types::project::ProjectRules;

use crate::app::History;

/// Depth cap of the undo and redo stacks; a few hundred bytes a step.
const HISTORY_DEPTH: usize = signex_history::DEFAULT_UNDO_DEPTH;

/// One project's rule-edit history.
///
/// Its save point is never read: the `.snxprj` is clean when it encodes to
/// the bytes on disk (`state/dirty/project_history.rs`), so no `is_dirty`
/// is exposed here.
#[derive(Debug)]
pub struct RulesHistory(History<Option<ProjectRules>>);

impl Default for RulesHistory {
    fn default() -> Self {
        Self(History::at_start(HISTORY_DEPTH))
    }
}

impl RulesHistory {
    /// Keep `depth` undo steps from now on; lowering it drops the oldest.
    pub fn set_undo_depth(&mut self, depth: usize) {
        self.0.set_cap(depth);
    }

    /// How many undo steps are kept.
    pub fn undo_depth(&self) -> usize {
        self.0.cap()
    }

    /// Remember `before` as the state to return to; a new edit drops redo.
    pub fn record(&mut self, before: Option<ProjectRules>) {
        self.0.record(before);
    }

    /// Step back from `current`; returns the state to restore.
    pub fn undo(&mut self, current: Option<ProjectRules>) -> Option<Option<ProjectRules>> {
        self.0.step(current, true)
    }

    /// Step forward from `current`; returns the state to restore.
    pub fn redo(&mut self, current: Option<ProjectRules>) -> Option<Option<ProjectRules>> {
        self.0.step(current, false)
    }

    pub fn can_undo(&self) -> bool {
        self.0.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.0.can_redo()
    }

    #[cfg(test)]
    pub(crate) fn undo_len(&self) -> usize {
        self.0.undo_len()
    }
}
