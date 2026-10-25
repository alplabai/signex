//! The `.gitignore` / `.gitattributes` side of Enable Version Control:
//! plan the merged text first (so a bad marker refuses before anything is
//! written), write it atomically, and restore the user's bytes if a later
//! step fails.

use std::path::{Path, PathBuf};

use signex_types::atomic_io::atomic_write;
use signex_types::managed_block::append_missing_lines;
use signex_types::project_vcs_files::{
    GITIGNORE_BLOCK_BODY, ManagedFileError, MergedFile, gitattributes_block_body, merge_file,
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
    /// pair is a [`LibraryError::Conflict`].
    pub(crate) fn new(
        project_dir: &Path,
        use_lfs: bool,
        rows: Option<&str>,
    ) -> Result<Self, LibraryError> {
        let mut staged = Vec::new();
        if use_lfs {
            let path = project_dir.join(".gitattributes");
            let file = merge_file(&path, &gitattributes_block_body(true)).map_err(refuse)?;
            staged.push(stage(path, file, None));
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
    /// no file when there was none. Best-effort, called on the failure path.
    pub(crate) fn roll_back(&self) {
        for &i in &self.written {
            let file = &self.staged[i];
            match &file.prior {
                Some(bytes) => {
                    let _ = atomic_write(&file.path, bytes);
                }
                None => {
                    let _ = std::fs::remove_file(&file.path);
                }
            }
        }
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

fn refuse(e: ManagedFileError) -> LibraryError {
    match e {
        ManagedFileError::Markers { .. } | ManagedFileError::NotUtf8 { .. } => {
            LibraryError::Conflict(e.to_string())
        }
        other => LibraryError::Backend(other.to_string()),
    }
}
