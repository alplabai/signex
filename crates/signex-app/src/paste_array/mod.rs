//! Paste Array dialog (#1264, #2527): state and messages (`state`), the
//! preview drawing (`preview`), pointer input on it (`pointer`) and the pure
//! view (`view`). The handler that validates and places lives in
//! `app/handlers/paste/array.rs`.

mod pointer;
mod preview;
mod state;
mod view;

pub(crate) use preview::grid as preview_grid;
pub use preview::{PREVIEW_MAX_DRAWN, PreviewPlan, preview_plan};
pub use state::{
    MAX_COPIES, MAX_PLACED_ITEMS, PasteArrayField, PasteArrayInput, PasteArrayMsg, PasteArrayState,
};
pub(crate) use view::PREVIEW_EDIT_INPUT_ID;
pub use view::view;
