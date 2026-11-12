//! Message ids by catalog area, one file per `locales/en-US/` file, for ids
//! added after the list in `lib.rs` reached the file-size cap. `ids`
//! re-exports them, so callers write `ids::NAME` either way.

mod editors;
mod library;
mod menu;
mod overlay;
mod panels;
mod preferences;

pub use editors::*;
pub use library::*;
pub use menu::*;
pub use overlay::*;
pub use panels::*;
pub use preferences::*;

/// Every id of every area list.
#[cfg(test)]
pub(crate) fn area_lists() -> Vec<crate::MessageId> {
    [
        editors::EDITORS,
        library::LIBRARY,
        menu::MENU,
        overlay::OVERLAY,
        panels::PANELS,
        preferences::PREFERENCES,
    ]
    .concat()
}
