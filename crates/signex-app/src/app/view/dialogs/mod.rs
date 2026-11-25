//! Shared modal-chrome constants (single source of truth) plus re-exports
//! of the shared modal primitives, kept at this path so the existing
//! `crate::app::view::dialogs::…` imports across the crate keep resolving.
//!
//! The dialog builders that used to live here were split into the child
//! modules of this `dialogs` folder (ADR-0001, issue #164) as pure code
//! motion.

use super::*;

mod annotate;
mod assign_footprints;
mod autosave_recovery;
mod back_annotate;
mod board_output;
mod board_reannotate;
mod bom;
pub(in crate::app) use bom::{default_column_width as bom_default_column_width, sorted_row_order};
// Mockup row 155: Change Component, drawn in the component picker modal.
pub(crate) mod change_component;
mod close_project;
mod compare_board;
mod confirms;
mod create_child_sheet;
mod create_rooms;
// #1282: Design ▸ Differential Pair Classes….
mod diff_pair_classes;
mod erc;
mod fab_outputs;
mod graphite;
mod graphite_annotate;
pub(crate) mod graphite_list;
pub(crate) mod grid_properties;
mod header;
mod netlist_incomplete;
pub(crate) mod new_project;
// Mockup row 202: Export Design Notebook PDF.
mod notebook_pdf;
mod preview_frame;
pub(crate) use preview_frame::preview_frame;
mod project;
mod quit_confirm;
/// `pub(crate)`: the window sizing and the Enter key read its `content`.
pub(crate) mod remove;
mod rename;
mod rule_editor;
mod save_project_as;
mod sheet_numbering;
mod sheet_sync;
mod sizes;
pub(crate) use sizes::NEW_PROJECT as NEW_PROJECT_SIZE;
mod title_bar;
mod update_pcb;
mod version_control;
mod workspace_layouts;
pub(crate) use header::DIALOG_HEADER_H;
pub(crate) use rule_editor::{
    RuleEditor, changed_cell_count, changed_rule_count, matrix_pane, one_rule_line,
    section_heading, severity_pane, shipped_rules,
};
pub(crate) use title_bar::DIALOG_TITLE_BAR_H;
mod widgets;
pub(crate) use header::{
    PREFERENCES_HEADER_H, dialog_scrim, graphite_header, graphite_header_closing,
    neutral_close_button, scrim_style,
};
pub(in crate::app::view) use widgets::{draggable_header, fit_modal_size};
pub(crate) use widgets::{modal_title_header, os_title_bar_slot};

#[cfg(test)]
mod fit_window_tests;
