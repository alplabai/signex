//! Paste Array dialog (#1264, #2527): state and messages (`state`), the
//! read-only preview drawing (`preview`) and the pure view (`view`). The
//! handler that validates and places lives in `app/handlers/paste/array.rs`.

mod preview;
mod state;
mod view;

pub use preview::{PREVIEW_MAX_DRAWN, PreviewPlan, preview_plan};
pub use state::{
    MAX_COPIES, MAX_PLACED_ITEMS, PasteArrayField, PasteArrayInput, PasteArrayMsg, PasteArrayState,
};
pub use view::view;
