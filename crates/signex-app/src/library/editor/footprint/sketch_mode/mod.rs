//! Sketch-mode tooling for the footprint editor.
//!
//! Phase 5.5 + 6 of the v0.13 sketch-mode plan. The dispatcher in
//! [`crate::library::editor::footprint::sketch_dispatch`] consumes
//! [`SketchEdit`] values; the UI surface (Phase 6) routes
//! [`SketchModeMsg`] from iced through this module to the
//! dispatcher.
//!
//! v0.13 ships the `messages` and `tools` modules; the inspector +
//! overlay + DOF render layer land in Phase 6 follow-ups.

pub mod active_bar;
pub mod inspector;
pub mod messages;

pub use messages::{ActiveTool, SketchEdit, SketchModeMsg, ToolEvent};
