//! A saved sheet's engine, held only so its undo group can still step it.

use crate::app::core::project_id::ProjectId;

/// A saved sheet's engine, kept for the undo group that still has a step in it.
#[derive(Debug)]
pub(crate) struct HeldEngine {
    pub(crate) engine: signex_engine::Engine,
    /// The owning project: closing it ends the undo.
    pub(crate) project: Option<ProjectId>,
}
