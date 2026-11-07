//! The one list of unsaved documents the title count, Save All enablement
//! and `has_dirty_documents` read (#661).
//!
//! Files come from the dirty path set plus the dirty drafts. A member the
//! app has no document for (a library manifest, a draft save in flight whose
//! editor closed, a case-variant spelling) is listed with no key. Component
//! rows live outside the set and are listed apart, by their dirty flag only.

use std::path::{Path, PathBuf};

use super::DocumentKey;
use crate::app::Signex;
use crate::app::state::DocumentState;
use crate::library::state::EditorAddress;

/// One unsaved file: its path, and the document that owns it when one exists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UnsavedFile {
    pub path: PathBuf,
    pub key: Option<DocumentKey>,
}

/// Everything with unsaved edits: files, and rows by their dirty flag.
#[derive(Debug, Default)]
pub(crate) struct UnsavedDocuments {
    pub files: Vec<UnsavedFile>,
    pub rows: Vec<EditorAddress>,
}

impl DocumentState {
    /// Every unsaved file, one entry per path.
    pub(crate) fn unsaved_files(&self) -> Vec<UnsavedFile> {
        let mut paths: Vec<PathBuf> = self.stored_dirty_paths().cloned().collect();
        paths.extend(self.dirty_primitive_paths());
        paths.sort();
        paths.dedup();
        paths
            .into_iter()
            .map(|path: PathBuf| UnsavedFile {
                key: self.key_for_path(&path),
                path,
            })
            .collect()
    }

    fn key_for_path(&self, path: &Path) -> Option<DocumentKey> {
        if self.symbol_editors.contains_key(path) {
            return Some(DocumentKey::Symbol(path.to_path_buf()));
        }
        if self.footprint_editors.contains_key(path) {
            return Some(DocumentKey::Footprint(path.to_path_buf()));
        }
        if let Some(project) = self.projects.iter().find(|p| p.path == path) {
            return Some(DocumentKey::Project(project.id));
        }
        if self.engines.contains_key(path) {
            return Some(DocumentKey::Schematic(path.to_path_buf()));
        }
        let is_pcb = path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("snxpcb"));
        is_pcb.then(|| DocumentKey::Pcb(path.to_path_buf()))
    }
}

impl Signex {
    /// The unsaved files and the dirty Component rows.
    pub(crate) fn unsaved_set(&self) -> UnsavedDocuments {
        UnsavedDocuments {
            files: self.document_state.unsaved_files(),
            rows: self
                .library
                .editors
                .iter()
                .filter(|(_, editor)| editor.dirty)
                .map(|(address, _)| address.clone())
                .collect(),
        }
    }
}

impl UnsavedDocuments {
    /// The title bar's "N unsaved": files only, rows are not counted.
    pub(crate) fn title_count(&self) -> usize {
        self.files.len()
    }

    /// Does Save All have anything to write?
    pub(crate) fn is_empty(&self) -> bool {
        self.files.is_empty() && self.rows.is_empty()
    }
}
