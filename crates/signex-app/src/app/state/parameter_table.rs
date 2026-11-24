//! Transient state of the Parameter Manager table.
//!
//! Cell text is never held here: every keystroke is written to the document
//! at once, and the engine folds the keystrokes of one cell into one undo
//! step, so no typed text lives outside the document. The session keeps
//! only each edited cell's value from before its first edit, for the
//! changed-cell marks and Revert changes.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default)]
pub struct ParameterTableState {
    /// The sheet the last cell edit went to; a different active sheet ends
    /// the cell-typing run.
    pub run_path: Option<PathBuf>,
    /// Name typed into the "new parameter" box.
    pub new_key: String,
    /// Case-insensitive row filter over reference / value / footprint.
    pub filter: String,
    /// The cells edited since the dialog opened. Cleared on open and close.
    pub session: ParameterSession,
    /// The parameter column whose header the pointer is over: its remove
    /// button shows only then.
    pub hovered_column: Option<String>,
    /// The symbol whose row holds the focused cell: the row being edited,
    /// drawn tinted (mockup row 70).
    pub editing_row: Option<uuid::Uuid>,
}

/// One cell of the table: a parameter of a symbol on a sheet.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ParameterCell {
    pub sheet: Option<PathBuf>,
    pub symbol: uuid::Uuid,
    pub key: String,
}

/// The cells edited in this opening of the dialog, each with its value from
/// before the first edit (`None`: the symbol had no such parameter).
#[derive(Debug, Clone, Default)]
pub struct ParameterSession {
    originals: BTreeMap<ParameterCell, Option<String>>,
}

impl ParameterSession {
    /// Note an edit of `cell`; only the first edit's `before` is kept.
    pub fn record(&mut self, cell: ParameterCell, before: Option<String>) {
        self.originals.entry(cell).or_insert(before);
    }

    /// The edited cells of `sheet` with their original values.
    pub fn cells_on<'a>(
        &'a self,
        sheet: Option<&'a Path>,
    ) -> impl Iterator<Item = (&'a ParameterCell, &'a Option<String>)> + 'a {
        self.originals
            .iter()
            .filter(move |(cell, _)| cell.sheet.as_deref() == sheet)
    }

    /// The value `cell` had before its first edit this session; `None` when
    /// it was not edited.
    pub fn original(&self, cell: &ParameterCell) -> Option<&Option<String>> {
        self.originals.get(cell)
    }
}
