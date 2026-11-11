//! The one list of unsaved documents the title count, Save All enablement
//! and `has_dirty_documents` read (#661).
//!
//! Files come from the dirty path set plus the dirty drafts and a board's
//! unapplied sketch (#2234). A member the
//! app has no document for (a library manifest, a draft save in flight whose
//! editor closed, a case-variant spelling) is listed like any other. Component
//! rows live outside the set and are listed apart: a row with unsaved work
//! (its dirty flag, or a half-typed value, #2132).

use std::path::PathBuf;

use super::DocumentKey;
use crate::app::Signex;
use crate::app::state::DocumentState;
use crate::library::state::EditorAddress;

/// Everything with unsaved edits: files, and rows with unsaved work.
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

    /// The unsaved files that are symbol or footprint drafts: what the Close
    /// Project and Close Library prompts place by their owning library.
    pub(crate) fn unsaved_drafts(&self) -> Vec<PathBuf> {
        let mut paths = self.unsaved_files();
        paths.retain(|path| {
            matches!(
                self.document_key_for_path(path),
                Some(DocumentKey::Symbol(_) | DocumentKey::Footprint(_))
            )
        });
        paths
    }
}

impl Signex {
    /// The unsaved files and the dirty Component rows.
    pub(crate) fn unsaved_set(&self) -> UnsavedDocuments {
        UnsavedDocuments {
            files: self.unsaved_files_with_sketch(),
            rows: self.library.dirty_editors(),
        }
    }

    /// How many files are unsaved: the title bar's "N unsaved".
    pub(crate) fn unsaved_file_count(&self) -> usize {
        self.unsaved_files_with_sketch().len()
    }

    /// The unsaved files, plus a board whose sketch has changes it has not
    /// taken yet (#2234): that is the board's unsaved work too.
    fn unsaved_files_with_sketch(&self) -> Vec<PathBuf> {
        let mut files = self.document_state.unsaved_files();
        if let Some(board) = self.board_sketch_owed()
            && let Err(at) = files.binary_search(&board)
        {
            files.insert(at, board);
        }
        files
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
