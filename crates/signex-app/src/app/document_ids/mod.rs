//! What documents exist and what each one is, by [`DocumentId`](signex_types::document_id::DocumentId) (#2172).
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
mod project_named;
mod reconcile;
mod target;
#[cfg(test)]
mod tests;
mod waivers;
pub(crate) use waivers::Follow;

pub(crate) use confine::confined_read;
pub(crate) use entries::referenced_files;
pub(crate) use entries::{DocumentEntry, DocumentKind};
pub use headers::HeaderRead;
pub(crate) use project_named::NoProjectNamed;
pub use target::DocumentTarget;

pub use crate::app::core::document_ids::DocumentIds;

impl DocumentIds {
    /// Whether `path` needs its head read: never read, read and holding no
    /// id (a file written since may hold one now), or changed on disk since
    /// (one stat; a file whose time cannot be read keeps its head).
    pub(crate) fn header_wanted(&self, path: &std::path::Path) -> bool {
        self.header_wanted_given(path, || {
            std::fs::metadata(path).and_then(|meta| meta.modified())
        })
    }
}
