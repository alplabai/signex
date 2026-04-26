//! Sim tab state — WS-E PENDING.
//!
//! WS-B replaced the inline `SpiceModel` with the typed `SimModel`
//! primitive bound through `Revision::sim_ref`. WS-E rebuilds this
//! state struct; until then the module exposes empty types so the
//! editor can compile without dragging the pre-refactor `text_editor`
//! plumbing along.

/// Empty placeholder — every field landed on `SimModel` in v0.9.
#[derive(Debug, Default, Clone)]
pub struct SimTabState;
