//! Cross-process write lock for a `.snxlib` (#832).
//!
//! `disk_sync` makes each mutation reload the file and apply the edit on top,
//! so an external change is merged instead of overwritten. That alone still
//! leaves a window: two Signex processes can both reload, both apply, and the
//! second write drops the first one's edit. Each mutation therefore holds an
//! exclusive, advisory lock across reload → edit → write → commit.
//!
//! Advisory means only Signex honours it, which is the case it exists for
//! (two windows or two instances on one library). A text editor or `git pull`
//! is still covered by the reload-and-merge in `disk_sync`. The OS drops the
//! lock when the holding process exits, so a crash leaves nothing stuck.
//!
//! The lock is a separate file, because the `.snxlib` itself is replaced by
//! an atomic rename on every write and a lock on the old file would guard
//! nothing. With version control it lives in the repository's git directory
//! (resolved through git2, so a submodule or linked worktree works too), as
//! `signex/<name>.writelock`: outside the working tree, and not matched by a
//! `rm .git/*.lock` cleanup. Without version control it is a hidden file
//! next to the library, which [`is_write_lock_file`] keeps out of the first
//! commit when version control is enabled later.
//!
//! # Lock order (#858, #893)
//!
//! Every lock a library write takes, outermost first. A path may skip locks,
//! never take them in another order, and never wait for an outer lock while
//! holding an inner one:
//!
//! 1. the write queue (this module): held only while waiting for 2;
//! 2. the library write lock (this module): reload, edit, write, commit;
//! 3. the adapter's `git_lock` (in-process commit mutex);
//! 4. the repository commit lock (`commit_lock`): held for one commit only
//!    and the last lock taken, so nothing waits on anything while it is held.
//!
//! The commit paths that hold no write lock (`commit_external_change`, the
//! project adapter) start at 3, which is why nothing can wait on 2 while
//! holding 3 or 4. `tests/integration/local_git_lock_order.rs` pins this.
//!
//! # Fairness
//!
//! The lock is advisory and polled, so a thread that saves in a loop would
//! win every race against a waiter that polls every 20 ms: release and
//! re-acquire leave no gap to land in. On a slow disk (a Windows runner
//! whose commits take a second) the waiter then ran out of its 5 s while the
//! lock changed hands many times. Waiters therefore queue: a writer first
//! takes the write queue, then waits for the write lock, then lets the queue
//! go. A holder that comes back for another turn waits behind the writer
//! already queued, so each writer waits for one holder, not for a streak.

use std::fs::{self, File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use super::*;

/// How long a mutation waits for another process to finish writing the same
/// library. A writer holds the lock for one reload, write and commit, so the
/// wait assumes the other process is not in a long burst of edits; past it
/// the edit is refused rather than risk dropping the other process's write.
pub const WRITE_LOCK_WAIT: Duration = Duration::from_secs(5);

const RETRY_EVERY: Duration = Duration::from_millis(20);

/// Suffix of the no-version-control lock file.
const SIDECAR_SUFFIX: &str = ".signex-writelock";

/// Suffix that turns a lock file name into its write queue's (see the module
/// doc, "Fairness").
const QUEUE_SUFFIX: &str = ".queue";

/// The write queue beside the lock file at `lock`.
fn queue_path(lock: &Path) -> PathBuf {
    let mut name = lock.file_name().map(|n| n.to_owned()).unwrap_or_default();
    name.push(QUEUE_SUFFIX);
    lock.with_file_name(name)
}

/// Held for the duration of one mutation; the lock is released on drop.
#[must_use = "the lock is released as soon as this is dropped"]
pub(super) struct WriteLock {
    _file: File,
}

fn library_name(file_path: &Path) -> String {
    file_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "library.snxlib".to_owned())
}

fn sidecar_path(file_path: &Path) -> PathBuf {
    file_path.with_file_name(format!(".{}{SIDECAR_SUFFIX}", library_name(file_path)))
}

/// Where the write lock for the library at `file_path` lives: under the git
/// directory of the repository the library sits in, else next to the file.
pub fn write_lock_path(file_path: &Path) -> PathBuf {
    let dir = file_path.parent().unwrap_or_else(|| Path::new("."));
    match git2::Repository::open(dir) {
        Ok(repo) => repo
            .path()
            .join("signex")
            .join(format!("{}.writelock", library_name(file_path))),
        Err(_) => sidecar_path(file_path),
    }
}

/// Whether `path` is a no-version-control write-lock file, which must never
/// be committed (the stage-everything of enabling version control skips it).
pub fn is_write_lock_file(path: &Path) -> bool {
    path.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
        n.starts_with('.')
            && (n.ends_with(SIDECAR_SUFFIX)
                || n.strip_suffix(QUEUE_SUFFIX)
                    .is_some_and(|n| n.ends_with(SIDECAR_SUFFIX)))
    })
}

impl LocalGitAdapter {
    /// Take the cross-process write lock, waiting up to [`WRITE_LOCK_WAIT`]
    /// for another process to finish. A lock still held after that refuses
    /// the edit rather than risk dropping the other process's write.
    ///
    /// A filesystem that cannot lock (some network mounts) gives `None`: the
    /// edit goes ahead unlocked, as before #832, with a one-time warning,
    /// rather than leaving the library read-only.
    pub(super) fn lock_for_write(&self) -> Result<Option<WriteLock>, LibraryError> {
        acquire(&self.file_path, &self.write_lock_warned)
    }
}

/// Rewrite `abs_path`, a file of the library at `library_file`, under that
/// library's write lock, for a caller that has no [`LocalGitAdapter`] open on
/// it: a primitive file opened on its own (#902). See
/// [`crate::adapter::LibraryAdapter::rewrite_file_locked`].
pub fn rewrite_file_under_library_lock(
    library_file: &Path,
    abs_path: &Path,
    rewrite: &mut crate::adapter::FileRewrite<'_>,
) -> Result<bool, LibraryError> {
    static WARNED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    let _write_lock = acquire(library_file, &WARNED)?;
    crate::adapter::rewrite_file(abs_path, rewrite)
}

/// The `.snxlib` of the library whose directory holds `path`, if any: the
/// nearest ancestor directory with a `.snxlib` file in it. Reads one
/// directory per level up to the root.
pub fn enclosing_library_file(path: &Path) -> Option<PathBuf> {
    path.ancestors().skip(1).find_map(|dir| {
        let mut libraries: Vec<PathBuf> = fs::read_dir(dir)
            .ok()?
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_file() && p.extension().is_some_and(|ext| ext == "snxlib"))
            .collect();
        // `read_dir` order is unspecified; pick the same one every time.
        libraries.sort();
        libraries.into_iter().next()
    })
}

fn acquire(file_path: &Path, warned: &AtomicBool) -> Result<Option<WriteLock>, LibraryError> {
    let path = write_lock_path(file_path);
    let file = match open_lock_file(&path) {
        Ok(file) => file,
        Err(e) => return Ok(unlocked(file_path, &path, &e, warned)),
    };
    // Queue first (see the module doc, "Fairness"), and let it go as soon as
    // the lock is ours, so the writer behind us is the next to wait.
    let queue = match open_lock_file(&queue_path(&path)) {
        Ok(queue) => match wait_for(&queue) {
            Waited::Locked => Some(queue),
            Waited::TimedOut => return Err(held_too_long(file_path, &queue_path(&path))),
            Waited::Unsupported(_) => None,
        },
        Err(_) => None,
    };
    match wait_for(&file) {
        Waited::Locked => {
            drop(queue);
            // A sidecar from before version control was enabled is no
            // longer the lock anyone takes.
            if path != sidecar_path(file_path) {
                drop(fs::remove_file(sidecar_path(file_path)));
                drop(fs::remove_file(queue_path(&sidecar_path(file_path))));
            }
            Ok(Some(WriteLock { _file: file }))
        }
        Waited::TimedOut => Err(held_too_long(file_path, &path)),
        Waited::Unsupported(e) => Ok(unlocked(file_path, &path, &e, warned)),
    }
}

/// How waiting for an advisory lock ended.
enum Waited {
    Locked,
    TimedOut,
    /// The filesystem cannot lock.
    Unsupported(std::io::Error),
}

/// Take the exclusive lock on `file`, polling for up to [`WRITE_LOCK_WAIT`].
fn wait_for(file: &File) -> Waited {
    let deadline = Instant::now() + WRITE_LOCK_WAIT;
    loop {
        match file.try_lock() {
            Ok(()) => return Waited::Locked,
            Err(TryLockError::WouldBlock) if Instant::now() < deadline => {
                thread::sleep(RETRY_EVERY);
            }
            Err(TryLockError::WouldBlock) => return Waited::TimedOut,
            Err(TryLockError::Error(e)) => return Waited::Unsupported(e),
        }
    }
}

fn held_too_long(file_path: &Path, lock: &Path) -> LibraryError {
    LibraryError::Backend(format!(
        "{} is being written by another Signex process (lock {} held for over {} s); the \
         edit was not saved",
        file_path.display(),
        lock.display(),
        WRITE_LOCK_WAIT.as_secs()
    ))
}

fn unlocked(
    file_path: &Path,
    path: &Path,
    error: &std::io::Error,
    warned: &AtomicBool,
) -> Option<WriteLock> {
    if !warned.swap(true, Ordering::Relaxed) {
        tracing::warn!(
            target: "signex::library",
            library = %file_path.display(),
            lock = %path.display(),
            error = %error,
            "cannot lock the library for writing on this filesystem; edits are saved \
             without protection against another Signex process writing at the same time"
        );
    }
    None
}

fn open_lock_file(path: &Path) -> std::io::Result<File> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)
}
