//! Placement extras on a sheet (#1667), carried under `EditMsg::Extras`:
//! Autoplace Fields, Copy Attributes, Place Design Block and Import Graphics.

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
}
