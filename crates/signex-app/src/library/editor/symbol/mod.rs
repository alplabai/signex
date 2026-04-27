//! Symbol editor module — submodule re-exports only.
//!
//! v0.9-refactor-2 (DBLib model): the in-Component Editor symbol tab
//! is gone. Symbol editing happens via the standalone `.snxsym`
//! document editor (WS-7), which lives in
//! [`crate::library::editor::standalone`] and re-uses [`canvas`] /
//! [`state`] verbatim.

pub mod ai_stub;
pub mod canvas;
pub mod state;

#[cfg(test)]
mod tests;
