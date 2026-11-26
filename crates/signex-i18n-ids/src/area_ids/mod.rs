//! Message ids by catalog area, one file per `signex-i18n/locales/en-US/` file, for ids
//! added after the list in `lib.rs` reached the file-size cap. `ids`
//! re-exports them, so callers write `ids::NAME` either way.

mod annotations;
mod assign_footprints;
mod board_checks;
mod board_output;
mod board_panel;
mod board_query;
mod board_sketch;
mod board_status;
mod board_view;
mod bom;
mod chrome;
mod command_log;
mod command_needs;
mod compare_board;
mod dialog;
mod dialog_text;
mod drc_rules;
mod editors;
mod fab_outputs;
mod file_dialog;
mod find;
mod fp_props;
mod gerber_viewer;
mod library;
mod new_project;
mod notebook_measurements;
mod notebook_pdf;
mod notebook_references;
mod notebook_tab;
mod notice;
mod overlay;
mod panel_body;
mod panels;
mod parameter_io;
mod passive_calculator;
mod pcb;
mod pcb_board;
mod pcb_component;
mod pcb_graphics;
mod pcb_primitive;
mod pcb_stack;
mod pdf_preview;
mod placeholder;
mod preferences;
mod project;
mod properties;
mod requirements;
mod rooms;
mod rules;
mod sheet_sync;
mod sketch_bake;
mod sketch_canvas;
mod sketch_parameters;
mod sketch_snap;
mod sketch_tools;
mod update_pcb;
mod workspace_layouts;

pub use annotations::*;
pub use assign_footprints::*;
pub use board_checks::*;
pub use board_output::*;
pub use board_panel::*;
pub use board_query::*;
pub use board_sketch::*;
pub use board_status::*;
pub use board_view::*;
pub use bom::*;
pub use chrome::*;
pub use command_log::*;
pub use command_needs::*;
pub use compare_board::*;
pub use dialog::*;
pub use dialog_text::*;
pub use drc_rules::*;
pub use editors::*;
pub use fab_outputs::*;
pub use file_dialog::*;
pub use find::*;
pub use fp_props::*;
pub use gerber_viewer::*;
pub use library::*;
pub use new_project::*;
pub use notebook_measurements::*;
pub use notebook_pdf::*;
pub use notebook_references::*;
pub use notebook_tab::*;
pub use notice::*;
pub use overlay::*;
pub use panel_body::*;
pub use panels::*;
pub use parameter_io::*;
pub use passive_calculator::*;
pub use pcb::*;
pub use pcb_board::*;
pub use pcb_component::*;
pub use pcb_graphics::*;
pub use pcb_primitive::*;
pub use pcb_stack::*;
pub use pdf_preview::*;
pub use placeholder::*;
pub use preferences::*;
pub use project::*;
pub use properties::*;
pub use requirements::*;
pub use rooms::*;
pub use rules::*;
pub use sheet_sync::*;
pub use sketch_bake::*;
pub use sketch_canvas::*;
pub use sketch_parameters::*;
pub use sketch_snap::*;
pub use sketch_tools::*;
pub use update_pcb::*;
pub use workspace_layouts::*;

/// Every id of every area list.
#[cfg(any(test, feature = "test-support"))]
pub(crate) fn area_lists() -> Vec<crate::MessageId> {
    [
        annotations::ANNOTATIONS,
        assign_footprints::ASSIGN_FOOTPRINTS,
        board_checks::BOARD_CHECKS,
        board_output::BOARD_OUTPUT,
        board_panel::BOARD_PANEL,
        board_query::BOARD_QUERY,
        board_status::BOARD_STATUS,
        board_view::BOARD_VIEW,
        bom::BOM,
        chrome::CHROME,
        command_log::COMMAND_LOG,
        command_needs::COMMAND_NEEDS,
        compare_board::COMPARE_BOARD,
        dialog::DIALOG,
        dialog_text::DIALOG_TEXT,
        drc_rules::DRC_RULES,
        editors::EDITORS,
        fab_outputs::FAB_OUTPUTS,
        file_dialog::FILE_DIALOG,
        find::FIND,
        gerber_viewer::GERBER_VIEWER,
        fp_props::FP_PROPS,
        library::LIBRARY,
        new_project::NEW_PROJECT,
        notebook_measurements::NOTEBOOK_MEASUREMENTS,
        notebook_pdf::NOTEBOOK_PDF,
        notebook_references::NOTEBOOK_REFERENCES,
        notebook_tab::NOTEBOOK_TAB,
        notice::NOTICE,
        overlay::OVERLAY,
        panel_body::PANEL_BODY,
        panels::PANELS,
        parameter_io::PARAMETER_IO,
        passive_calculator::PASSIVE_CALCULATOR,
        pcb::PCB,
        pcb_board::PCB_BOARD,
        pcb_component::PCB_COMPONENT,
        pcb_graphics::PCB_GRAPHICS,
        pcb_primitive::PCB_PRIMITIVE,
        pcb_stack::PCB_STACK,
        pdf_preview::PDF_PREVIEW,
        placeholder::PLACEHOLDER,
        preferences::PREFERENCES,
        project::PROJECT,
        properties::PROPERTIES,
        requirements::REQUIREMENTS,
        rooms::ROOMS,
        rules::RULES,
        sheet_sync::SHEET_SYNC,
        sketch_snap::SKETCH_SNAP,
        update_pcb::UPDATE_PCB,
        workspace_layouts::WORKSPACE_LAYOUTS,
        sketch_parameters::SKETCH_PARAMETERS,
        sketch_canvas::SKETCH_CANVAS,
        board_sketch::BOARD_SKETCH,
        sketch_bake::SKETCH_BAKE,
        sketch_tools::SKETCH_TOOLS,
    ]
    .concat()
}
