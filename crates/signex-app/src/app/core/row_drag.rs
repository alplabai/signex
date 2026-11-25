//! A Sheets or Libraries row held by its grip: plain state, UI-free (ADR-0006).

use std::path::PathBuf;

/// The row being dragged, by what the project stores for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowKey {
    Sheet(String),
    Library(PathBuf),
}

/// A row held by its grip, and the row the pointer is over.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowDrag {
    pub(crate) key: RowKey,
    pub(crate) over: Option<usize>,
}
