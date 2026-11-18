//! Message ids by catalog area, one file per `signex-i18n/locales/en-US/` file, for ids
//! added after the list in `lib.rs` reached the file-size cap. `ids`
//! re-exports them, so callers write `ids::NAME` either way.

mod board_checks;
mod board_output;
mod board_panel;
mod board_query;
mod bom;
mod chrome;
mod dialog;
mod dialog_text;
mod editors;
mod fab_outputs;
mod file_dialog;
mod find;
mod library;
mod notice;
mod overlay;
mod panel_body;
mod panels;
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
mod rules;
mod update_pcb;

pub use board_checks::*;
pub use board_output::*;
pub use board_panel::*;
pub use board_query::*;
pub use bom::*;
pub use chrome::*;
pub use dialog::*;
pub use dialog_text::*;
pub use editors::*;
pub use fab_outputs::*;
pub use file_dialog::*;
pub use find::*;
pub use library::*;
pub use notice::*;
pub use overlay::*;
pub use panel_body::*;
pub use panels::*;
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
pub use rules::*;
pub use update_pcb::*;

/// Every id of every area list.
#[cfg(any(test, feature = "test-support"))]
pub(crate) fn area_lists() -> Vec<crate::MessageId> {
    [
        board_checks::BOARD_CHECKS,
        board_output::BOARD_OUTPUT,
        board_panel::BOARD_PANEL,
        board_query::BOARD_QUERY,
        bom::BOM,
        chrome::CHROME,
        dialog::DIALOG,
        dialog_text::DIALOG_TEXT,
        editors::EDITORS,
        fab_outputs::FAB_OUTPUTS,
        file_dialog::FILE_DIALOG,
        find::FIND,
        library::LIBRARY,
        notice::NOTICE,
        overlay::OVERLAY,
        panel_body::PANEL_BODY,
        panels::PANELS,
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
        rules::RULES,
        update_pcb::UPDATE_PCB,
    ]
    .concat()
}
