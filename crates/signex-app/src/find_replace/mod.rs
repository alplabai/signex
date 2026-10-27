//! Find / Replace dialog: state and messages (`state`) and the pure view
//! (`view`). The handlers that scan, replace and navigate live in
//! `app/handlers/find_replace/`.

mod state;
mod view;

pub use state::{FindMatch, FindReplaceMsg, FindReplaceState, FindScope, ProjectScan};
pub use view::view;
