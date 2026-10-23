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
//!   A mutation additionally compares a content hash of the file against
//!   the hash of the text last read or written, because a same-length edit
//!   inside one mtime tick (HFS+ 1 s, FAT/exFAT 2 s) leaves the stamp
//!   unchanged and a mutation is the one path that would destroy it. Reads
//!   trust the stamp alone: a full read per call is what the large-library
//!   numbers in #530 rule out, and a stale read heals on the next change.
//!
//! Concurrent edits to the *same* row are last-writer-wins: the in-app edit
//! is applied on top of the freshly reloaded file, so it replaces the
//! external version of that row. Different rows and tables merge cleanly.
//!
//! Every in-app mutation is persisted before it returns, so the adapter holds
//! no unsaved state of its own that a reload could discard. Unsaved
//! *editor drafts* live in the app and are untouched by this.

use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::time::SystemTime;

use super::*;

/// Identity of a `.snxlib` as last read or written by this adapter.
///
/// mtime + length is a heuristic: an external write that lands in the same
/// filesystem timestamp tick *and* keeps the byte length goes unnoticed by
/// it. Reads accept that; mutations close the gap with a content hash (see
/// the module doc).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiskStamp {
    modified: Option<SystemTime>,
    len: u64,
}

impl DiskStamp {
    /// One `stat` of `path`.
    pub fn of(path: &Path) -> Result<Self, LibraryError> {
        let meta = fs::metadata(path)?;
        Ok(Self {
            modified: meta.modified().ok(),
            len: meta.len(),
        })
    }
}

/// What the adapter knows about the file behind its in-memory copy: the
/// stat stamp and a hash of the exact text that was parsed or written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct KnownDisk {
    stamp: DiskStamp,
    text_hash: u64,
}

impl KnownDisk {
    pub(super) fn new(stamp: DiskStamp, text: &str) -> Self {
        Self {
            stamp,
            text_hash: hash_text(text),
        }
    }
}

/// Hash for change detection only — never persisted, so the hasher's
/// algorithm is free to change between releases.
fn hash_text(text: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

/// How hard [`LocalGitAdapter::reload_if_stale`] checks that the copy in
/// memory still matches the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Verify {
    /// Trust the mtime + length stamp (reads).
    Stamp,
    /// Also compare a content hash when the stamp matches (mutations).
    Content,
}

/// An on-disk state a failed reload was logged for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DiskState {
    /// The file cannot be `stat`ed (deleted, or its directory is gone).
    Missing,
    Present(DiskStamp),
}

impl LocalGitAdapter {
    /// Reload the in-memory copy if the file changed on disk. The caller
    /// holds the write guard, so the swap and the stamp update are atomic
    /// with respect to other readers.
    pub(super) fn reload_if_stale(
        &self,
        lf: &mut LibraryFile,
        verify: Verify,
    ) -> Result<(), LibraryError> {
        let on_disk = DiskStamp::of(&self.file_path)?;
        let mut known = self
            .disk_stamp
            .lock()
            .map_err(|_| LibraryError::Backend("disk_stamp lock poisoned".into()))?;
        let stamp_matches = known.is_some_and(|k| k.stamp == on_disk);
        if stamp_matches && verify == Verify::Stamp {
            return Ok(());
        }
        // The stamp was taken before the read, so a write racing in between
        // leaves the stamp older than the text and costs one extra reload,
        // never a missed one.
        let text = fs::read_to_string(&self.file_path)?;
        let fresh = KnownDisk::new(on_disk, &text);
        if stamp_matches && known.is_some_and(|k| k.text_hash == fresh.text_hash) {
            return Ok(());
        }
        *lf = LibraryFile::parse(&text)?;
        *known = Some(fresh);
        Ok(())
    }

    /// Record the stamp and text of the file this adapter just wrote.
    pub(super) fn note_own_write(&self, text: &str) -> Result<(), LibraryError> {
        let stamp = DiskStamp::of(&self.file_path)?;
        *self
            .disk_stamp
            .lock()
            .map_err(|_| LibraryError::Backend("disk_stamp lock poisoned".into()))? =
            Some(KnownDisk::new(stamp, text));
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
                self.warn_reload_failed(DiskState::Missing, &e);
                return;
            }
        };
        let is_current = self
            .disk_stamp
            .lock()
            .map(|known| known.is_some_and(|k| k.stamp == on_disk))
            .unwrap_or(false);
        if is_current {
            self.clear_reload_warning();
            return;
        }
        let result = match self.library_file.write() {
            Ok(mut guard) => self.reload_if_stale(&mut guard, Verify::Stamp),
            Err(_) => Err(LibraryError::Backend(
                "library_file write lock poisoned".into(),
            )),
        };
        match result {
            Ok(()) => self.clear_reload_warning(),
            Err(e) => self.warn_reload_failed(DiskState::Present(on_disk), &e),
        }
    }

    /// A good read ends the failure episode, so the same failure recurring
    /// later is reported again instead of being taken for a repeat.
    fn clear_reload_warning(&self) {
        if let Ok(mut warned) = self.warned_state.lock() {
            *warned = None;
        }
    }

    fn warn_reload_failed(&self, state: DiskState, error: &LibraryError) {
        if let Ok(mut warned) = self.warned_state.lock() {
            if *warned == Some(state) {
                return;
            }
            *warned = Some(state);
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
