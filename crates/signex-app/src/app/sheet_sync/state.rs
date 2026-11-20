//! What the Synchronize Sheet Entries and Ports dialog holds between
//! messages: the sheet symbol it syncs, the child sheet read for it, the
//! preview of each direction and why Apply refused, if it did.

use std::path::PathBuf;
use std::sync::Arc;

use signex_engine::{SheetSyncDirection, SheetSyncPreview};
use signex_types::schematic::SchematicSheet;
use uuid::Uuid;

use crate::library::closed_sheets::FileStamp;

/// A child sheet no engine held, read off the UI thread, and its file stamp
/// at that time: Apply loads it into a tabless engine only while the file
/// is unchanged.
#[derive(Debug)]
pub struct ChildRead {
    pub sheet: SchematicSheet,
    pub stamp: Option<FileStamp>,
}

/// The changes of both directions, worked out together so a switch of
/// direction needs no read.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SheetSyncPreviews {
    pub to_symbol: SheetSyncPreview,
    pub to_sheet: SheetSyncPreview,
}

/// Why the dialog shows no preview, or Apply changed nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SheetSyncProblem {
    /// The child sheet could not be read.
    ReadFailed(String),
    /// The parent sheet, the sheet symbol or the set of open sheets changed.
    Stale,
    /// The sheets changed since the preview; it now shows the current
    /// changes.
    Changed,
    /// A sheet that is not open is being saved or moved.
    Busy(PathBuf),
    /// A sheet opens read-only.
    ReadOnly(PathBuf),
    /// A sheet that is not open could not be loaded.
    NotLoaded(PathBuf),
    /// A sheet refused the change.
    NotChanged(PathBuf),
    /// A closed child sheet changed on disk after it was read.
    DiskChanged(PathBuf),
}

/// The open dialog.
#[derive(Debug)]
pub struct SheetSyncState {
    /// Which read of the child sheet this dialog waits for.
    pub generation: u64,
    /// The sheet the sheet symbol is on: the sheet the dialog was asked on,
    /// the active one or an undocked window's. Apply edits this sheet.
    pub parent: PathBuf,
    pub symbol: Uuid,
    pub symbol_name: String,
    /// The symbol's file reference, and the file it resolves to.
    pub child_filename: String,
    pub child_path: PathBuf,
    /// `DocumentState::documents_epoch` when the dialog opened: an engine
    /// that came or went since makes the preview stale.
    pub epoch: u64,
    pub direction: SheetSyncDirection,
    /// The closed child sheet, once read; `None` while it is read and when
    /// the child is open (its live document is used instead).
    pub read: Option<Arc<ChildRead>>,
    /// `None` until the child sheet is known.
    pub previews: Option<SheetSyncPreviews>,
    /// Apply is re-checking the closed child sheet's file.
    pub checking: bool,
    pub problem: Option<SheetSyncProblem>,
}

impl SheetSyncState {
    /// The changes of the chosen direction, once known.
    pub fn preview(&self) -> Option<&SheetSyncPreview> {
        self.previews.as_ref().map(|previews| match self.direction {
            SheetSyncDirection::SheetToSymbol => &previews.to_symbol,
            SheetSyncDirection::SymbolToSheet => &previews.to_sheet,
        })
    }

    /// Apply has something to do and is not already waiting.
    pub fn can_apply(&self) -> bool {
        !self.checking && self.preview().is_some_and(|preview| !preview.is_empty())
    }
}
