//! Keeping the in-memory `.snxlib` view in step with the file on disk (#531).
//!
//! The adapter serves every read from a parsed [`LibraryFile`], but the file
//! can be changed by another process (a second window, `git pull`, a text
//! editor). Each read and each mutation therefore first compares a cheap
//! [`DiskStamp`] (mtime + length, one `stat`) against the stamp of the copy
//! in memory and re-parses only when they differ.
//!
//! Policy when the file changed:
//!
//! * **Reads** swap in the new parse. If the new text does not parse (a
//!   half-written file, a merge-conflict marker), the last good copy keeps
//!   being served and the failure is logged — a mounted library never
//!   blanks because of a transient bad write.
//! * **Mutations** reload first and apply the in-app edit on top, so an
//!   external row is never overwritten by a stale copy. If the reload fails
//!   the mutation is refused: writing would clobber a file we could not read.
//!
//! Every in-app mutation is persisted before it returns, so the adapter holds
//! no unsaved state of its own that a reload could discard. Unsaved
//! *editor drafts* live in the app and are untouched by this.

use std::fs;
use std::time::SystemTime;

use super::*;

/// Identity of a `.snxlib` as last read or written by this adapter.
///
/// mtime + length is a heuristic: an external write that lands in the same
/// filesystem timestamp tick *and* keeps the byte length would go unnoticed.
/// Hashing the content instead would cost a full read on every call, which is
/// exactly what the large-library numbers in #530 rule out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct DiskStamp {
    modified: Option<SystemTime>,
    len: u64,
}

impl DiskStamp {
    pub(super) fn of(path: &Path) -> Result<Self, LibraryError> {
        let meta = fs::metadata(path)?;
        Ok(Self {
            modified: meta.modified().ok(),
            len: meta.len(),
        })
    }
}

impl LocalGitAdapter {
    /// Reload the in-memory copy if the file changed on disk. The caller
    /// holds the write guard, so the swap and the stamp update are atomic
    /// with respect to other readers.
    pub(super) fn reload_if_stale(&self, lf: &mut LibraryFile) -> Result<(), LibraryError> {
        let on_disk = DiskStamp::of(&self.file_path)?;
        let mut known = self
            .disk_stamp
            .lock()
            .map_err(|_| LibraryError::Backend("disk_stamp lock poisoned".into()))?;
        if *known == Some(on_disk) {
            return Ok(());
        }
        // The stamp was taken before the read, so a write racing in between
        // leaves the stamp older than the text and costs one extra reload,
        // never a missed one.
        let text = fs::read_to_string(&self.file_path)?;
        *lf = LibraryFile::parse(&text)?;
        *known = Some(on_disk);
        Ok(())
    }

    /// Record the stamp of the file this adapter just wrote.
    pub(super) fn note_own_write(&self) -> Result<(), LibraryError> {
        let stamp = DiskStamp::of(&self.file_path)?;
        *self
            .disk_stamp
            .lock()
            .map_err(|_| LibraryError::Backend("disk_stamp lock poisoned".into()))? = Some(stamp);
        Ok(())
    }

    /// Bring the read view up to date before serving a read. Never fails the
    /// read: a file that cannot be re-read keeps the last good copy, and the
    /// reason is logged once per distinct on-disk state for the Messages
    /// panel (reads happen on every refresh; repeating it would drown it).
    pub(super) fn refresh_for_read(&self) {
        let on_disk = match DiskStamp::of(&self.file_path) {
            Ok(stamp) => stamp,
            Err(e) => {
                self.warn_reload_failed(None, &e);
                return;
            }
        };
        let is_current = self
            .disk_stamp
            .lock()
            .map(|known| *known == Some(on_disk))
            .unwrap_or(false);
        if is_current {
            return;
        }
        let result = match self.library_file.write() {
            Ok(mut guard) => self.reload_if_stale(&mut guard),
            Err(_) => Err(LibraryError::Backend(
                "library_file write lock poisoned".into(),
            )),
        };
        if let Err(e) = result {
            self.warn_reload_failed(Some(on_disk), &e);
        }
    }

    fn warn_reload_failed(&self, stamp: Option<DiskStamp>, error: &LibraryError) {
        if let Ok(mut warned) = self.warned_stamp.lock() {
            if stamp.is_some() && *warned == stamp {
                return;
            }
            *warned = stamp;
        }
        tracing::warn!(
            target: "signex::library",
            path = %self.file_path.display(),
            error = %error,
            "could not re-read the library after it changed on disk; \
             still showing the last good copy"
        );
    }
}
