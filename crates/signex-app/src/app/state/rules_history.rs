//! Undo / redo of a project's stored rule set (#1085).
//!
//! The rule set is a few hundred bytes, so each step keeps the whole prior
//! `Option<ProjectRules>` rather than an inverse command: `None` is "the
//! project stored no rules", which makes the store action undoable too.

use signex_types::project::ProjectRules;

/// One project's rule-edit history.
#[derive(Debug, Default, Clone)]
pub struct RulesHistory {
    undo: Vec<Option<ProjectRules>>,
    redo: Vec<Option<ProjectRules>>,
}

impl RulesHistory {
    /// Remember `before` as the state to return to; a new edit drops redo.
    pub fn record(&mut self, before: Option<ProjectRules>) {
        self.undo.push(before);
        self.redo.clear();
    }

    /// Step back from `current`; returns the state to restore.
    pub fn undo(&mut self, current: Option<ProjectRules>) -> Option<Option<ProjectRules>> {
        let previous = self.undo.pop()?;
        self.redo.push(current);
        Some(previous)
    }

    /// Step forward from `current`; returns the state to restore.
    pub fn redo(&mut self, current: Option<ProjectRules>) -> Option<Option<ProjectRules>> {
        let next = self.redo.pop()?;
        self.undo.push(current);
        Some(next)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}
