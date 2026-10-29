//! What an off-thread symbol / footprint container save reports back (#859).
//!
//! The UI thread snapshots the editor's container and asks for a write;
//! these types carry the outcome to `update`, which decides from the
//! editor's own dirty flag whether the draft is still unsaved.

use std::collections::HashMap;
use std::path::PathBuf;

use signex_library::{FootprintFile, RowId, SymbolFile};
use uuid::Uuid;

/// The container as the write left it, versions bumped.
#[derive(Debug, Clone)]
pub enum WrittenContainer {
    Symbol(SymbolFile),
    Footprint(FootprintFile),
}

/// A container save that reached disk.
#[derive(Debug, Clone)]
pub struct PrimitiveWritten {
    pub file: WrittenContainer,
    /// The version each primitive was bumped to by the save.
    pub versions: HashMap<Uuid, String>,
    /// Message of the project git commit queued once the file is on disk.
    pub commit_message: String,
    /// Component rows the version cascade moved onto the saved version
    /// (#832); their open Component Previews hold stale content (#1072).
    pub cascaded_rows: Vec<RowId>,
    /// Saved primitives whose rows the cascade could not move and a retry
    /// can: the editor keeps them and the next Save cascades them again.
    pub cascade_owed: Vec<Uuid>,
}

/// Where a Save As came from: the old path and tab title, to put back if
/// the write fails.
#[derive(Debug, Clone)]
pub struct SaveAsOrigin {
    pub from: PathBuf,
    pub title: Option<String>,
}

/// Outcome of one container save, carried back to `update`.
#[derive(Debug, Clone)]
pub struct PrimitiveSaveDone {
    pub path: PathBuf,
    /// The stamp the editor got when the snapshot was taken; a completion
    /// for an editor since replaced (discarded and reopened, History
    /// restore) leaves the new one alone.
    pub generation: u64,
    pub save_as: Option<SaveAsOrigin>,
    pub result: Result<PrimitiveWritten, String>,
}
