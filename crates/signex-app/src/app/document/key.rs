//! The identity of a document, derived from the tab that shows it.

use std::path::PathBuf;

use crate::app::state::ProjectId;
use crate::app::{TabInfo, TabKind};
use crate::library::state::EditorAddress;

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

impl DocumentKey {
    /// The document `tab` hosts; `None` for a Library Browser tab, which
    /// shows a library and is not itself a document.
    pub(crate) fn of(tab: &TabInfo) -> Option<Self> {
        match &tab.kind {
            TabKind::Schematic => Some(Self::Schematic(tab.path.clone())),
            TabKind::Pcb => Some(Self::Pcb(tab.path.clone())),
            TabKind::SymbolEditor(path) => Some(Self::Symbol(path.clone())),
            TabKind::FootprintEditor(path) => Some(Self::Footprint(path.clone())),
            TabKind::ComponentEditor(ce) => Some(Self::Row(EditorAddress::new(
                ce.library_path.clone(),
                ce.table.clone(),
                ce.row_id,
            ))),
            TabKind::Project(id) => Some(Self::Project(*id)),
            TabKind::LibraryBrowser(_) => None,
        }
    }
}
