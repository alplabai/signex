//! The block diagram document tab (mockup row 171, #2796) over the UI-free
//! `signex_types::block_diagram` model: blocks with their ports, the
//! signal-flow connections between them with a protocol label, groups drawn
//! as frames, the toolbar, and Properties for what is selected. Every edit
//! goes through `edit.rs`, which the `block_*` commands (`commands.rs`) run
//! too. Reads and writes run in tasks (`io.rs`).
//!
//! The Power tree view, Re-sync from the sheet hierarchy and the exports
//! of mockup row 171 belong to the other units of the Block Diagram band.

mod canvas;
mod commands;
mod edit;
mod fields;
mod io;
mod query;
mod refusal;
mod tab;
mod update;
mod view;

#[cfg(test)]
mod command_tests;
#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};

use signex_types::block_diagram::{BlockDiagram, BlockShape, Endpoint, ReadBlockDiagram};
use signex_types::schematic::Point;
use signex_types::theme::Color;
use uuid::Uuid;

pub(crate) use commands::{
    block_add, block_group, block_group_set, block_link, block_link_set, block_port_add,
    block_port_remove, block_remove, block_set, block_ungroup, block_unlink, new_block_diagram,
    save_by_command, step_by_command,
};
// The state is core-bound (ADR-0006), so it lives in the application core.
use crate::app::core::block_diagram_editor as state;
pub use io::FileFailure;
pub use state::{
    BlockDiagramEditor, BlockDiagrams, Camera, DiagramItem, DiagramTool, PropertyField,
};

pub use signex_types::block_diagram::BLOCK_DIAGRAM_EXTENSION;

/// Whether `path` names a block diagram file, in any letter case.
pub fn is_block_diagram_path(path: &Path) -> bool {
    crate::app::file_extension::has_extension(path, BLOCK_DIAGRAM_EXTENSION)
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
        result: Result<Box<ReadBlockDiagram>, FileFailure>,
    },
    /// The write of `path` finished; `written` is the diagram it wrote.
    Saved {
        path: PathBuf,
        written: Box<BlockDiagram>,
        result: Result<(), FileFailure>,
    },
    /// Something happened on the tab of the diagram at `path`.
    Tab { path: PathBuf, event: TabEvent },
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
}
