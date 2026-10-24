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

#[path = "regression/close_support.rs"]
mod close_support;

#[path = "regression/settle_support.rs"]
mod settle_support;

#[path = "regression/project.rs"]
mod project;

#[path = "regression/bom_validation.rs"]
mod bom_validation;

#[path = "regression/pdf_page_setup.rs"]
mod pdf_page_setup;

#[path = "regression/annotate_undo_scope.rs"]
mod annotate_undo_scope;

#[path = "regression/prefs.rs"]
mod prefs;

#[path = "regression/preferences_dirty_guard.rs"]
mod preferences_dirty_guard;

#[path = "regression/detached_modal_os_close.rs"]
mod detached_modal_os_close;

#[path = "regression/diagnostics_routing.rs"]
mod diagnostics_routing;

#[path = "regression/dock_layout_persist.rs"]
mod dock_layout_persist;
#[path = "regression/document_save_async.rs"]
mod document_save_async;
// #859 — rename / History restore wait for an in-flight document write.
#[path = "regression/document_save_ordering.rs"]
mod document_save_ordering;
#[path = "regression/search_prefs_persist.rs"]
mod search_prefs_persist;

#[path = "regression/preferences_export_status.rs"]
mod preferences_export_status;

#[path = "regression/preferences_prefs_recovery.rs"]
mod preferences_prefs_recovery;

#[path = "regression/preferences_symbol_drafts.rs"]
mod preferences_symbol_drafts;

#[path = "regression/undo_marker_divergence.rs"]
mod undo_marker_divergence;

#[path = "regression/primitive_dirty_quit.rs"]
mod primitive_dirty_quit;

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

#[path = "regression/startup_open.rs"]
mod startup_open;

#[path = "regression/history_load_failure.rs"]
mod history_load_failure;

#[path = "regression/history_reload_repair.rs"]
mod history_reload_repair;

#[path = "regression/library_browser_cell_commit.rs"]
mod library_browser_cell_commit;

#[path = "regression/library_new_component.rs"]
mod library_new_component;

#[path = "regression/render_config_grid_style.rs"]
mod render_config_grid_style;

#[path = "regression/zoom_speed_pref.rs"]
mod zoom_speed_pref;

#[path = "regression/rename_preserves_references.rs"]
mod rename_preserves_references;

#[path = "regression/rename_save_as_rekey.rs"]
mod rename_save_as_rekey;

#[path = "regression/deleted_sheet_not_resaved.rs"]
mod deleted_sheet_not_resaved;
#[path = "regression/library_components_panel_actions.rs"]
mod library_components_panel_actions;
#[path = "regression/library_git_recovery.rs"]
mod library_git_recovery;

#[path = "regression/sync_sheet_pins.rs"]
mod sync_sheet_pins;

#[path = "regression/start_screen.rs"]
mod start_screen;

#[path = "regression/command_enablement.rs"]
mod command_enablement;
#[path = "regression/dead_schematic_controls.rs"]
mod dead_schematic_controls;
#[path = "regression/idle_cursor_move.rs"]
mod idle_cursor_move;
#[path = "regression/library_dangling_refs.rs"]
mod library_dangling_refs;
#[path = "regression/library_delete_undo.rs"]
mod library_delete_undo;
#[path = "regression/newer_minor_read_only.rs"]
mod newer_minor_read_only;
#[path = "regression/primitive_container_merge.rs"]
mod primitive_container_merge;
#[path = "regression/primitive_save_cascade.rs"]
mod primitive_save_cascade;
#[path = "regression/reload_and_switch_state.rs"]
mod reload_and_switch_state;
#[path = "regression/save_as_by_document_kind.rs"]
mod save_as_by_document_kind;
#[path = "regression/stable_addressing.rs"]
mod stable_addressing;

#[path = "regression/place_sheet_tool.rs"]
mod place_sheet_tool;
#[path = "regression/sheet_symbol_edit.rs"]
mod sheet_symbol_edit;

#[path = "regression/symbol_undo_steps.rs"]
mod symbol_undo_steps;

#[path = "regression/library_place_symbol.rs"]
mod library_place_symbol;

#[path = "regression/canvas_editing_correctness.rs"]
mod canvas_editing_correctness;
#[path = "regression/schematic_field_drag.rs"]
mod schematic_field_drag;
#[path = "regression/surface_failures.rs"]
mod surface_failures;

#[path = "regression/no_erc_directive.rs"]
mod no_erc_directive;

#[path = "regression/click_click_move.rs"]
mod click_click_move;

// #925 — non-finite grid steps are refused into the Messages panel.
#[path = "regression/grid_step_refusal.rs"]
mod grid_step_refusal;

// #961 — the Annotate dialog's order of processing drives the numbering.
#[path = "regression/annotate_order.rs"]
mod annotate_order;
