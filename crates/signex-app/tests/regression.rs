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

#[path = "support/drive.rs"]
mod drive;

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

#[path = "regression/paste_clipboard.rs"]
mod paste_clipboard;
#[path = "regression/paste_numbering_async.rs"]
mod paste_numbering_async;
#[path = "regression/paste_stamp_array.rs"]
mod paste_stamp_array;
#[path = "regression/paste_support.rs"]
mod paste_support;

#[path = "regression/paste_instance_entries.rs"]
mod paste_instance_entries;

#[path = "regression/annotate_undo_scope.rs"]
mod annotate_undo_scope;

#[path = "regression/prefs.rs"]
mod prefs;

#[path = "regression/net_nav.rs"]
mod net_nav;

// #1070 — the palette lists only commands that can act in the active editor.
#[path = "regression/palette_context_filter.rs"]
mod palette_context_filter;

#[path = "regression/preferences_dirty_guard.rs"]
mod preferences_dirty_guard;

#[path = "regression/detached_modal_os_close.rs"]
mod detached_modal_os_close;

#[path = "regression/diagnostics_routing.rs"]
mod diagnostics_routing;

#[path = "regression/autosave.rs"]
mod autosave;

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
#[path = "regression/rule_profiles_ui.rs"]
mod rule_profiles_ui;

#[path = "regression/preferences_symbol_drafts.rs"]
mod preferences_symbol_drafts;

#[path = "regression/preferences_window_controls.rs"]
mod preferences_window_controls;

#[path = "regression/undo_marker_divergence.rs"]
mod undo_marker_divergence;

#[path = "regression/primitive_dirty_quit.rs"]
mod primitive_dirty_quit;

#[path = "regression/dirty_paths_gateway.rs"]
mod dirty_paths_gateway;

// #1250 — a locked symbol refuses delete and move, with a Messages note.
#[path = "regression/symbol_locked_enforced.rs"]
mod symbol_locked_enforced;

// #768 — an over-cap foreign-column save names the objects to delete.
#[path = "regression/foreign_column_save_blocked.rs"]
mod foreign_column_save_blocked;

#[path = "regression/library_undo.rs"]
mod library_undo;

#[path = "regression/library_placement.rs"]
mod library_placement;

#[path = "regression/library_pad_geometry.rs"]
mod library_pad_geometry;

#[path = "regression/footprint_legacy_pad_stack.rs"]
mod footprint_legacy_pad_stack;
#[path = "regression/footprint_pad_fields_persist.rs"]
mod footprint_pad_fields_persist;

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
#[path = "regression/library_range_filter.rs"]
mod library_range_filter;
#[path = "regression/library_row_base_local.rs"]
mod library_row_base_local;

#[path = "regression/library_read_only_mount.rs"]
mod library_read_only_mount;

#[path = "regression/library_new_component.rs"]
mod library_new_component;

#[path = "regression/library_pricing_refresh.rs"]
mod library_pricing_refresh;

#[path = "regression/render_config_grid_style.rs"]
mod render_config_grid_style;

#[path = "regression/zoom_speed_pref.rs"]
mod zoom_speed_pref;

#[path = "regression/ui_scale_pref.rs"]
mod ui_scale_pref;

#[path = "regression/rename_preserves_references.rs"]
mod rename_preserves_references;

#[path = "regression/save_as_pending_open.rs"]
mod save_as_pending_open;

#[path = "regression/rename_save_as_rekey.rs"]
mod rename_save_as_rekey;

#[path = "regression/library_row_identity.rs"]
mod library_row_identity;

#[path = "regression/deleted_sheet_not_resaved.rs"]
mod deleted_sheet_not_resaved;
#[path = "regression/exclude_dirty_sheet_prompt.rs"]
mod exclude_dirty_sheet_prompt;
#[path = "regression/library_components_panel_actions.rs"]
mod library_components_panel_actions;
#[path = "regression/library_git_recovery.rs"]
mod library_git_recovery;

#[path = "regression/sync_sheet_pins.rs"]
mod sync_sheet_pins;

// #1061 — a net named differently on two sheets is reported by an ERC run.
#[path = "regression/cross_sheet_multiple_names.rs"]
mod cross_sheet_multiple_names;

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
#[path = "regression/managed_gitignore.rs"]
mod managed_gitignore;
#[path = "regression/new_project_folder.rs"]
mod new_project_folder;
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
#[path = "regression/symbol_body_style.rs"]
mod symbol_body_style;

#[path = "regression/place_sheet_tool.rs"]
mod place_sheet_tool;
#[path = "regression/sheet_symbol_edit.rs"]
mod sheet_symbol_edit;

#[path = "regression/symbol_undo_steps.rs"]
mod symbol_undo_steps;

#[path = "regression/field_edit_support.rs"]
mod field_edit_support;
#[path = "regression/library_alternate_symbols.rs"]
mod library_alternate_symbols;
#[path = "regression/library_place_designator_standard.rs"]
mod library_place_designator_standard;
#[path = "regression/library_place_symbol.rs"]
mod library_place_symbol;
#[path = "regression/library_place_ui.rs"]
mod library_place_ui;
#[path = "regression/library_stock_place.rs"]
mod library_stock_place;
#[path = "regression/library_symbol_drift.rs"]
mod library_symbol_drift;
#[path = "regression/repeat_placement.rs"]
mod repeat_placement;
// #1258 — Properties edits the shared fields of a multi-selection.
#[path = "regression/multi_selection_edit.rs"]
mod multi_selection_edit;
// #1255, #1670 — in-place editing of a part's designator, value and footprint.
#[path = "regression/inplace_field_edit.rs"]
mod inplace_field_edit;
// #1301, #1262, #1260 — Open Datasheet, Edit Symbol and Change Component
// on a placed part.
#[path = "regression/placed_part_actions.rs"]
mod placed_part_actions;
// #1231 — Properties lists a placed symbol's library row documents.
#[path = "regression/library_row_documents.rs"]
mod library_row_documents;
// #1338 — a placed symbol's library link survives save and reopen, and the
// open-time scan it switches on behaves.
#[path = "regression/library_link_reopen.rs"]
mod library_link_reopen;
#[path = "regression/library_update_apply.rs"]
mod library_update_apply;
#[path = "regression/non_finite_edits.rs"]
mod non_finite_edits;
#[path = "regression/numeric_drafts.rs"]
mod numeric_drafts;
#[path = "regression/paste_on_cursor.rs"]
mod paste_on_cursor;
#[path = "regression/placement_session.rs"]
mod placement_session;
#[path = "regression/reprefix_standard.rs"]
mod reprefix_standard;

#[path = "regression/canvas_editing_correctness.rs"]
mod canvas_editing_correctness;
#[path = "regression/schematic_field_drag.rs"]
mod schematic_field_drag;

#[path = "regression/surface_failures.rs"]
mod surface_failures;
#[path = "regression/text_font_family.rs"]
mod text_font_family;

#[path = "regression/junction_tool.rs"]
mod junction_tool;
#[path = "regression/no_erc_directive.rs"]
mod no_erc_directive;

#[path = "regression/parameter_set_no_net_label.rs"]
mod parameter_set_no_net_label;

#[path = "regression/styled_power_port_not_flag.rs"]
mod styled_power_port_not_flag;

#[path = "regression/click_click_move.rs"]
mod click_click_move;
#[path = "regression/rubber_band_move.rs"]
mod rubber_band_move;

// #1057 - grid snapping lands on exact nm multiples (align / move).
#[path = "regression/grid_snap_exact.rs"]
mod grid_snap_exact;

// #1246 - the electrical-grid toggle snaps wire clicks onto pin ends.
#[path = "regression/hotspot_snap.rs"]
mod hotspot_snap;

// #925 — non-finite grid steps are refused into the Messages panel.
#[path = "regression/grid_step_refusal.rs"]
mod grid_step_refusal;

// #961 — the Annotate dialog's order of processing drives the numbering.
#[path = "regression/annotate_order.rs"]
mod annotate_order;

// #962 — Space / Shift+Space rotate in opposite directions, mirrored symbols included.
#[path = "regression/rotate_direction.rs"]
mod rotate_direction;

// #859 / #1068 — a document edited while Close Project Save All writes is asked about again.
#[path = "regression/save_all_race.rs"]
mod save_all_race;

// #1096 — Rename / Delete refuse a file whose open is still pending.
#[path = "regression/pending_open_rename_delete.rs"]
mod pending_open_rename_delete;

// #1050 — bus entries snap onto a bus at placement and rotate / mirror
// (Space / Shift+Space / X / Y), at placement and on a selected entry.
#[path = "regression/bus_entry.rs"]
mod bus_entry;

// #1404 — the X / Y keys, context-menu Mirror X / Y and the active bar's
// Flip Along X / Y all flip a symbol the same way.
#[path = "regression/drag_direction_select.rs"]
mod drag_direction_select;

#[path = "regression/measure_and_zoom_selection.rs"]
mod measure_and_zoom_selection;

#[path = "regression/mirror_axis_entry_points.rs"]
mod mirror_axis_entry_points;

// #1039 — rotate / mirror act on any selection, about its centre, as one undo step.
#[path = "regression/rotate_mirror_selection.rs"]
mod rotate_mirror_selection;

// #1080 — the sheet's title block is edited from the Properties panel.
#[path = "regression/title_block_fields.rs"]
mod title_block_fields;

// #1042 — the Properties panel edits the project's net-class table through
// undoable engine commands.
#[path = "regression/net_class_rendering.rs"]
mod net_class_rendering;
#[path = "regression/net_classes_panel.rs"]
mod net_classes_panel;

// #1079 — the canvas draws the sheet frame, zones and title block.
#[path = "regression/sheet_frame_canvas.rs"]
mod sheet_frame_canvas;

// #1249 / #1251 — planned Active Bar tools arm and place nothing; Generate
// BOM opens the BOM dialog.
#[path = "regression/planned_tools.rs"]
mod planned_tools;

// #1202 — the Project document tab replaces the Project Options modal.
#[path = "regression/project_parameters.rs"]
mod project_parameters;
#[path = "regression/project_tab.rs"]
mod project_tab;
#[path = "regression/project_tab_rename.rs"]
mod project_tab_rename;
#[path = "regression/project_tab_saves.rs"]
mod project_tab_saves;
#[path = "regression/project_tab_support.rs"]
mod project_tab_support;

#[path = "regression/navigator_async.rs"]
mod navigator_async;
