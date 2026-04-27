//! Sim tab state — WS-L.
//!
//! All persistent state for the Sim tab now lives on the typed
//! [`signex_library::SimModel`] primitive bound through
//! `Revision::sim_ref`. The only piece of UI-only state — the live
//! `text_editor::Content` mirror of the SPICE deck — sits on
//! [`crate::library::state::ComponentEditorState::sim_body`] so it
//! shares the per-editor lifetime that the rest of the tab tooling
//! uses.
//!
//! This module stays as a no-op surface: dropping the file would
//! cascade into `pub mod state;` removals across the editor module
//! tree; the empty `SimTabState` keeps the public surface stable
//! for any sibling that imports it.

/// Placeholder — superseded by `ComponentEditorState::sim_body` and
/// the SimModel primitive itself. Kept for backwards-compatible
/// imports until WS-? deletes the module.
#[derive(Debug, Default, Clone)]
#[allow(dead_code)]
pub struct SimTabState;
