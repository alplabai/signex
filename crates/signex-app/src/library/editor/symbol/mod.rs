//! Symbol editor module — submodule re-exports only.
//!
//! Symbol editing happens via the standalone `.snxsym` document
//! editor in [`crate::library::editor::standalone`], which re-uses
//! [`canvas`] / [`state`] verbatim.

pub mod active_bar;
pub mod canvas;
pub mod context_menu;
pub mod pins_panel;
pub mod state;
pub(crate) mod status;
pub(crate) mod updates;

/// Fit the symbol editor camera to a viewport; the document layer calls it
/// when it opens a symbol.
pub(crate) use updates::fit_to_viewport;
