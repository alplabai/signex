//! Message ids by catalog area, one file per `locales/en-US/` file, for ids
//! added after the list in `lib.rs` reached the file-size cap. `ids`
//! re-exports them, so callers write `ids::NAME` either way.

mod board_checks;
mod board_output;
mod dialog;
mod editors;
mod fab_outputs;
mod file_dialog;
mod library;
mod notice;
mod overlay;
mod panels;
mod pcb;
mod pcb_board;
mod pcb_component;
mod pcb_primitive;
mod pcb_stack;
mod preferences;
mod rules;
mod update_pcb;

pub use board_checks::*;
pub use board_output::*;
pub use dialog::*;
pub use editors::*;
pub use fab_outputs::*;
pub use file_dialog::*;
pub use library::*;
pub use notice::*;
pub use overlay::*;
pub use panels::*;
pub use pcb::*;
pub use pcb_board::*;
pub use pcb_component::*;
pub use pcb_primitive::*;
pub use pcb_stack::*;
pub use preferences::*;
pub use rules::*;
pub use update_pcb::*;

/// Every id of every area list.
#[cfg(test)]
pub(crate) fn area_lists() -> Vec<crate::MessageId> {
    [
        board_checks::BOARD_CHECKS,
        board_output::BOARD_OUTPUT,
        dialog::DIALOG,
        editors::EDITORS,
        fab_outputs::FAB_OUTPUTS,
        file_dialog::FILE_DIALOG,
        library::LIBRARY,
        notice::NOTICE,
        overlay::OVERLAY,
        panels::PANELS,
        pcb::PCB,
        pcb_board::PCB_BOARD,
        pcb_component::PCB_COMPONENT,
        pcb_primitive::PCB_PRIMITIVE,
        pcb_stack::PCB_STACK,
        preferences::PREFERENCES,
        rules::RULES,
        update_pcb::UPDATE_PCB,
    ]
    .concat()
}
