//! Pinned datasheet PDFs on a row save: which files the commit must carry,
//! and undoing a save whose commit failed (#1332, #1471).

use std::path::Path;
use std::sync::RwLockWriteGuard;

use super::*;
use crate::component::{ComponentRow, DatasheetRef};
use crate::datasheet_asset::datasheet_asset_rel_path;
use crate::library_file::LibraryFile;

impl LocalGitAdapter {
    /// `given` plus every PDF `row` pins that sits beside the library but is
    /// not in HEAD's tree. A row that pins a hash must never be committed
    /// without its file; after a save that failed twice the file can be on
    /// disk and untracked while no editor still stages it. A pinned file
    /// that is neither tracked nor on disk is left as it is: refusing would
    /// block every edit of a row that was already dangling, and this save
    /// makes it no worse. Without a git repository nothing is committed.
    pub(super) fn pinned_assets_to_commit(
        &self,
        row: &ComponentRow,
        given: &[String],
    ) -> Vec<String> {
        let mut out = given.to_vec();
        if !self.root_dir.join(".git").exists() {
            return out;
        }
        // A repository that cannot be read makes the commit itself fail
        // (and be reported) a moment later; there is nothing to add here.
        let Ok(repo) = git2::Repository::open(&self.root_dir) else {
            return out;
        };
        let head_tree = match repo.head() {
            Ok(head) => match head.peel_to_commit().and_then(|commit| commit.tree()) {
                Ok(tree) => Some(tree),
                Err(_) => return out,
            },
            Err(e) if e.code() == git2::ErrorCode::UnbornBranch => None,
            Err(_) => return out,
        };
        for doc in row.documents() {
            let DatasheetRef::HashPinned { hash, .. } = &doc.source else {
                continue;
            };
            let Some(rel) = datasheet_asset_rel_path(hash) else {
                continue;
            };
            if out.contains(&rel) {
                continue;
            }
            let tracked = head_tree
                .as_ref()
                .is_some_and(|tree| tree.get_path(Path::new(&rel)).is_ok());
            if !tracked && self.root_dir.join(&rel).is_file() {
                out.push(rel);
            }
        }
        out
    }

    /// The commit that carries a row's pinned PDFs failed: put the library
    /// file `previous` back on disk (through `write`) and in memory, so the
    /// caller keeps its edit and can save again. If the restore write fails
    /// too, disk and memory both keep the saved row (nothing is half
    /// undone) and the error says so.
    pub(super) fn undo_asset_commit(
        &self,
        guard: &mut RwLockWriteGuard<'_, LibraryFile>,
        previous: Vec<u8>,
        previous_file: LibraryFile,
        commit_err: LibraryError,
        write: impl FnOnce(&Path, &[u8]) -> std::io::Result<()>,
    ) -> LibraryError {
        if let Err(e) = write(&self.file_path, &previous) {
            return LibraryError::Backend(format!(
                "{commit_err}; the row was saved but its datasheet PDF was not committed, \
                 and the library file could not be restored: {e}. Saving the row again \
                 commits the PDF"
            ));
        }
        **guard = previous_file;
        if let Err(stamp_err) = self.note_own_write(&String::from_utf8_lossy(&previous)) {
            tracing::warn!(
                target: "signex::library",
                path = %self.file_path.display(),
                "restored, but the disk stamp could not be recorded: {stamp_err}"
            );
        }
        LibraryError::Backend(format!(
            "the row was not saved: its datasheet PDF could not be committed ({commit_err})"
        ))
    }
}

#[cfg(test)]
mod tests;
