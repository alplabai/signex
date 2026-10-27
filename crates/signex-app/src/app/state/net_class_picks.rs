//! Undo order of net-class picks against a sheet's own edits (#1220).
//!
//! A pick changes the project's `.snxprj` class table, not the sheet, so the
//! sheet's engine history never sees it. Each pick is stamped with the engine's
//! edit-timeline position of the sheet it was made from (as net-colour steps
//! are), which lets Ctrl+Z interleave picks and sheet edits chronologically.
//! The table itself is restored through the project's `NetClassHistory`.

use super::ProjectId;

/// One pick: which project's table it changed, when, and the identity of its
/// step in that project's class history (to tell the pick is still the newest
/// class edit; depth cannot, since a capped or undone history revisits it).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NetClassPickStep {
    pub project: ProjectId,
    pub at: usize,
    pub step: signex_engine::NetClassStepId,
}

/// A sheet's pick stacks.
#[derive(Debug, Clone, Default)]
pub struct NetClassPickSteps {
    pub undo: Vec<NetClassPickStep>,
    pub redo: Vec<NetClassPickStep>,
}
