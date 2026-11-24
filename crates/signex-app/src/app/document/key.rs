//! The identity of a document, derived from the tab that shows it. The key
//! type itself lives in `app::core::document_key`.

use std::path::Path;

use crate::app::core::document_key::DocumentKey;
use crate::app::core::editor_address::EditorAddress;
use crate::app::state::DocumentState;
use crate::app::{TabInfo, TabKind};

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
            // A board page tab edits, saves and undoes its board.
            TabKind::Pcb | TabKind::LayerStack | TabKind::PolygonManager => {
                Some(Self::Pcb(tab.path.clone()))
            }
            TabKind::SymbolEditor(path) => Some(Self::Symbol(path.clone())),
            // A Pin table tab edits, saves and undoes its symbol.
            TabKind::PinTable => Some(Self::Symbol(tab.path.clone())),
            TabKind::FootprintEditor(path) => Some(Self::Footprint(path.clone())),
            TabKind::ComponentEditor(ce) => Some(Self::Row(EditorAddress::new(
                ce.library_path.clone(),
                ce.table.clone(),
                ce.row_id,
            ))),
            TabKind::Project(id) | TabKind::Rules(id) => Some(Self::Project(*id)),
            TabKind::LibraryBrowser(_) => None,
            // Read-only fabrication files: nothing to save or undo.
            TabKind::GerberViewer => None,
        }
    }
}
