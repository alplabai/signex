//! Symbol editor module — submodule re-exports only.
//!
//! Symbol editing happens via the standalone `.snxsym` document
//! editor in [`crate::library::editor::standalone`], which re-uses
//! [`canvas`] / [`state`] verbatim.

pub mod active_bar;
pub mod canvas;
pub mod context_menu;
#[cfg(test)]
mod resize_tests;
pub mod state;
pub(crate) mod status;
pub(crate) mod updates;

/// Fit the symbol editor camera to a viewport; the document layer calls it
/// when it opens a symbol.
pub(crate) use updates::fit_to_viewport;

/// The Properties panel's state and messages for a symbol editor (mockup
/// rows 122 and 123), and the rectangle Width / Height rule its form and the graphic
/// edit share: the slice's surface for the panel and the dock handler.
pub use state::{
    PartChip, PartStep, SymbolPropsMsg, SymbolPropsTab, SymbolPropsUi, rect_end_for_extent,
    rect_extent,
};
