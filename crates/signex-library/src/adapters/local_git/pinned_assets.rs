//! Pinned datasheet PDFs on a row save: which files the commit must carry,
//! and undoing a save whose commit failed (#1332, #1471).

use std::path::Path;
use std::sync::RwLockWriteGuard;

use super::*;
use crate::component::DatasheetRef;
use crate::datasheet_asset::datasheet_asset_rel_path;
use crate::library_file::LibraryFile;

impl LocalGitAdapter {
    /// `given` plus every PDF that a row of `candidate` pins and that sits
    /// beside the library but is not in HEAD's tree. Every `.snxlib` commit
    /// writes every row's pins, so the scan covers the whole candidate file,
    /// not just the row being edited: after a save that failed twice the
    /// file is on disk and untracked while no editor still stages it, and
    /// any later commit of the library would otherwise put its pin into
    /// HEAD without it. Call under the write lock. A pinned file that is
    /// neither tracked nor on disk is left as it is (refusing would block
    /// every edit of a library that was already dangling), and a file that
    /// is not what its name claims is not committed; each such pin is
    /// logged. Without a git repository nothing is committed.
    pub(super) fn pinned_assets_to_commit(
        &self,
        candidate: &LibraryFile,
        given: &[String],
    ) -> Vec<String> {
        let mut out = given.to_vec();
        let mut pins: Vec<(String, String)> = Vec::new();
        for table in candidate.tables.values() {
            for lib_row in &table.rows {
                let Ok(row) = library_row_to_component(lib_row) else {
                    continue;
                };
                for doc in row.documents() {
                    let DatasheetRef::HashPinned { hash, .. } = &doc.source else {
                        continue;
                    };
                    let Some(rel) = datasheet_asset_rel_path(hash) else {
                        tracing::warn!(
                            target: "signex::library",
                            "a row pins {hash:?}, which is not a datasheet hash"
                        );
                        continue;
                    };
                    if !out.contains(&rel) && !pins.iter().any(|(_, r)| *r == rel) {
                        pins.push((hash.clone(), rel));
                    }
                }
            }
        }
        if pins.is_empty() || !self.root_dir.join(".git").exists() {
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
        for (hash, rel) in pins {
            let tracked = head_tree
                .as_ref()
                .is_some_and(|tree| tree.get_path(Path::new(&rel)).is_ok());
            if tracked {
                continue;
            }
            match self.read_pinned_file(&hash, &rel) {
                Ok(Some(())) => out.push(rel),
                Ok(None) => tracing::warn!(
                    target: "signex::library",
                    "a row pins {rel}, but the file is not in the library"
                ),
                Err(why) => tracing::warn!(
                    target: "signex::library",
                    "a row pins {rel}, which is not committed: {why}"
                ),
            }
        }
        out
    }

    /// `Ok(Some(()))` when `rel` is a regular file inside the library, under
    /// the size cap, whose bytes hash to `hash`; `Ok(None)` when it is not
    /// on disk; `Err` with the reason when it must not be committed.
    fn read_pinned_file(&self, hash: &str, rel: &str) -> Result<Option<()>, String> {
        let mut dir = self.root_dir.clone();
        let parts: Vec<&str> = rel.split('/').collect();
        for part in &parts {
            dir.push(part);
            match std::fs::symlink_metadata(&dir) {
                Ok(meta) if meta.file_type().is_symlink() => {
                    return Err("it is reached through a symbolic link".into());
                }
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                Err(e) => return Err(e.to_string()),
            }
        }
        let meta = std::fs::metadata(&dir).map_err(|e| e.to_string())?;
        if !meta.is_file() {
            return Ok(None);
        }
        if meta.len() > crate::datasheet_asset::MAX_DATASHEET_BYTES {
            return Err("it is larger than the datasheet size limit".into());
        }
        let bytes = std::fs::read(&dir).map_err(|e| e.to_string())?;
        if crate::datasheet_asset::datasheet_hash(&bytes) != hash {
            return Err("its bytes do not hash to the pinned value".into());
        }
        Ok(Some(()))
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
        // The restored text is not taken as this adapter's own checked
        // write, so the next edit goes through the full write (#416).
        if let Err(stamp_err) = self.note_restored(&String::from_utf8_lossy(&previous)) {
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
