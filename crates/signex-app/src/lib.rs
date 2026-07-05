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

pub mod active_bar;
pub mod app;
pub mod canvas;
pub mod chrome;
pub mod diagnostics;
pub mod dock;
pub mod feature_flags;
pub mod find_replace;
pub mod first_run_tour;
pub mod fonts;
pub mod icons;
pub mod keyboard_shortcuts_modal;
pub mod keymap;
pub mod library;
pub mod menu_bar;
pub mod panels;
pub mod pcb_canvas;
pub mod preferences;
pub mod render_config;
pub mod schematic_runtime;
pub mod status_bar;
pub mod styles;
pub mod tab_bar;
pub mod toolbar;
pub mod undo;
