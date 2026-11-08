//! What documents exist and what each one is, by [`DocumentId`] (#2172).
//!
//! The id of a document is stored in the document itself (see
//! `docs/DOCUMENT_IDS.md`); this module reads it from wherever the document
//! is held, keeps the one place that assigns a missing or repeated id
//! ([`reconcile`]), and lists every document with its metadata
//! ([`entries`]). Nothing here reads UI state: not the active tab, not a
//! window, not a selection.

mod entries;
mod headers;
mod reconcile;
mod target;
#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::path::PathBuf;

use signex_types::document_id::DocumentId;

pub(crate) use entries::{DocumentEntry, DocumentKind};
pub use headers::HeaderRead;
pub use target::DocumentTarget;

use crate::app::document::DocumentKey;

/// What the app keeps about document ids besides the documents themselves.
///
/// A loaded document carries its own id, so this holds only what a document
/// cannot say: the ids of project files that are not loaded (read from their
/// headers in the background), and the order documents were first seen in,
/// which decides who keeps an id two documents claim.
#[derive(Debug, Default)]
pub struct DocumentIds {
    /// The id at the head of each project-referenced file that is not
    /// loaded; `None` when the file has none or could not be read.
    headers: HashMap<PathBuf, Option<DocumentId>>,
    seen: HashMap<DocumentKey, u64>,
    next_seen: u64,
}

impl DocumentIds {
    pub(crate) fn header(&self, path: &std::path::Path) -> Option<Option<DocumentId>> {
        self.headers.get(path).copied()
    }

    pub(crate) fn set_header(&mut self, path: PathBuf, id: Option<DocumentId>) {
        self.headers.insert(path, id);
    }

    /// Forget the header ids of files under `dir` (the project closed).
    pub(crate) fn forget_headers_under(&mut self, dir: &std::path::Path) {
        self.headers.retain(|path, _| !path.starts_with(dir));
    }

    /// The first-seen order of `key`, assigned on first sight.
    pub(crate) fn seen_order(&mut self, key: &DocumentKey) -> u64 {
        if let Some(order) = self.seen.get(key) {
            return *order;
        }
        self.next_seen += 1;
        self.seen.insert(key.clone(), self.next_seen);
        self.next_seen
    }
}
