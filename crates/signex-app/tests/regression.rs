//! Regression tests for v0.10–v0.12 walkthrough findings, split by
//! subsystem (see #432 — the flat 6,475-line file grew append-site
//! conflicts on every parallel branch and had no answer to "where does
//! my test go").
//!
//! These exercise dispatchers without spinning up the iced runtime —
//! `Signex::new()` constructs the app, the test populates state
//! directly via the `pub` fields on `DocumentState` / `UiState`, then
//! `Signex::update(Message::*)` routes through the same handler the
//! UI would. State changes (file system effects, `dirty_paths`,
//! tree state, etc.) are observed afterwards.
//!
//! Closes the manual-walkthrough gap for items where the only
//! genuine UI dependency is the `rfd::AsyncFileDialog` picker — those
//! still need a human eye.
//!
//! One test binary: `signex-app` links `iced`/`wgpu`, so these stay
//! `mod`-included here rather than becoming separate
//! `tests/regression_*.rs` targets (each would be its own link step).
//! Each module below carries its own helpers and `use` lines — no
//! shared `support` module, since no helper is called from more than
//! one module (confirmed while doing the split).

// This integration-test binary discards fallible test-setup calls with
// `let _ = ...` routinely (not a production `Task` getting dropped — see
// GH #99 part 1 / crates/signex-app/src/lib.rs's crate-level suppression
// for the equivalent in-src-test-module allow). `[lints]` in Cargo.toml
// is package-scoped, not target-scoped, so each integration-test crate
// root needs its own allow.
#![expect(
    clippy::let_underscore_must_use,
    reason = "integration tests discard Results with `let _ = ...` routinely; this is test scaffolding, not a production `iced::Task` being dropped, and `[lints]` in Cargo.toml is package-scoped so each integration-test crate root needs its own"
)]

#[path = "regression/project.rs"]
mod project;

#[path = "regression/prefs.rs"]
mod prefs;

#[path = "regression/preferences_dirty_guard.rs"]
mod preferences_dirty_guard;

#[path = "regression/preferences_export_status.rs"]
mod preferences_export_status;

#[path = "regression/preferences_prefs_recovery.rs"]
mod preferences_prefs_recovery;

#[path = "regression/undo_marker_divergence.rs"]
mod undo_marker_divergence;

#[path = "regression/dirty_paths_gateway.rs"]
mod dirty_paths_gateway;

#[path = "regression/library_undo.rs"]
mod library_undo;

#[path = "regression/library_placement.rs"]
mod library_placement;

#[path = "regression/library_pad_geometry.rs"]
mod library_pad_geometry;

#[path = "regression/library_cross_track.rs"]
mod library_cross_track;

#[path = "regression/library_pad_actions.rs"]
mod library_pad_actions;

#[path = "regression/library_bga.rs"]
mod library_bga;

#[path = "regression/sketch_state.rs"]
mod sketch_state;

#[path = "regression/file_open_async.rs"]
mod file_open_async;
