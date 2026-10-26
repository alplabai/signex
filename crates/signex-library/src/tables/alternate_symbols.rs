//! The optional trailing `alternate_symbols` column of a component table.

use crate::adapter::LibraryError;
use crate::alternate_symbol::AlternateSymbol;
use crate::component::ComponentRow;

use super::{TABLE_HEADER, from_json_cell, json_cell};

/// Optional trailing column holding a row's `alternate_symbols` as JSON.
pub const ALTERNATE_SYMBOLS_COLUMN: &str = "alternate_symbols";

/// Check a table header against the legacy columns, with or without the
/// optional trailing one. `Err` carries the mismatch detail.
pub(crate) fn check_columns(columns: &[&str]) -> Result<(), String> {
    let width = columns.len();
    let with_alternates = width == TABLE_HEADER.len() + 1;
    if width != TABLE_HEADER.len() && !with_alternates {
        return Err(format!(
            "{width} columns, expected {} or {}",
            TABLE_HEADER.len(),
            TABLE_HEADER.len() + 1
        ));
    }
    let extra = with_alternates.then_some(ALTERNATE_SYMBOLS_COLUMN);
    for (&got, want) in columns
        .iter()
        .zip(TABLE_HEADER.iter().copied().chain(extra))
    {
        if got != want {
            return Err(format!("column {got:?}, expected {want:?}"));
        }
    }
    Ok(())
}

/// JSON cell for `row.alternate_symbols`; `None` when there are none.
pub(crate) fn alternate_symbols_cell(row: &ComponentRow) -> Result<Option<String>, LibraryError> {
    if row.alternate_symbols.is_empty() {
        return Ok(None);
    }
    json_cell(&row.alternate_symbols).map(Some)
}

/// Parse an `alternate_symbols` cell; empty = none.
pub(super) fn alternate_symbols_from_cell(s: &str) -> Result<Vec<AlternateSymbol>, LibraryError> {
    if s.is_empty() {
        return Ok(Vec::new());
    }
    from_json_cell(s, ALTERNATE_SYMBOLS_COLUMN)
}
