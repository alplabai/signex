//! The block diagram document tab (mockup row 171, #2796) over the UI-free
//! `signex_types::block_diagram` model: blocks with their ports, the
//! signal-flow connections between them with a protocol label, groups drawn
//! as frames, the toolbar, and Properties for what is selected. Every edit
//! goes through `edit.rs`, which the `block_*` commands (`commands.rs`) run
//! too. Reads and writes run in tasks (`io.rs`); a command naming a
//! diagram not open reads it first (`on_demand.rs`). The tab's view switch
//! shows the project's Power tree (`power.rs`); its toolbar re-syncs the
//! diagram from the sheet hierarchy and exports it (`export.rs`).

mod canvas;
mod canvas_review;
mod commands;
mod edit;
mod export;
mod fields;
mod io;
mod on_demand;
mod power;
mod query;
mod refusal;
mod review;
mod tab;
mod update;
mod view;

#[cfg(test)]
mod command_tests;
#[cfg(test)]
mod on_demand_tests;
#[cfg(test)]
mod page_tests;
#[cfg(test)]
mod tab_tests;
#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) use canvas_review::{Preview, previews};
pub use on_demand::CommandRerun;
pub use power::PowerTreeLanded;
pub(crate) use power::PowerTreeRun;
pub use review::ReviewEvent;

use std::path::{Path, PathBuf};

use signex_types::block_diagram::{BlockDiagram, BlockShape, Endpoint};
use signex_types::schematic::Point;
use signex_types::theme::Color;
use uuid::Uuid;

use crate::persist::ContentHash;

pub(crate) use commands::{
    block_add, block_group, block_group_set, block_link, block_link_set, block_port_add,
    block_port_remove, block_remove, block_set, block_ungroup, block_unlink, new_block_diagram,
    save_by_command, step_by_command,
};
// The state is core-bound (ADR-0006), so it lives in the application core.
use crate::app::core::block_diagram_editor as state;
pub use io::{DiagramFile, FileFailure};
pub use state::{
    BlockDiagramEditor, BlockDiagramViewState, BlockDiagrams, Camera, DiagramItem, DiagramTool,
    DiagramView, ExportKind, ExportPage, ExportPaper, PropertyField, ResyncReview,
};

/// Whether `link` is a ground connection, which Show GND hides: a power
/// connection whose label, or the net it was made from, is a ground.
pub(crate) fn is_ground_link(link: &signex_types::block_diagram::Connection) -> bool {
    use signex_net::power_tree::is_ground_net;
    link.interface == signex_types::block_diagram::Interface::Power
        && (is_ground_net(&link.label) || link.derived_from.as_deref().is_some_and(is_ground_net))
}

pub use signex_types::block_diagram::BLOCK_DIAGRAM_EXTENSION;

/// Whether `path` names a block diagram file, in any letter case.
pub fn is_block_diagram_path(path: &Path) -> bool {
    crate::app::file_extension::has_extension(path, BLOCK_DIAGRAM_EXTENSION)
}

/// The open project a diagram belongs to: the one whose folder holds it
/// (diagrams are not listed in `.snxprj`).
pub(crate) fn project_holding<'a>(
    docs: &'a crate::app::state::DocumentState,
    path: &Path,
) -> Option<&'a crate::app::state::LoadedProject> {
    docs.projects
        .iter()
        .find(|p| crate::app::state::scope::path_is_under(path, p.dir(), p.case_fold))
}

/// The file name of `path`, for messages.
pub(crate) fn file_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}

impl From<BlockDiagramMsg> for crate::app::Message {
    fn from(msg: BlockDiagramMsg) -> Self {
        Self::BlockDiagram(msg)
    }
}

/// What happened to a block diagram. Namespaced under
/// `Message::BlockDiagram`, routed to `dispatch_block_diagram_message`.
#[derive(Debug, Clone)]
pub enum BlockDiagramMsg {
    /// File ▸ New ▸ Block Diagram.
    NewRequested,
    /// The read of `path` finished.
    Read {
        path: PathBuf,
        result: Result<Box<DiagramFile>, FileFailure>,
    },
    /// The write of `path` finished; `written` is the diagram it wrote and
    /// `hash` the fingerprint of its bytes.
    Saved {
        path: PathBuf,
        written: Box<BlockDiagram>,
        hash: ContentHash,
        result: Result<(), FileFailure>,
    },
    /// The read a command waited for finished (`on_demand`).
    ReadForCommand {
        path: PathBuf,
        result: Result<Box<DiagramFile>, FileFailure>,
        rerun: Box<CommandRerun>,
    },
    /// The power tree run for the diagram at `path` finished.
    PowerTreeRead(Box<PowerTreeLanded>),
    /// Something happened on the tab of the diagram at `path`.
    Tab { path: PathBuf, event: TabEvent },
}

/// What happened in the export popover.
#[derive(Debug, Clone, PartialEq)]
pub enum ExportEvent {
    /// The toolbar's SVG or PDF… opened the popover for that format.
    Opened(ExportKind),
    KindPicked(ExportKind),
    FileEdited(String),
    /// Print colours (black) or as shown.
    MonochromePicked(bool),
    TitleBlockToggled(bool),
    /// A page ticked (`true`) or unticked in Pages.
    PageToggled(ExportPage, bool),
    PaperPicked(ExportPaper),
    /// Portrait (`true`) or landscape.
    PortraitPicked(bool),
    Cancelled,
    ExportPressed,
}

/// What happened on a diagram's tab: its toolbar, canvas or Properties.
#[derive(Debug, Clone, PartialEq)]
pub enum TabEvent {
    ToolPicked(DiagramTool),
    AddBlockPressed,
    AddPortPressed,
    GroupPressed,
    DeletePressed,
    UndoPressed,
    RedoPressed,
    SavePressed,
    FitPressed,
    /// A click picked `item` (`None`: empty canvas); `add` with Shift held.
    ItemPicked {
        item: Option<DiagramItem>,
        add: bool,
    },
    /// The Connect tool picked a block or a port.
    EndPicked(Endpoint),
    /// A block was dragged and let go with its top-left corner at `to`.
    BlockMoved {
        block: Uuid,
        to: Point,
    },
    /// A block was double-clicked: its linked sheet opens.
    BlockOpened(Uuid),
    ViewChanged(Camera),
    FieldEdited(PropertyField, String),
    /// Enter in a Properties field.
    FieldCommitted(PropertyField),
    ShapePicked(BlockShape),
    /// A known interface was picked for the selected connection, by token.
    InterfacePicked(&'static str),
    /// The selected block was put in a group, or taken out of its group.
    GroupPicked(Option<Uuid>),
    /// A swatch for the selected block or group; `None` is the theme's.
    ColorPicked(Option<Color>),
    /// The view switch: Blocks or Power tree.
    ViewPicked(DiagramView),
    /// A rail picked in the Power tree view.
    RailPicked(String),
    /// A part clicked in the Power tree view, by its place in the run's
    /// topology: its sheet opens with it selected.
    PartOpened(usize),
    /// The Power tree's Variant field.
    VariantPicked(String),
    /// Show GND on the Block diagram view.
    ShowGroundToggled(bool),
    /// The Re-sync review side sheet (mockup row 210).
    Review(ReviewEvent),
    /// Re-sync: the diagram rebuilt from the sheet hierarchy, layout kept.
    ResyncPressed,
    Export(ExportEvent),
}

impl crate::app::Signex {
    /// What the tab shows of the diagram at `path` while it is open (the
    /// view is held from the moment the diagram is); `None` when it is not.
    pub(crate) fn open_diagram_view(&self, path: &Path) -> Option<&BlockDiagramViewState> {
        if !self
            .document_state
            .block_diagrams
            .editors
            .contains_key(path)
        {
            return None;
        }
        self.document_view.block_diagrams.get(path)
    }

    /// [`Self::open_diagram_view`], to change.
    pub(crate) fn open_diagram_view_mut(
        &mut self,
        path: &Path,
    ) -> Option<&mut BlockDiagramViewState> {
        (self.document_state.block_diagrams.editors)
            .contains_key(path)
            .then(|| view_of(&mut self.document_view.block_diagrams, path))
    }

    /// What the tab shows of the diagram at `path`, held from now on.
    pub(crate) fn diagram_view_mut(&mut self, path: &Path) -> &mut BlockDiagramViewState {
        view_of(&mut self.document_view.block_diagrams, path)
    }

    /// The diagram at `path` and its tab's view, while it is open.
    pub(crate) fn open_diagram(&self, path: &Path) -> Option<OpenDiagram<'_>> {
        Some(OpenDiagram {
            doc: self.document_state.block_diagrams.editors.get(path)?,
            tab: self.document_view.block_diagrams.get(path)?,
        })
    }
}

/// The view held for `path` in `views`, made fresh when none is: for an
/// update that holds the diagram (`DocumentState`) and its view
/// (`DocumentViewState`) at once.
pub(crate) fn view_of<'a>(
    views: &'a mut std::collections::BTreeMap<PathBuf, BlockDiagramViewState>,
    path: &Path,
) -> &'a mut BlockDiagramViewState {
    views.entry(path.to_path_buf()).or_default()
}

/// One open diagram as its tab draws it: the document and the tab's view of
/// it, held in `DocumentState` and `DocumentViewState`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct OpenDiagram<'a> {
    pub doc: &'a BlockDiagramEditor,
    pub tab: &'a BlockDiagramViewState,
}
