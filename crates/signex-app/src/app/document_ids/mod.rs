//! What documents exist and what each one is, by [`DocumentId`] (#2172).
//!
//! The id of a document is stored in the document itself (see
//! `docs/DOCUMENT_IDS.md`); this module reads it from wherever the document
//! is held, keeps the one place that assigns a missing or repeated id
//! ([`reconcile`]), and lists every document with its metadata
//! ([`entries`]). Nothing here reads UI state: not the active tab, not a
//! window, not a selection.

mod confine;
mod entries;
mod headers;
mod reconcile;
mod target;
#[cfg(test)]
mod tests;
mod waivers;
pub(crate) use waivers::Follow;

use std::collections::HashMap;
use std::path::PathBuf;

use signex_types::document_id::DocumentId;

pub(crate) use entries::referenced_files;
pub(crate) use entries::{DocumentEntry, DocumentKind};
pub use headers::HeaderRead;
pub use target::DocumentTarget;

use crate::app::document::DocumentKey;

/// One cached head: the id read and the file's modification time then.
#[derive(Debug, Clone, Copy)]
struct HeaderRecord {
    id: Option<DocumentId>,
    modified: Option<std::time::SystemTime>,
}

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
    headers: HashMap<PathBuf, HeaderRecord>,
    seen: HashMap<DocumentKey, u64>,
    next_seen: u64,
    /// Sheets a headless query loaded only to answer and drops afterwards:
    /// they are listed as not loaded and carry no revision.
    transient: std::collections::HashSet<PathBuf>,
}

impl DocumentIds {
    pub(crate) fn is_transient(&self, path: &std::path::Path) -> bool {
        self.transient.contains(path)
    }

    pub(crate) fn begin_transient(&mut self, path: PathBuf) {
        self.transient.insert(path);
    }

    pub(crate) fn end_transient(&mut self, path: &std::path::Path) {
        self.transient.remove(path);
    }

    pub(crate) fn header(&self, path: &std::path::Path) -> Option<Option<DocumentId>> {
        self.headers.get(path).map(|record| record.id)
    }

    /// Whether `path` needs its head read: never read, read and holding no
    /// id (a file written since may hold one now), or changed on disk since
    /// (one stat; a file whose time cannot be read keeps its head).
    pub(crate) fn header_wanted(&self, path: &std::path::Path) -> bool {
        self.headers.get(path).is_none_or(|record| {
            record.id.is_none()
                || std::fs::metadata(path)
                    .and_then(|meta| meta.modified())
                    .is_ok_and(|now| record.modified != Some(now))
        })
    }

    pub(crate) fn set_header(
        &mut self,
        path: PathBuf,
        id: Option<DocumentId>,
        modified: Option<std::time::SystemTime>,
    ) {
        self.headers.insert(path, HeaderRecord { id, modified });
    }

    /// The cached head of `path` no longer describes the file (it was
    /// written, its tab closed, or it changed on disk): read it again.
    pub(crate) fn forget_header(&mut self, path: &std::path::Path) {
        self.headers.remove(path);
    }

    /// Forget the heads of exactly these files (their project closed).
    pub(crate) fn forget_headers_of<'a>(
        &mut self,
        paths: impl IntoIterator<Item = &'a std::path::Path>,
    ) {
        for path in paths {
            self.headers.remove(path);
        }
    }

    /// Forget every head whose file's modification time `modified_of` knows
    /// and finds different from the one the head was read at.
    pub(crate) fn forget_changed_headers(
        &mut self,
        modified_of: impl Fn(&std::path::Path) -> Option<std::time::SystemTime>,
    ) {
        self.headers.retain(|path, record| match modified_of(path) {
            Some(now) => record.modified == Some(now),
            None => true,
        });
    }

    /// Forget the first-seen order of every document not in `held`: one that
    /// was closed has no claim on an id, so a copy opened later does not
    /// outrank the document that holds it now.
    pub(crate) fn forget_unheld_orders<'a>(&mut self, held: impl Iterator<Item = &'a DocumentKey>) {
        let held: std::collections::HashSet<&DocumentKey> = held.collect();
        self.seen.retain(|key, _| held.contains(key));
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
