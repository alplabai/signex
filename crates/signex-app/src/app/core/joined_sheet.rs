//! A sheet an edit brought into the running agent step (#2501): plain data
//! kept in `AgentEdits` until the step commits or rolls back.

use std::path::PathBuf;

/// A sheet an edit brought into the running agent step (#2501).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct JoinedSheet {
    pub(crate) path: PathBuf,
    /// No engine held it: it was loaded, tabless, for this step.
    pub(crate) tabless: bool,
    /// It had no unsaved edits when it joined.
    pub(crate) clean_before: bool,
}
