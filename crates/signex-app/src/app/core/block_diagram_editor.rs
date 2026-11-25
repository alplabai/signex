//! The block diagram tab's state (mockup row 171): every open diagram by
//! its path, with its undo and redo steps, the selection, the armed tool,
//! the view and the Properties text being typed. Pure data and pure rules;
//! the reads and writes run in `app::block_diagram`, off the update thread.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use signex_types::block_diagram::{BlockDiagram, Endpoint, ReadBlockDiagram, SNXBLK_VERSION};
use uuid::Uuid;

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

/// One open diagram.
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
    pub selection: Vec<DiagramItem>,
    pub tool: DiagramTool,
    /// The first end picked with the Connect tool.
    pub connect_from: Option<Endpoint>,
    pub camera: Camera,
    /// Properties text typed and not committed yet.
    pub drafts: BTreeMap<PropertyField, String>,
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
            selection: Vec::new(),
            tool: DiagramTool::Select,
            connect_from: None,
            camera: Camera::default(),
            drafts: BTreeMap::new(),
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

    /// One undo (or redo) step; `false` with none to take.
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
        self.prune_selection();
        true
    }

    /// Drop the selected objects and the connect start the diagram no
    /// longer has, and the drafts typed for them.
    pub fn prune_selection(&mut self) {
        let diagram = &self.diagram;
        let before = self.selection.clone();
        self.selection.retain(|item| has_item(diagram, *item));
        if self.selection != before {
            self.drafts.clear();
        }
        let start_gone = self.connect_from.is_some_and(|end| {
            diagram
                .block(end.block)
                .is_none_or(|b| end.port.is_some_and(|p| b.port(p).is_none()))
        });
        if start_gone {
            self.connect_from = None;
        }
    }

    /// The one selected object, when exactly one is.
    pub fn single_selection(&self) -> Option<DiagramItem> {
        match self.selection.as_slice() {
            [item] => Some(*item),
            _ => None,
        }
    }

    /// The selected blocks, in selection order.
    pub fn selected_blocks(&self) -> Vec<Uuid> {
        self.selection
            .iter()
            .filter_map(|item| match item {
                DiagramItem::Block(id) => Some(*id),
                _ => None,
            })
            .collect()
    }

    /// Pick `item`, or with `add` add it to the selection or take it out.
    pub fn pick(&mut self, item: Option<DiagramItem>, add: bool) {
        self.drafts.clear();
        match (item, add) {
            (None, false) => self.selection.clear(),
            (None, true) => {}
            (Some(item), false) => self.selection = vec![item],
            (Some(item), true) => {
                if let Some(at) = self.selection.iter().position(|i| *i == item) {
                    self.selection.remove(at);
                } else {
                    self.selection.push(item);
                }
            }
        }
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
