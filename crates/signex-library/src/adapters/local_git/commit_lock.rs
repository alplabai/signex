//! One commit at a time per repository, across processes and adapters (#893).
//!
//! `git_lock` serialises the commits of one adapter instance. Two adapters
//! on the same repository (two windows, a project adapter and a library
//! adapter, another Signex process) each have their own, and race on
//! `.git/index.lock` and the branch ref. libgit2 reports the loser as a
//! failed commit. This advisory file lock, in the repository's git directory
//! beside the library write locks, makes every commit path in this module
//! take its turn. The OS drops it when the holder exits.

use std::fs::{self, File, OpenOptions, TryLockError};
use std::path::Path;
use std::time::Instant;

use super::write_lock::WRITE_LOCK_WAIT;

/// Held while one commit is made; released on drop.
#[must_use = "the lock is released as soon as this is dropped"]
pub(super) struct CommitLock {
    _file: File,
}

/// Take the commit lock of the repository whose git directory is `git_dir`,
/// waiting up to [`WRITE_LOCK_WAIT`]. `Ok(None)` on a filesystem that cannot
/// lock: the commit goes ahead unserialised across processes, as it did
/// before. `Err` is the reason a lock still held past the wait refuses the
/// commit; the save it follows is already on disk.
pub(super) fn lock_repo_for_commit(git_dir: &Path) -> Result<Option<CommitLock>, String> {
    let path = git_dir.join("signex").join("commit.lock");
    let opened = path
        .parent()
        .map_or(Ok(()), fs::create_dir_all)
        .and_then(|()| {
            OpenOptions::new()
                .create(true)
                .truncate(false)
                .write(true)
                .open(&path)
        });
    let Ok(file) = opened else {
        return Ok(None);
    };
    let deadline = Instant::now() + WRITE_LOCK_WAIT;
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(Some(CommitLock { _file: file })),
            Err(TryLockError::WouldBlock) if Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            Err(TryLockError::WouldBlock) => {
                return Err(format!(
                    "another Signex process is committing to this repository (lock {} held \
                     for over {} s)",
                    path.display(),
                    WRITE_LOCK_WAIT.as_secs()
                ));
            }
            Err(TryLockError::Error(_)) => return Ok(None),
        }
    }
}
