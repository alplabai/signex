//! An asked-for branch switch: plain state, UI-free (ADR-0006).

use std::path::PathBuf;

use signex_library::repo_branches::BranchTarget;

use crate::app::core::project_id::ProjectId;

/// The surface a switch was asked from, which hears why it did not happen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwitchSurface {
    ProjectTab(ProjectId),
    History,
}

/// One asked-for branch switch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwitchRequest {
    pub surface: SwitchSurface,
    pub root: PathBuf,
    pub target: BranchTarget,
}
