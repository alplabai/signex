//! The identity of a document. Deriving it from a tab or a path lives in
//! `app/document/key.rs`.

use std::path::PathBuf;

use crate::app::core::editor_address::EditorAddress;
use crate::app::core::project_id::ProjectId;

/// Which document a tab hosts. Each kind keeps its state in a different map,
/// so the key names the owner instead of boxing it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum DocumentKey {
    Schematic(PathBuf),
    Pcb(PathBuf),
    Symbol(PathBuf),
    Footprint(PathBuf),
    Row(EditorAddress),
    Project(ProjectId),
}
