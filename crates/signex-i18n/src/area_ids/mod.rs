//! Message ids by catalog area, one file per `locales/en-US/` file, for ids
//! added after the list in `lib.rs` reached the file-size cap. `ids`
//! re-exports them, so callers write `ids::NAME` either way.

mod bom;
mod dialog;
mod dialog_text;
mod editors;
mod file_dialog;
mod find;
mod library;
mod notice;
mod overlay;
mod panels;
mod pdf_preview;
mod preferences;

pub use bom::*;
pub use dialog::*;
pub use dialog_text::*;
pub use editors::*;
pub use file_dialog::*;
pub use find::*;
pub use library::*;
pub use notice::*;
pub use overlay::*;
pub use panels::*;
pub use pdf_preview::*;
pub use preferences::*;

/// Every id of every area list.
#[cfg(test)]
pub(crate) fn area_lists() -> Vec<crate::MessageId> {
    [
        bom::BOM,
        dialog::DIALOG,
        dialog_text::DIALOG_TEXT,
        editors::EDITORS,
        file_dialog::FILE_DIALOG,
        find::FIND,
        library::LIBRARY,
        notice::NOTICE,
        overlay::OVERLAY,
        panels::PANELS,
        pdf_preview::PDF_PREVIEW,
        preferences::PREFERENCES,
    ]
    .concat()
}
