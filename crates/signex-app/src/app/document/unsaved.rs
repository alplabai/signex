//! The one list of unsaved documents the title count, Save All enablement
//! and `has_dirty_documents` read (#661).
//!
//! Files come from the dirty path set plus the dirty drafts. A member the
//! app has no document for (a library manifest, a draft save in flight whose
//! editor closed, a case-variant spelling) is listed like any other. Component
//! rows live outside the set and are listed apart, by their dirty flag only.

use std::path::PathBuf;

use crate::app::Signex;
use crate::app::state::DocumentState;
use crate::library::state::EditorAddress;

/// Everything with unsaved edits: files, and rows by their dirty flag.
#[derive(Debug, Default)]
pub(crate) struct UnsavedDocuments {
    pub files: Vec<PathBuf>,
    pub rows: Vec<EditorAddress>,
}

impl DocumentState {
    /// Every unsaved file's path, sorted, one entry per path.
    pub(crate) fn unsaved_files(&self) -> Vec<PathBuf> {
        let mut paths: Vec<PathBuf> = self.stored_dirty_paths().cloned().collect();
        paths.extend(self.dirty_primitive_paths());
        paths.sort();
        paths.dedup();
        paths
    }
}

impl Signex {
    /// The unsaved files and the dirty Component rows.
    pub(crate) fn unsaved_set(&self) -> UnsavedDocuments {
        UnsavedDocuments {
            files: self.document_state.unsaved_files(),
            rows: self.library.dirty_editors(),
        }
    }
}

impl UnsavedDocuments {
    /// The files' paths, sorted: the order the quit prompt lists them in.
    pub(crate) fn paths(&self) -> Vec<PathBuf> {
        self.files.clone()
    }

    /// Does Save All have anything to write?
    pub(crate) fn is_empty(&self) -> bool {
        self.files.is_empty() && self.rows.is_empty()
    }
}
