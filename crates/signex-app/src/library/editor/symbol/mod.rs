//! Symbol editor module — submodule re-exports only.
//!
//! Symbol editing happens via the standalone `.snxsym` document
//! editor in [`crate::library::editor::standalone`], which re-uses
//! [`canvas`] / [`state`] verbatim.

pub mod active_bar;
pub mod active_bar_dropdowns;
pub mod ai_stub;
pub mod canvas;
pub mod state;
pub(crate) mod updates;

#[cfg(test)]
mod tests;
