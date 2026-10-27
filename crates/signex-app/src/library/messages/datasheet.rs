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
    /// Async result of the Pinned-PDF upload. Boxed: it is the largest
    /// datasheet message, and every `Message` is as large as its largest
    /// variant.
    UploadResult(Box<PickedPdf>),
    /// The staged PDFs of a row save were written to the library on a
    /// worker: the paths (relative to the library root) on success, or
    /// why a write failed. The row is saved and committed next.
    AssetsStored(Result<Vec<String>, String>),
    /// Append an empty URL document.
    Add,
    /// Remove a document; removing the primary promotes the next one.
    Remove(usize),
    /// Move a document to another position; moving to 0 makes it the
    /// primary.
    Move { from: usize, to: usize },
}

/// What the Pinned-PDF dialog returned, and for which document.
#[derive(Debug, Clone)]
pub struct PickedPdf {
    /// The document's index when the dialog opened.
    pub index: usize,
    /// The document as it was when the dialog opened: the list may have
    /// changed while the dialog was open, so the result re-finds its
    /// document by it.
    pub picked_for: DatasheetDoc,
    /// `None` on cancel; else the file the worker read, validated and
    /// hashed, or why it was refused.
    pub file: Option<Result<PickedFile, String>>,
}

/// A PDF the file dialog returned: read, checked for the `%PDF` header and
/// hashed on a worker. Held on the editor, unwritten, until the row is saved.
#[derive(Debug, Clone)]
pub struct PickedFile {
    pub asset: signex_library::PdfAsset,
    /// The file's name, shown as the pinned document's label.
    pub filename: String,
}
