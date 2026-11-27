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
pub(crate) use updates::{
    apply_symbol_primitive_edit, new_unstorable_pin_text, pin_text_refused, refuse_pin_text,
    refuse_read_only,
};

/// The Properties panel's state and messages for a symbol editor (mockup
/// rows 122 and 123), and the rectangle Width / Height rule its form and the graphic
/// edit share: the slice's surface for the panel and the dock handler.
pub use state::{
    BodyScope, BodyScopeRow, PartChip, PartStep, SymActiveBarMenu, SymbolPropsMsg, SymbolPropsTab,
    SymbolPropsUi, SymbolSelection, rect_end_for_extent, rect_extent,
};
/// The Pin table document tab's state, messages and edits (mockup row 122b).
pub use state::{
    PasteDraft, PasteField, PasteMode, PinTableMsg, PinTableUi, add_table_pin, apply_bulk,
    apply_paste, parse_mm, paste_preview, remove_pins,
};
