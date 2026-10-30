//! The `.gitignore` / `.gitattributes` side of Enable Version Control:
//! plan the merged text first (so a bad marker, a link or a read-only file
//! refuses before anything is written), write it atomically, and restore the
//! user's bytes if a later step fails ([`VcsFilePlan::roll_back`]).

use std::path::{Path, PathBuf};

use signex_types::atomic_io::atomic_write;
use signex_types::managed_block::append_missing_lines;
use signex_types::project_vcs_files::{
    GITIGNORE_BLOCK_BODY, ManagedFileError, MergedFile, merge_file, merge_gitattributes_file,
};

use crate::adapter::LibraryError;

/// A file to write, with what it held before so a failure can put it back.
struct Staged {
    path: PathBuf,
    text: String,
    prior: Option<Vec<u8>>,
}

/// The files Enable Version Control will write, fully merged in memory.
pub(crate) struct VcsFilePlan {
    staged: Vec<Staged>,
    written: Vec<usize>,
}

impl VcsFilePlan {
    /// Merge the managed blocks (and the user's unticked `rows`, outside the
    /// block) into the existing files. Writes nothing; a malformed marker
    /// pair, a link, a read-only or hard-linked file is a
    /// [`LibraryError::Conflict`].
    ///
    /// The managed `.gitignore` block is staged at both scopes, so OS and
    /// temp clutter never reaches a commit (#1729). `project_scope` adds
    /// the managed `.gitattributes` block (LFS rules only when `use_lfs`);
    /// library scope touches `.gitattributes` only for LFS. Hard-link
    /// refusal is Unix-only.
    pub(crate) fn new(
        project_dir: &Path,
        use_lfs: bool,
        project_scope: bool,
        rows: Option<&str>,
    ) -> Result<Self, LibraryError> {
        let mut staged = Vec::new();
        // Project scope always carries a managed `.gitattributes` block, so a
        // bad file is refused here, before `git init`, whatever `use_lfs` is.
        if use_lfs || project_scope {
            let file = merge_gitattributes_file(project_dir, use_lfs).map_err(refuse)?;
            staged.push(stage(project_dir.join(".gitattributes"), file, None));
        }
        let path = project_dir.join(".gitignore");
        let file = merge_file(&path, GITIGNORE_BLOCK_BODY).map_err(refuse)?;
        staged.push(stage(path, file, rows));
        Ok(Self {
            staged,
            written: Vec::new(),
        })
    }

    /// Write every file whose text changed, atomically.
    pub(crate) fn write(&mut self) -> Result<(), LibraryError> {
        for (i, file) in self.staged.iter().enumerate() {
            if file.prior.as_deref() == Some(file.text.as_bytes()) {
                continue;
            }
            atomic_write(&file.path, file.text.as_bytes()).map_err(|e| {
                LibraryError::Backend(format!("write {}: {e}", file.path.display()))
            })?;
            self.written.push(i);
        }
        Ok(())
    }

    /// Put every written file back as it was: the user's original bytes, or
    /// no file when there was none. A file whose bytes are no longer what
    /// [`Self::write`] put there (the user edited it during init) is left as
    /// is. Returns what could not be restored or was left alone, so the
    /// caller can say so instead of hiding it.
    pub(crate) fn roll_back(&self) -> Vec<String> {
        let mut failures = Vec::new();
        for &i in &self.written {
            let file = &self.staged[i];
            match std::fs::read(&file.path) {
                Ok(on_disk) if on_disk == file.text.as_bytes() => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Ok(_) => {
                    failures.push(format!(
                        "{} changed while version control was being enabled; left as is",
                        file.path.display()
                    ));
                    continue;
                }
                Err(e) => {
                    failures.push(format!(
                        "could not check {} before restoring it, left as is: {e}",
                        file.path.display()
                    ));
                    continue;
                }
            }
            let result = match &file.prior {
                Some(bytes) => atomic_write(&file.path, bytes),
                None => std::fs::remove_file(&file.path),
            };
            if let Err(e) = result {
                failures.push(format!("could not restore {}: {e}", file.path.display()));
            }
        }
        failures
    }
}

fn stage(path: PathBuf, file: MergedFile, rows: Option<&str>) -> Staged {
    let text = match rows {
        Some(rows) => append_missing_lines(&file.merged, rows),
        None => file.merged,
    };
    Staged {
        path,
        text,
        prior: file.existing.map(String::into_bytes),
    }
}

/// Remove the `.git` directory a failed init left in `project_dir`. A note
/// for the user comes back only when it is still there afterwards: a
/// missing `.git` (init failed before creating it) is already the state
/// the rollback wants.
pub(crate) fn remove_new_git_dir(project_dir: &Path) -> Option<String> {
    match std::fs::remove_dir_all(project_dir.join(".git")) {
        Ok(()) => None,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => Some(format!("could not remove the new .git directory: {e}")),
    }
}

fn refuse(e: ManagedFileError) -> LibraryError {
    match e {
        ManagedFileError::Read { .. } | ManagedFileError::Write { .. } => {
            LibraryError::Backend(e.to_string())
        }
        other => LibraryError::Conflict(other.to_string()),
    }
}

#[cfg(test)]
#[path = "vcs_init_files_tests.rs"]
mod vcs_init_files_tests;
