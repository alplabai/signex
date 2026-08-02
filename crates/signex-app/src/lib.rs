//! Signex EDA — library face of the application binary.
//!
//! `main.rs` is a thin wrapper that calls into this crate; the real
//! contents live here so that integration tests in `tests/` can
//! `use signex_app::*` and exercise dispatchers without spinning up
//! the iced runtime.
//!
//! All modules are `pub` for test access. The published surface is
//! deliberately wide because tests need to reach into engine state,
//! dispatch handlers, and inspect dirty bits / panel context — there's
//! no narrow public API to design here, the test harness IS the audit.

// `clippy::let_underscore_must_use` (Cargo.toml `[lints.clippy]`, GH #99
// part 1) is warn-level in production code on purpose — it flags the 13
// pre-existing discarded-`Result` sites tracked separately, plus it would
// re-catch a regression of the `let _ = self.handle_x(...)` class this PR
// just fixed. In-src `#[cfg(test)] mod tests { ... }` blocks are a much
// noisier, lower-stakes surface (assertions like `let _ = fallible_call();`
// are routine test-setup idiom, not a production Task getting dropped), so
// suppress it there rather than let ~30 test-only warnings dilute the
// production signal.
#![cfg_attr(test, allow(clippy::let_underscore_must_use))]

pub mod active_bar;
pub mod app;
pub mod canvas;
pub mod chrome;
pub mod config_root;
pub mod diagnostics;
pub mod dock;
pub mod feature_flags;
pub mod find_replace;
pub mod first_run_tour;
pub mod fonts;
pub mod icons;
// Crate-internal: `IgnoreResult` is an in-tree convention, not part of
// this crate's API surface, so the module stays private.
mod ignore;
pub mod keyboard_shortcuts_modal;
pub mod keymap;
pub mod library;
pub mod menu_bar;
pub mod panels;
pub mod passive_calculator_modal;
pub mod pcb_canvas;
pub mod preferences;
pub mod render_config;
pub mod renderer_scene_canvas;
pub mod scene_shader;
pub mod schematic_runtime;
pub mod schematic_shader;
pub mod status_bar;
pub mod styles;
pub mod tab_bar;
#[cfg(test)]
pub(crate) mod test_support;
pub mod toolbar;
pub mod undo;
