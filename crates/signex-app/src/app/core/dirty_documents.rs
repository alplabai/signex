//! The stored dirty set; `state/dirty.rs` owns every read and write of it
//! (see docs/DIRTY_STATE.md).

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

/// The `.snxprj` saved cursors, keyed by project file
/// (`state/dirty/project_history.rs`).
pub(crate) type ProjectSavedBytes = HashMap<PathBuf, Vec<u8>>;

/// The paths of documents with unsaved edits that no editor draft holds.
#[derive(Debug, Default)]
pub(crate) struct DirtyDocuments {
    pub(in crate::app) paths: HashSet<PathBuf>,
    /// The subset of `paths` marked by something other than the engine's
    /// undo history (a project edit, a net colour, an annotate pass, a
    /// restore). Undoing back to the engine's save point cannot clear these:
    /// only a save does.
    pub(in crate::app) external: HashSet<PathBuf>,
    /// A `.snxprj`'s saved cursor: the bytes it encodes to when it equals the
    /// file (`project_history.rs`). Absent means unknown, so it stays dirty.
    pub(in crate::app) project_saved: ProjectSavedBytes,
}
