//! Document panes (mockup row 207, #1307): a tab dragged onto an edge of the
//! document area moves into a new pane there; with Ctrl held it opens a
//! second view of its document instead. The split is kept per project in
//! `prefs.json` and comes back when the project is reopened. Design:
//! docs/MENU_BAR.md, "View ▸ Window ▸ Split".
//!
//! This module holds the data, the rules and the store; the update half is
//! `app/handlers/document_panes.rs`, and the panes draw through the tiled
//! main window (`app/view/tiled_panes.rs`).

pub mod geometry;
pub mod model;
mod ops;
pub mod store;

#[cfg(test)]
mod geometry_tests;
#[cfg(test)]
mod ops_tests;
#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::Value;

pub use geometry::{PANE_STRIP_HEIGHT, PaneShares, Splitter, grid_groups, pane_tiles, splitters};
pub use model::{
    DropEdge, DropTarget, MAX_PANES, Pane, PaneId, PaneSplit, ShownPane, SplitRefusal, drop_target,
};
pub use store::{DocumentPanesStore, PrefsDocumentPanesStore};

/// A pane's pan and zoom: the screen offset of the world origin and the
/// pixels per mm, as `live_camera` reads them.
pub type PaneCamera = (f32, f32, f32);

/// `camera` at `scale` pixels per mm, keeping the world point at the
/// centre of a `viewport` (width, height) where it is: Sync Zoom with Other
/// Pane. A scale that is not a positive number leaves it as it is.
pub fn zoom_kept_centre(camera: PaneCamera, viewport: (f32, f32), scale: f32) -> PaneCamera {
    let (x, y, old) = camera;
    if !(scale.is_finite() && scale > 0.0 && old.is_finite() && old > 0.0) {
        return camera;
    }
    let (cx, cy) = (viewport.0 / 2.0, viewport.1 / 2.0);
    let (wx, wy) = ((cx - x) / old, (cy - y) / old);
    (cx - wx * scale, cy - wy * scale, scale)
}

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
    /// The pane sizes after a splitter drag; `None` is equal.
    pub shares: Option<PaneShares>,
    /// The splitter being dragged, by its index.
    pub splitter_drag: Option<usize>,
    /// The pane whose ⋯ menu is open, by its place among the panes.
    pub menu: Option<usize>,
    /// The pane the main canvases drew at the last settle, by its id
    /// (`None` under plain tiling, where a pane is its one tab), and its
    /// document: a pane that loses the focus keeps their view. An id, not a
    /// place, so an earlier pane closing does not read as a focus move.
    pub focus: Option<(Option<PaneId>, PathBuf)>,
    /// What the panes outlined for the focused selection last, so the
    /// outline is drawn again only when it changed.
    pub outlined: Option<u64>,
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
            shares: None,
            splitter_drag: None,
            menu: None,
            focus: None,
            outlined: None,
        }
    }
}
