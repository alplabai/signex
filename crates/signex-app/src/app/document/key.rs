//! The identity of a document, derived from the tab that shows it.

use std::path::{Path, PathBuf};

use crate::app::state::{DocumentState, ProjectId};
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

impl DocumentState {
    /// The document the file at `path` is held as, by the precedence Save
    /// All routes by: a live engine, then a PCB tab or parked board, then a
    /// symbol or footprint editor, then an open project's own `.snxprj`. A
    /// file nothing holds, a library manifest included, has no key.
    pub(crate) fn document_key_for_path(&self, path: &Path) -> Option<DocumentKey> {
        if self.engines.contains_key(path) {
            return Some(DocumentKey::Schematic(path.to_path_buf()));
        }
        if self.holds_pcb(path) {
            return Some(DocumentKey::Pcb(path.to_path_buf()));
        }
        if self.symbol_editors.contains_key(path) {
            return Some(DocumentKey::Symbol(path.to_path_buf()));
        }
        if self.footprint_editors.contains_key(path) {
            return Some(DocumentKey::Footprint(path.to_path_buf()));
        }
        self.projects
            .iter()
            .find(|p| p.path == path)
            .map(|p| DocumentKey::Project(p.id))
    }
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
