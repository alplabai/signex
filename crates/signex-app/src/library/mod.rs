//! `signex-app`'s library subsystem (DBLib model).
//!
//! Wires the [`signex_library`] crate (data + adapters) into the iced
//! UI. Components are TSV rows under `<lib>/tables/<category>.tsv`,
//! addressed by [`crate::library::state::EditorAddress`]. This module
//! owns:
//!
//! * the **left-dock Library panel** (`panel.rs`) — a flat list of
//!   open libraries, each opening its Library Browser tab;
//! * the **Library Browser** (`browser/`) and the **Place Component
//!   picker modal** (`picker.rs`, `place.rs`);
//! * the **Component Preview tab** (`component_preview/`, `editor/`) —
//!   Symbol + Footprint render plus Preview / Parameters / Supply /
//!   Datasheet / Simulation tabs; primitive editing happens in the
//!   standalone `.snxsym` / `.snxfpt` document tabs;
//! * **mounting** (`mount.rs`, off-thread), **drift / update**
//!   (`drift.rs`, `updates_dialog.rs`), **dangling-reference** checks
//!   (`dangling.rs`), **pricing** (`pricing.rs`, `bom_prices.rs`),
//!   external-change polling (`external_poll.rs`) and **recovery**
//!   (`recovery.rs`);
//! * the **Distributor APIs settings** panel
//!   (`settings/distributor_apis.rs`).

pub mod bom_prices;
pub mod browser;
pub mod close_prompt;
pub mod commands;
pub mod component_preview;
pub mod create_options;
pub mod dangling;
pub mod document_options;
pub mod drift;
pub mod edit_row_modal;
mod edit_row_params;
pub mod editor;
pub mod external_poll;
pub mod held_back;
pub mod messages;
pub(crate) mod modal_chrome;
pub mod mount;
pub mod panel;
pub mod part_label;
pub mod picker;
pub mod place;
pub mod placed_part;
pub(crate) mod prefix_picker;
pub mod preview_reads;
#[cfg(test)]
mod preview_reads_tests;
pub(crate) mod preview_rebase;
pub mod preview_refresh;
#[cfg(test)]
mod preview_refresh_tests;
pub mod pricing;
pub mod primitive_picker;
pub mod range_filter;
pub mod recovery;
pub mod refresh;
pub mod reprefix_lookup;
pub mod resolve;
pub mod row_documents;
pub mod row_summaries;
pub mod settings;
pub mod shared_resolver;
pub mod state;
pub mod stock_copy;
#[cfg(test)]
pub(crate) mod test_adapters;
pub(crate) mod unit_picker;
pub mod updates_dialog;
#[cfg(test)]
mod view_purity_tests;
pub mod writes;

// Re-export the heavily-used types so callers don't have to know
// the internal module layout. `LibraryMessage` is folded into the
// top-level Message; `LibraryState` holds the subsystem's state on
// `Signex`. The remaining sub-types are used widely enough across
// the app crate to warrant a flat re-export, even where the only
// current consumer is the dispatcher (Phase 2 widens the surface).
pub use messages::{
    BrowserEditMsg, EditorMsg, LibraryMessage, PickerMsg, PlacedPartAction, PlacedPartMsg,
    PrimitivePickerMsg, SettingsMsg,
};
pub use state::{
    ComponentPreviewState, ComponentsMountSource, ComponentsPanelState, DeleteConfirmState,
    EditRowModalState, LibraryBrowserState, LibraryState, NewLibraryDefaults, OpenLibrary,
    PickerState, PreviewTab, PrimitivePickerState, PrimitivePickerTarget,
};

/// A dialog card's length: `px` in the main window, the whole window in the
/// dialog's own window (the window is the frame; docs/DETACHED_MODALS.md).
pub(crate) fn card_len(windowed: bool, px: f32) -> iced::Length {
    if windowed {
        iced::Length::Fill
    } else {
        iced::Length::Fixed(px)
    }
}
