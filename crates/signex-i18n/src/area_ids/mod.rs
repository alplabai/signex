//! Message ids by catalog area, one file per `locales/en-US/` file, for ids
//! added after the list in `lib.rs` reached the file-size cap. `ids`
//! re-exports them, so callers write `ids::NAME` either way.

mod dialog;
mod editors;
mod file_dialog;
mod library;
mod notice;
mod overlay;
mod panels;
mod preferences;

pub use dialog::*;
pub use editors::*;
pub use file_dialog::*;
pub use library::*;
pub use notice::*;
pub use overlay::*;
pub use panels::*;
pub use preferences::*;

/// Every id of every area list.
#[cfg(test)]
pub(crate) fn area_lists() -> Vec<crate::MessageId> {
    [
        dialog::DIALOG,
        editors::EDITORS,
        file_dialog::FILE_DIALOG,
        library::LIBRARY,
        notice::NOTICE,
        overlay::OVERLAY,
        panels::PANELS,
        preferences::PREFERENCES,
    ]
    .concat()
}
