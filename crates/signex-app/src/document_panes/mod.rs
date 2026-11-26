//! Document panes (mockup row 207, #1307): a tab dragged onto an edge of the
//! document area moves into a new pane there; with Ctrl held it opens a
//! second view of its document instead. The split is kept per project in
//! `prefs.json` and comes back when the project is reopened. Design:
//! docs/MENU_BAR.md, "View ▸ Window ▸ Split".
//!
//! This module holds the data, the rules and the store; the update half is
//! `app/handlers/document_panes.rs`, and the panes draw through the tiled
//! main window (`app/view/tiled_panes.rs`).

pub mod model;
pub mod store;

#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::Value;

pub use model::{
    DropEdge, DropTarget, MAX_PANES, Pane, PaneSplit, ShownPane, SplitRefusal, drop_target,
};
pub use store::{DocumentPanesStore, PrefsDocumentPanesStore};

/// A tab as the panes name it: its file and which page of that file it
/// shows, since a board's Layer Stack tab shares the board's path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneTab {
    pub path: PathBuf,
    /// `schematic`, `pcb`, `layer_stack`, … (`handlers::document_panes`).
    pub page: String,
}

/// The panes and what is kept of them, in `UiState`.
pub struct DocumentPanesState {
    /// The main window's split while it shows one.
    pub split: Option<PaneSplit<PaneTab>>,
    /// The stored splits by project file, raw until their project asks.
    pub kept: BTreeMap<String, Value>,
    /// The projects whose kept split was brought back, or that the user
    /// split or merged, this session: it is not brought back again.
    pub restored: HashSet<PathBuf>,
    /// Where the kept splits are written; a test swaps in a recorder.
    pub store: Arc<dyn DocumentPanesStore>,
    /// Ctrl (Cmd on macOS) is held, so a drop opens a second view.
    pub second_view_held: bool,
}

impl DocumentPanesState {
    /// The state a launch starts with: the kept splits read from `prefs`
    /// (its whole JSON), written back through `store`.
    pub fn boot(prefs: Option<&Value>, store: Arc<dyn DocumentPanesStore>) -> Self {
        Self {
            split: None,
            kept: store::read_kept(prefs),
            restored: HashSet::new(),
            store,
            second_view_held: false,
        }
    }
}
