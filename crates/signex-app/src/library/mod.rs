//! `signex-app`'s library subsystem.
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
//! WS-F refactor: Symbol + Footprint tabs now operate on typed
//! `Symbol` / `Footprint` primitives loaded by `PrimitiveRef`. The
//! Footprint tab also hosts the new Body3D editor + procedural 3D
//! preview + STEP attachment pane. The Overview / Params / Supply /
//! Sim / 3D / History tabs render WS-E-pending placeholders until
//! WS-E lands the binding-record rewire.

// WS-F: most legacy view modules and message variants are stubbed
// pending WS-E. Each stubbed module carries a `TODO(merge-with-WS-E)`
// marker pointing at the rebuild.

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
