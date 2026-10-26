//! Messages of the Component Preview's Datasheet tab (#1231).
//!
//! Each document is addressed by its index in `row.documents()`; index 0
//! is the primary.

use signex_library::DatasheetDoc;

use crate::library::editor::datasheet_picker::DatasheetMode;

#[derive(Debug, Clone)]
pub enum DatasheetMsg {
    /// Switch a document between URL / Pinned PDF modes.
    SetMode(usize, DatasheetMode),
    /// Live edit of a document's URL field.
    SetUrl(usize, String),
    /// Live edit of a document's label ("Errata", "App note", …).
    SetLabel(usize, String),
    /// Open the Pinned-PDF upload dialog for a document.
    UploadDialog(usize),
    /// Async result of the Pinned-PDF upload. `picked_for` is the document
    /// as it was when the dialog opened: the list may have changed while
    /// the dialog was open, so the result re-finds its document by it.
    /// `file` is `Some((bytes, filename))` on pick, `None` on cancel.
    UploadResult {
        index: usize,
        picked_for: DatasheetDoc,
        file: Option<(Vec<u8>, String)>,
    },
    /// Append an empty URL document.
    Add,
    /// Remove a document; removing the primary promotes the next one.
    Remove(usize),
    /// Move a document to another position; moving to 0 makes it the
    /// primary.
    Move { from: usize, to: usize },
}
