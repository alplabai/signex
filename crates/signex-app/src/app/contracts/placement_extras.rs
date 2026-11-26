//! Placement extras on a sheet (#1667), carried under `EditMsg::Extras`:
//! Autoplace Fields, Copy Attributes, Place Design Block, Import Graphics,
//! and the snippets and templates kept for reuse (#1283).

use std::path::PathBuf;
use std::sync::Arc;

use signex_types::schematic::SchematicSheet;

#[derive(Debug, Clone)]
pub enum PlacementExtrasMsg {
    /// Put the fields of the selected parts where the autoplacer puts them.
    AutoplaceFieldsRequested,
    /// Arm placing another of the object at `at` (world mm; `None`: under
    /// the cursor) with its look: a part, a label or port, or a text note.
    CopyAttributesRequested { at: Option<(f64, f64)> },
    /// Ask for the sheet file to place as a design block.
    DesignBlockRequested,
    /// The design block dialog answered; `None` when it was cancelled.
    DesignBlockPicked(Option<PathBuf>),
    /// A design block file was read off the UI thread for the sheet
    /// `target`, asked for at `epoch`.
    DesignBlockRead {
        path: PathBuf,
        target: PathBuf,
        epoch: u64,
        sheet: Result<Arc<SchematicSheet>, String>,
    },
    /// Ask for the DXF file to import.
    ImportGraphicsRequested,
    /// The import dialog answered; `None` when it was cancelled.
    ImportGraphicsPicked(Option<PathBuf>),
    /// A DXF file was read and turned into drawings off the UI thread.
    GraphicsRead {
        path: PathBuf,
        target: PathBuf,
        epoch: u64,
        graphics: Result<Arc<signex_engine::ImportedGraphics>, String>,
    },
    /// `place_snippet` ran without operands (#1283): pick a snippet file,
    /// starting in the project's snippets folder.
    SnippetRequested,
    /// `save_snippet` ran without operands: the selection is saved as a
    /// snippet of the project, named after its designators.
    SnippetSaveRequested,
    /// A snippet file was written, or why it was not; `waiting` is the
    /// command run with operands that waits for it.
    SnippetWritten {
        saved: crate::app::design_reuse::SavedSnippet,
        result: Result<(), String>,
        waiting: Option<crate::app::agent::WaitingCommand>,
    },
    /// The folders `list_snippets` or `list_templates` reads were listed off
    /// the UI thread.
    ReuseListed {
        waiting: crate::app::agent::WaitingCommand,
        answer: crate::app::design_reuse::ListAnswer,
        listing: Arc<signex_types::reuse::ReuseListing>,
    },
}
