//! `signex-app`'s library subsystem (DBLib model — v0.9-refactor-2).
//!
//! Wires the [`signex_library`] crate (data + adapters) into the iced
//! UI. Components are TSV rows under `<lib>/tables/<category>.tsv`,
//! addressed by [`crate::library::state::EditorAddress`]. This module
//! owns:
//!
//! * the **left-dock Library panel** — open libraries with inline
//!   per-table row grids (see `panel.rs`);
//! * the **Place Component picker modal** (`picker.rs`);
//! * the **Component Preview tab** (`editor/`) — read-only Symbol +
//!   Footprint render plus 5 tabs (Preview / Parameters / Supply /
//!   Datasheet / Simulation). Symbol / Footprint editing happens via
//!   the standalone `.snxsym` / `.snxfpt` document tabs;
//! * the **Distributor APIs settings** panel
//!   (`settings/distributor_apis.rs`).

pub mod browser;
pub mod close_prompt;
pub mod commands;
pub mod component_preview;
pub mod create_options;
pub mod document_options;
pub mod edit_row_modal;
pub mod editor;
pub mod messages;
pub mod new_component;
pub mod panel;
pub mod picker;
pub mod primitive_picker;
pub mod recovery;
pub mod settings;
pub mod state;
pub mod updates_dialog;

// Re-export the heavily-used types so callers don't have to know
// the internal module layout. `LibraryMessage` is folded into the
// top-level Message; `LibraryState` holds the subsystem's state on
// `Signex`. The remaining sub-types are used widely enough across
// the app crate to warrant a flat re-export, even where the only
// current consumer is the dispatcher (Phase 2 widens the surface).
#[allow(unused_imports)]
pub use messages::{
    BrowserEditMsg, EditorMsg, LibraryMessage, PickerMsg, PrimitivePickerMsg, SettingsMsg,
};
#[allow(unused_imports)]
pub use state::{
    ComponentEditorState, ComponentPreviewState, ComponentsMountSource, ComponentsPanelState,
    DeleteConfirmState, EditRowModalState, LibraryBrowserState, LibraryState, OpenLibrary,
    PickerState, PreviewTab, PrimitivePickerState, PrimitivePickerTarget,
};
