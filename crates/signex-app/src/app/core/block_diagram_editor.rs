//! The block diagram tab's document state (mockup row 171): every open
//! diagram by its path, with its undo and redo steps, and the types its view
//! state names. Pure data and pure rules; the reads and writes run in
//! `app::block_diagram`, off the update thread. What the tab shows of a
//! diagram is in `block_diagram_view`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use signex_types::block_diagram::{BlockDiagram, ReadBlockDiagram, SNXBLK_VERSION};
use uuid::Uuid;

pub use super::block_diagram_panes::{
    DiagramView, ExportDraft, ExportKind, ExportPage, ExportPaper, PowerTreeRead, PowerTreeView,
    ResyncReview,
};
pub use super::block_diagram_view::BlockDiagramViewState;

/// Every open block diagram, and the paths being read or written.
#[derive(Debug, Default)]
pub struct BlockDiagrams {
    /// The open diagrams by path. A diagram stays here after its tab closes
    /// while it holds unsaved edits, so Save All and the quit prompt see it.
    pub editors: BTreeMap<PathBuf, BlockDiagramEditor>,
    /// Paths a read is running for.
    pub opening: BTreeSet<PathBuf>,
    /// Paths a write is running for.
    pub saving: BTreeSet<PathBuf>,
}

/// One object of a diagram, as the canvas and Properties pick it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DiagramItem {
    Block(Uuid),
    Port(Uuid),
    Connection(Uuid),
    Group(Uuid),
}

/// What a click on the canvas does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DiagramTool {
    /// Pick, and drag a block to move it.
    #[default]
    Select,
    /// Pick a block or port, then another, to connect them.
    Connect,
}

/// Where the canvas looks: `zoom` 1 fits `home`, `pan` moves the view
/// centre from the centre of `home`, in mm.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    pub zoom: f64,
    pub pan: (f64, f64),
    /// The box the view is fitted to, mm: x, y, width, height. `None`
    /// follows the diagram's extent (a fresh view, or Fit); the first pan,
    /// zoom or edit holds it, so an edit never moves the view.
    pub home: Option<[f64; 4]>,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan: (0.0, 0.0),
            home: None,
        }
    }
}

/// The box an empty diagram is fitted as, mm.
const EMPTY_HOME_MM: [f64; 4] = [0.0, 0.0, 120.0, 80.0];

impl Camera {
    /// The box the view fits for `diagram`: the held one, else the
    /// diagram's extent (a box at the origin while it is empty).
    pub fn home_for(&self, diagram: &BlockDiagram) -> [f64; 4] {
        self.home.unwrap_or_else(|| {
            diagram.extent().map_or(EMPTY_HOME_MM, |b| {
                [b.x, b.y, b.width.max(1.0), b.height.max(1.0)]
            })
        })
    }

    /// Hold the box the view fits now, so an edit that grows the diagram
    /// leaves the view where it is.
    pub fn hold(&mut self, diagram: &BlockDiagram) {
        self.home = Some(self.home_for(diagram));
    }
}

/// A Properties field typed as text, committed on Enter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PropertyField {
    BlockName,
    BlockFunction,
    BlockNote,
    BlockRails,
    BlockSpecs,
    BlockX,
    BlockY,
    BlockWidth,
    BlockHeight,
    ConnectionLabel,
    ConnectionInterface,
    GroupName,
}

/// One open diagram: the document and its history. What the tab shows of
/// it (selection, tool, camera, pane, popovers, the Re-sync review) is its
/// [`BlockDiagramViewState`](super::block_diagram_view::BlockDiagramViewState),
/// kept with the GUI state (`DocumentViewState::block_diagrams`).
#[derive(Debug, Clone)]
pub struct BlockDiagramEditor {
    pub diagram: BlockDiagram,
    /// The token the file was read with; a newer minor makes the diagram
    /// read-only.
    pub token: String,
    /// The diagram as the file holds it; `None` for a diagram never saved.
    pub saved: Option<BlockDiagram>,
    /// Earlier states, the newest last.
    pub undo: Vec<BlockDiagram>,
    /// States undone, the newest last.
    pub redo: Vec<BlockDiagram>,
}

impl BlockDiagramEditor {
    /// A diagram read from its file. A minted id is not an edit: the
    /// diagram reads clean and the next save writes the id.
    pub fn opened(read: ReadBlockDiagram) -> Self {
        let mut editor = Self::created(read.diagram);
        editor.saved = Some(editor.diagram.clone());
        editor.token = read.token;
        editor
    }

    /// A diagram made in memory, never saved.
    pub fn created(diagram: BlockDiagram) -> Self {
        Self {
            diagram,
            token: signex_types::block_diagram::SNXBLK_FORMAT_V1.to_string(),
            saved: None,
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    /// From a newer minor of the format: shown, never saved.
    pub fn is_read_only(&self) -> bool {
        SNXBLK_VERSION.is_newer_minor(&self.token)
    }

    /// Never saved: its file may not exist, and a save must not replace one
    /// that does.
    pub fn is_new(&self) -> bool {
        self.saved.is_none()
    }

    /// Holds edits the file does not.
    pub fn is_dirty(&self) -> bool {
        self.saved.as_ref() != Some(&self.diagram)
    }

    /// Keep `before` as an undo step, at most `depth` of them, and drop the
    /// redo steps.
    pub fn record(&mut self, before: BlockDiagram, depth: usize) {
        self.undo.push(before);
        let extra = self.undo.len().saturating_sub(depth.max(1));
        self.undo.drain(..extra);
        self.redo.clear();
    }

    /// One undo (or redo) step; `false` with none to take. The tab's view
    /// prunes what the step took away (`BlockDiagramViewState::prune`).
    pub fn step(&mut self, undo: bool) -> bool {
        let (from, to) = if undo {
            (&mut self.undo, &mut self.redo)
        } else {
            (&mut self.redo, &mut self.undo)
        };
        let Some(state) = from.pop() else {
            return false;
        };
        to.push(std::mem::replace(&mut self.diagram, state));
        true
    }
}

/// Whether `diagram` still has `item`.
pub fn has_item(diagram: &BlockDiagram, item: DiagramItem) -> bool {
    match item {
        DiagramItem::Block(id) => diagram.block(id).is_some(),
        DiagramItem::Port(id) => diagram.blocks.iter().any(|b| b.port(id).is_some()),
        DiagramItem::Connection(id) => diagram.connection(id).is_some(),
        DiagramItem::Group(id) => diagram.group(id).is_some(),
    }
}
