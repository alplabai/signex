//! Paste Array dialog (#1264): state and messages (`state`) and the pure view
//! (`view`). The handler that validates and places lives in
//! `app/handlers/paste/array.rs`.

mod state;
mod view;

pub use state::{MAX_COPIES, PasteArrayInput, PasteArrayMsg, PasteArrayState};
pub use view::view;
