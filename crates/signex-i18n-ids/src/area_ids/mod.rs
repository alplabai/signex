//! Message ids by catalog area, one file per `signex-i18n/locales/en-US/` file, for ids
//! added after the list in `lib.rs` reached the file-size cap. `ids`
//! re-exports them, so callers write `ids::NAME` either way.

mod bom;
mod chrome;
mod dialog;
mod dialog_text;
mod editors;
mod file_dialog;
mod find;
mod library;
mod notice;
mod overlay;
mod panels;
mod pcb;
mod pcb_board;
mod pcb_component;
mod pcb_primitive;
mod pdf_preview;
mod placeholder;
mod preferences;
mod update_pcb;

pub use bom::*;
pub use chrome::*;
pub use dialog::*;
pub use dialog_text::*;
pub use editors::*;
pub use file_dialog::*;
pub use find::*;
pub use library::*;
pub use notice::*;
pub use overlay::*;
pub use panels::*;
pub use pcb::*;
pub use pcb_board::*;
pub use pcb_component::*;
pub use pcb_primitive::*;
pub use pdf_preview::*;
pub use placeholder::*;
pub use preferences::*;
pub use update_pcb::*;

/// Every id of every area list.
#[cfg(any(test, feature = "test-support"))]
pub(crate) fn area_lists() -> Vec<crate::MessageId> {
    [
        bom::BOM,
        chrome::CHROME,
        dialog::DIALOG,
        dialog_text::DIALOG_TEXT,
        editors::EDITORS,
        file_dialog::FILE_DIALOG,
        find::FIND,
        library::LIBRARY,
        notice::NOTICE,
        overlay::OVERLAY,
        panels::PANELS,
        pcb::PCB,
        pcb_board::PCB_BOARD,
        pcb_component::PCB_COMPONENT,
        pcb_primitive::PCB_PRIMITIVE,
        pdf_preview::PDF_PREVIEW,
        placeholder::PLACEHOLDER,
        preferences::PREFERENCES,
        update_pcb::UPDATE_PCB,
    ]
    .concat()
}
