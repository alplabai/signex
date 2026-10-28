//! Find / Replace dialog: state and messages (`state`) and the pure view
//! (`view`). The handlers that scan, replace and navigate live in
//! `app/handlers/find_replace/`.

mod state;
mod view;

pub use state::{
    ChildSheetSignature, FindMatch, FindReplaceMsg, FindReplaceState, FindScope, ProjectScan,
    ReplaceJob, ReplacePrep,
};
pub use view::{view, window_view};
