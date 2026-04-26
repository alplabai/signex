//! `signex-app`'s library subsystem (v0.9 Phase 1).
//!
//! This module wires the [`signex_library`] crate (data + adapters)
//! into the iced UI. It owns:
//!
//! * the **left-dock Library panel** — open libraries, search, drill
//!   into components (see `panel.rs`);
//! * the **Place Component picker modal** (`picker.rs`);
//! * the **Component Editor** (`editor/`) — multi-window tabbed
//!   surface with 9 tabs (Overview / Symbol / Footprint / 3D /
//!   Params / Supply / Sim / History / Where-Used);
//! * the **Distributor APIs settings** panel
//!   (`settings/distributor_apis.rs`).
//!
//! Phase 1 fills Overview / Params / Supply / History / Where-Used;
//! Symbol / Footprint / 3D / Sim render placeholder cards with
//! TODO(v0.9-phase-2) markers. See LIBRARY_PLAN §10–§11 for the
//! full scope.

pub mod close_prompt;
pub mod commands;
pub mod editor;
pub mod messages;
pub mod new_component;
pub mod panel;
pub mod picker;
pub mod settings;
pub mod state;

// Re-export the heavily-used types so callers don't have to know
// the internal module layout. `LibraryMessage` is folded into the
// top-level Message; `LibraryState` holds the subsystem's state on
// `Signex`. The remaining sub-types are used widely enough across
// the app crate to warrant a flat re-export, even where the only
// current consumer is the dispatcher (Phase 2 widens the surface).
#[allow(unused_imports)]
pub use messages::{EditorMsg, LibraryMessage, PickerMsg, SettingsMsg};
#[allow(unused_imports)]
pub use state::{ComponentEditorState, EditorTab, LibraryState, OpenLibrary, PickerState};
