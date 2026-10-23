//! Cross-process write lock for a `.snxlib` (#832).
//!
//! `disk_sync` makes each mutation reload the file and apply the edit on top,
//! so an external change is merged instead of overwritten. That alone still
//! leaves a window: two Signex processes can both reload, both apply, and the
//! second write drops the first one's edit. Each mutation therefore holds an
//! exclusive, advisory lock across reload → edit → write.
//!
//! Advisory means only Signex honours it, which is the case it exists for
//! (two windows or two instances on one library). A text editor or `git pull`
//! is still covered by the reload-and-merge in `disk_sync`.
//!
//! The lock is a separate file, because the `.snxlib` itself is replaced by
//! an atomic rename on every write and a lock on the old file would guard
//! nothing. It lives under `.git/` when the library has one, so it never
//! shows up as an untracked file, and next to the `.snxlib` otherwise.

use std::fs::{File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use super::*;

/// How long a mutation waits for another process to finish writing the same
/// library. A writer holds the lock only for one reload and one write, so a
/// longer wait means that process is stuck.
pub const WRITE_LOCK_WAIT: Duration = Duration::from_secs(2);

const RETRY_EVERY: Duration = Duration::from_millis(20);

/// Held for the duration of one mutation; the lock is released on drop.
#[must_use = "the lock is released as soon as this is dropped"]
pub(super) struct WriteLock {
    _file: File,
}

/// Where the lock for the library at `file_path` (under `root_dir`) lives.
pub fn write_lock_path(root_dir: &Path, file_path: &Path) -> PathBuf {
    let name = file_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "library.snxlib".to_owned());
    let git_dir = root_dir.join(".git");
    if git_dir.is_dir() {
        git_dir.join(format!("signex-{name}.lock"))
    } else {
        file_path.with_file_name(format!(".{name}.lock"))
    }
}

impl LocalGitAdapter {
    /// Take the cross-process write lock, waiting up to [`WRITE_LOCK_WAIT`]
    /// for another process to finish. A lock still held after that refuses
    /// the edit rather than risk dropping the other process's write.
    pub(super) fn lock_for_write(&self) -> Result<WriteLock, LibraryError> {
        let path = write_lock_path(&self.root_dir, &self.file_path);
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&path)?;
        let deadline = Instant::now() + WRITE_LOCK_WAIT;
        loop {
            match file.try_lock() {
                Ok(()) => return Ok(WriteLock { _file: file }),
                Err(TryLockError::WouldBlock) if Instant::now() < deadline => {
                    thread::sleep(RETRY_EVERY);
                }
                Err(TryLockError::WouldBlock) => {
                    return Err(LibraryError::Backend(format!(
                        "{} is being written by another Signex process (lock {} held for over \
                         {} s); the edit was not saved",
                        self.file_path.display(),
                        path.display(),
                        WRITE_LOCK_WAIT.as_secs()
                    )));
                }
                Err(TryLockError::Error(e)) => return Err(e.into()),
            }
        }
    }
}
