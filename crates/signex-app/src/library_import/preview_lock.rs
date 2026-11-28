//! The lock a staged preview's owner holds for the preview's lifetime
//! (#3234), so the stale sweep of another running Signex never takes it.
//!
//! The lock file sits beside the staging folder, named after it with
//! [`LOCK_SUFFIX`] (inside, it would move into the library). The owner
//! creates it right after the folder and holds an exclusive lock on it
//! (`flock` on Unix, `LockFileEx` on Windows, through std) until the
//! preview is discarded, moved into place or dropped; the system releases
//! the lock when the process ends, a crash included. A sweep that can take
//! the lock knows the owner is gone; one that cannot leaves the preview.

use std::fs::{File, OpenOptions, TryLockError};
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

/// What a staging folder's lock file adds to the folder's name.
pub const LOCK_SUFFIX: &str = ".lock";

/// The lock file of the staging folder `staging`.
pub fn lock_path_for(staging: &Path) -> PathBuf {
    let mut name = staging.as_os_str().to_os_string();
    name.push(LOCK_SUFFIX);
    PathBuf::from(name)
}

/// A locked lock file: removed, then closed (which releases the lock),
/// when dropped.
#[derive(Debug)]
pub(super) struct HeldLock {
    path: PathBuf,
    file: File,
}

impl Drop for HeldLock {
    fn drop(&mut self) {
        // Removed while still locked: whoever opens it after finds no file.
        match std::fs::remove_file(&self.path) {
            Err(e) if e.kind() != ErrorKind::NotFound => {
                log::warn!(
                    "library import: the preview lock {} stays: {e}",
                    self.path.display()
                );
            }
            _ => {}
        }
    }
}

/// A staged preview's lock, shared by every clone of the preview. Held
/// until [`Self::release`] or until the last clone drops.
#[derive(Debug, Clone)]
pub struct PreviewLock {
    path: PathBuf,
    held: Arc<Mutex<Option<HeldLock>>>,
}

impl PreviewLock {
    /// Create and lock the lock file of the staging folder `staging`. A
    /// lock file already there is refused (`AlreadyExists`), never reused.
    pub fn take(staging: &Path) -> io::Result<Self> {
        let path = lock_path_for(staging);
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)?;
        let held = HeldLock {
            path: path.clone(),
            file,
        };
        let locked = held.file.try_lock();
        match locked {
            Ok(()) => Ok(Self {
                path,
                held: Arc::new(Mutex::new(Some(held))),
            }),
            // `held` drops here and removes the file it created.
            Err(TryLockError::WouldBlock) => Err(io::Error::from(ErrorKind::WouldBlock)),
            Err(TryLockError::Error(e)) => Err(e),
        }
    }

    /// Remove the lock file and let go of the lock, for every clone. Once
    /// the folder is gone the lock guards nothing, and the file must go
    /// before the parent folders the preview made can.
    pub fn release(&self) {
        let held = self
            .held
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        drop(held);
    }
}

/// Two locks are equal when they lock the same file.
impl PartialEq for PreviewLock {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path
    }
}

impl Eq for PreviewLock {}

/// What a sweep finds of a staging folder's owner.
#[derive(Debug)]
pub(super) enum Owner {
    /// No lock file: a preview from before the lock, or one whose owner
    /// removed it. Its age alone decides.
    Unknown,
    /// The owner is gone; the sweep now holds the lock, and dropping it
    /// removes the lock file.
    Gone(HeldLock),
    /// A running process holds the lock, or the lock file cannot be read
    /// as one: the preview stays.
    Alive,
}

/// The owner of the staging folder `staging`, by its lock file. The file
/// is opened without following a link and must be a plain file.
pub(super) fn owner_of(staging: &Path) -> Owner {
    let path = lock_path_for(staging);
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = match options.open(&path) {
        Err(e) if e.kind() == ErrorKind::NotFound => return Owner::Unknown,
        Err(_) => return Owner::Alive,
        Ok(file) => file,
    };
    let plain = std::fs::symlink_metadata(&path)
        .is_ok_and(|meta| meta.is_file() && !super::empty_dir::is_link(&meta));
    if !plain || !file.metadata().is_ok_and(|meta| meta.is_file()) {
        return Owner::Alive;
    }
    match file.try_lock() {
        Ok(()) => Owner::Gone(HeldLock { path, file }),
        Err(_) => Owner::Alive,
    }
}
