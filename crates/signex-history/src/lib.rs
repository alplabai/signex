//! Snapshot undo/redo history with a save point (#661), shared by every
//! document kind. UI-free and dependency-free: it knows stack lengths, not
//! what a step holds.

mod history;
mod save_point;

pub use history::{
    DEFAULT_UNDO_DEPTH, History, MAX_UNDO_DEPTH, MIN_UNDO_DEPTH, undo_depth_in_range,
};
pub use save_point::SavePoint;
