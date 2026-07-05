//! Symbol editor module — submodule re-exports only.
//!
//! Symbol editing happens via the standalone `.snxsym` document
//! editor in [`crate::library::editor::standalone`], which re-uses
//! [`canvas`] / [`state`] verbatim.

pub mod active_bar;
pub mod active_bar_dropdowns;
pub mod ai_stub;
pub mod apply;
pub mod canvas;
// Groundwork for the shader/scene renderer port (not yet wired into
// the live canvas — see snapshot.rs module docs).
pub mod snapshot;
pub mod state;

#[cfg(test)]
mod tests;
