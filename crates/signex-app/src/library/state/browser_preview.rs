//! Why the Library Browser preview pane (mockup row 118) has no drawing of
//! the selected row. Typed, so a surface that shows it maps each case to a
//! catalog message; only the path and the reader's or system's text are
//! user content.

use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrowserPreviewError {
    /// The row's library is not open, so nothing can read it.
    LibraryNotOpen(PathBuf),
    /// The library reader's own reason (the Components panel's loader).
    Unreadable(String),
    /// The background read stopped (the runtime's own text).
    Interrupted(String),
}
