//! What the Synchronize Sheet Entries and Ports dialog holds between
//! messages: the sheet symbol it syncs, the child sheet read for it, the
//! preview of each direction and why Apply refused, if it did.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

use signex_engine::{SheetEntryChange, SheetSyncDirection, SheetSyncEntry, SheetSyncPreview};
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

/// The written side of each direction, entry by entry, for the drawing and
/// the change table. Kept apart from [`SheetSyncPreviews`], which alone
/// decides whether Apply still matches what was shown.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SheetSyncEntries {
    pub to_symbol: Vec<SheetSyncEntry>,
    pub to_sheet: Vec<SheetSyncEntry>,
}

/// Which state the dialog's drawing shows.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SheetSyncPreviewMode {
    #[default]
    After,
    Before,
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
    /// `None` until the child sheet is known, like `previews`.
    pub entries: Option<SheetSyncEntries>,
    /// Ports Symbol to Sheet would remove whose tick is off: Apply keeps
    /// them. Every removal starts ticked.
    pub kept_ports: BTreeSet<String>,
    pub preview_mode: SheetSyncPreviewMode,
    pub preview_expanded: bool,
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

    /// The written side of the chosen direction, once known.
    pub fn entries(&self) -> Option<&[SheetSyncEntry]> {
        self.entries.as_ref().map(|entries| match self.direction {
            SheetSyncDirection::SheetToSymbol => entries.to_symbol.as_slice(),
            SheetSyncDirection::SymbolToSheet => entries.to_sheet.as_slice(),
        })
    }

    /// What Apply makes in the chosen direction: the preview without the
    /// removals left out.
    pub fn applied_preview(&self) -> Option<SheetSyncPreview> {
        let previews = self.previews.as_ref()?;
        Some(match self.direction {
            SheetSyncDirection::SheetToSymbol => previews.to_symbol.clone(),
            SheetSyncDirection::SymbolToSheet => previews.to_sheet.leaving_out(&self.kept_ports),
        })
    }

    /// Apply has something to do and is not already waiting.
    pub fn can_apply(&self) -> bool {
        !self.checking
            && self
                .applied_preview()
                .is_some_and(|preview| !preview.is_empty())
    }

    /// The change table carries a tick per removal: Symbol to Sheet with
    /// ports to remove.
    pub fn shows_removal_ticks(&self) -> bool {
        self.direction == SheetSyncDirection::SymbolToSheet
            && self
                .previews
                .as_ref()
                .is_some_and(|previews| !previews.to_sheet.removed.is_empty())
    }

    /// `entry` is a removal the user left out.
    pub fn is_left_out(&self, entry: &SheetSyncEntry) -> bool {
        self.direction == SheetSyncDirection::SymbolToSheet
            && entry.change == SheetEntryChange::Removed
            && self.kept_ports.contains(&entry.name)
    }

    /// The ports Apply removes from the sheet, for the warning.
    pub fn ports_to_remove(&self) -> Vec<String> {
        match self.direction {
            SheetSyncDirection::SheetToSymbol => Vec::new(),
            SheetSyncDirection::SymbolToSheet => self
                .applied_preview()
                .map_or_else(Vec::new, |preview| preview.removed),
        }
    }

    /// The written side as the drawing shows it: a removal left out stays.
    pub fn drawn_entries(&self) -> Option<Vec<SheetSyncEntry>> {
        let entries = self.entries()?;
        Some(
            entries
                .iter()
                .map(|entry| SheetSyncEntry {
                    change: if self.is_left_out(entry) {
                        SheetEntryChange::Kept
                    } else {
                        entry.change.clone()
                    },
                    ..entry.clone()
                })
                .collect(),
        )
    }

    /// The tick of the removal `port` was set. Ignored for a name that is no
    /// removal, and while Apply is waiting on the file check, so what was
    /// confirmed is what is written.
    pub fn set_removal_ticked(&mut self, port: &str, ticked: bool) {
        let listed = self
            .previews
            .as_ref()
            .is_some_and(|previews| previews.to_sheet.removed.iter().any(|name| name == port));
        if self.checking || !listed {
            return;
        }
        if ticked {
            self.kept_ports.remove(port);
        } else {
            self.kept_ports.insert(port.to_string());
        }
    }
}
