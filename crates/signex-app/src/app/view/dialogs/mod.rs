//! Shared modal-chrome constants (single source of truth) plus re-exports
//! of the shared modal primitives, kept at this path so the existing
//! `crate::app::view::dialogs::…` imports across the crate keep resolving.
//!
//! The dialog builders that used to live here were split into the child
//! modules of this `dialogs` folder (ADR-0001, issue #164) as pure code
//! motion.

use super::*;

mod annotate;
mod autosave_recovery;
mod bom;
pub(in crate::app) use bom::{default_column_width as bom_default_column_width, sorted_row_order};
mod confirms;
mod create_child_sheet;
mod erc;
mod graphite;
mod graphite_annotate;
mod header;
pub(crate) mod new_project;
mod project;
mod rule_editor;
mod sheet_numbering;
mod sizes;
mod title_bar;
mod update_pcb;
pub(crate) use header::DIALOG_HEADER_H;
pub(crate) use rule_editor::{RuleEditor, rule_editor};
pub(crate) use title_bar::DIALOG_TITLE_BAR_H;
mod widgets;
pub(crate) use header::{
    PREFERENCES_HEADER_H, graphite_header, graphite_header_closing, scrim_style,
};
pub(in crate::app::view) use widgets::{draggable_header, fit_modal_size};
pub(crate) use widgets::{modal_title_header, os_title_bar_slot};

#[cfg(test)]
mod fit_window_tests;
