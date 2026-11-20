//! Why the Library Browser preview pane (mockup row 118) has no drawing of
//! the selected row. Typed, so a surface that shows it maps each case to a
//! catalog message; only the path, the table, the row id and the reader's
//! or runtime's own error text are kept, never a sentence of ours.

use std::path::PathBuf;

use crate::panels::components_panel::details::load::RowReadError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrowserPreviewError {
    /// The row's library is not open, so nothing can read it.
    LibraryNotOpen(PathBuf),
    /// The library reader could not read the row.
    Unreadable(RowReadError),
    /// The background read stopped (the runtime's own text).
    Interrupted(String),
}
