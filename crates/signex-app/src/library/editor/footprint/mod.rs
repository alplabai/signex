//! Footprint editor module — submodule re-exports only.
//!
//! Footprint editing happens via the standalone `.snxfpt` document
//! editor in [`crate::library::editor::standalone`], which re-uses
//! [`canvas`] / [`layers`] / [`state`] verbatim.
//!
//! `body3d`, `preview3d`, and `step_attach` remain on disk for the
//! eventual standalone Body 3D / STEP attach editor side-pane.

pub mod body3d;
pub mod canvas;
pub mod layers;
pub mod pad_to_sketch;
pub mod pads_active_bar;
pub mod preview3d;
pub mod sketch_dispatch;
#[cfg(test)]
mod sketch_dispatch_tests;
pub mod sketch_mode;
pub mod snap;
pub mod state;
pub mod step_attach;
pub mod unified_active_bar;

#[cfg(test)]
mod tests;
