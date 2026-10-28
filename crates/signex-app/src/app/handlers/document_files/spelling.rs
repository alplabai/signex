//! One document per file: a file opened under another spelling of a path the
//! app already holds reuses that document (#1149).
//!
//! `Lib.snxsch` and `lib.snxsch` are one file on a case-insensitive volume
//! (and `é` as NFC / NFD is one name on APFS). Two documents for it can both
//! end clean after Ctrl+S wrote different bytes, so the second one is never
//! created: every open route first swaps the requested path for the spelling
//! the existing document holds, and its exact-path lookups then do the rest.
//!
//! The comparison is [`path_key_with`] under the volume rule of the project
//! that owns the path, so it never touches the disk.

use std::path::{Path, PathBuf};

use super::super::super::*;
use crate::app::state::scope::{CaseFold, path_key_with};

impl Signex {
    /// The key rules for `path`: the probed fold of the deepest loaded project
    /// whose directory contains it, at any depth (compared under that
    /// project's own fold, so a directory spelled `PROJ` finds the project
    /// opened as `proj`), else the
    /// host default for a path under no project.
    pub(crate) fn fold_for_document_path(&self, path: &Path) -> CaseFold {
        // The deepest project directory holding the path, so a sheet in a
        // subfolder keys under its project's volume rule too.
        self.fold_for_path(path)
    }

    /// The spelling a document, an in-flight open or a parked dirty engine
    /// already holds for the file `path` names, `path` itself when that is
    /// the held spelling, `None` when no document holds the file.
    pub(crate) fn held_spelling(&self, path: &Path) -> Option<PathBuf> {
        let fold = self.fold_for_document_path(path);
        let key = path_key_with(path, fold);
        let document_state = &self.document_state;
        let parked_dirty = document_state
            .engines
            .keys()
            .filter(|held| document_state.dirty_paths.contains(*held));
        document_state
            .tabs
            .iter()
            .map(|tab| &tab.path)
            .chain(document_state.pending_opens.paths())
            // A restore still running (even a cancelled one) holds its file's
            // spelling too, so a reopen under another spelling meets the same
            // save refusal.
            .chain(document_state.pending_restores.paths())
            .chain(parked_dirty)
            .find(|held| path_key_with(held, fold) == key)
            .cloned()
    }

    /// `path` re-spelled as the document that already holds the file, or
    /// unchanged when none does. The first call of every open route.
    pub(crate) fn resolve_held_spelling(&self, path: PathBuf) -> PathBuf {
        self.held_spelling(&path).unwrap_or(path)
    }
}
