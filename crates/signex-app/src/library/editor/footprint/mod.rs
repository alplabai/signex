//! Footprint editor module — submodule re-exports only.
//!
//! v0.9-refactor-2 (DBLib model): the in-Component Editor footprint
//! tab is gone. Footprint editing happens via the standalone
//! `.snxfpt` document editor (WS-7), which lives in
//! [`crate::library::editor::standalone`] and re-uses
//! [`canvas`] / [`layers`] / [`state`] verbatim.
//!
//! `body3d`, `preview3d`, and `step_attach` remain on disk for the
//! eventual standalone Body 3D / STEP attach editor side-pane.

pub mod body3d;
pub mod canvas;
pub mod layers;
pub mod preview3d;
pub mod state;
pub mod step_attach;

#[cfg(test)]
mod tests;
