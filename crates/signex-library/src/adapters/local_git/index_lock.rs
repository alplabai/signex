//! Updates of the repository's on-disk index made under git's own
//! `index.lock` protocol (#2337).

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

/// The scratch index an update is written to, inside the git dir. libgit2
/// writes an index only to its own path, so the result goes here and is
/// then moved onto `index.lock`. Only the holder of `index.lock` touches it.
const SCRATCH_INDEX: &str = "index.signex-scratch";

/// Apply `edit` to the repository's own index, read while we hold
/// `index.lock`, and publish it through that lock. `Err` is the update's
/// failure (a held lock is `ErrorCode::Locked`); `Ok(Err(_))` is `edit`'s.
/// Keep slow work (hashing, hooks) out of `edit`: it runs under the lock.
/// Protocol: see docs/architecture/library-writes.md, "The git index after
/// a commit".
pub(super) fn update_index_locked<E>(
    repo: &git2::Repository,
    edit: impl FnOnce(&mut git2::Index) -> Result<(), E>,
) -> Result<Result<(), E>, git2::Error> {
    let mut lock = IndexLock::take(repo)?;
    lock.read()?;
    let edited = edit(&mut lock.own);
    if edited.is_ok() {
        lock.publish()?;
    }
    Ok(edited)
}

/// `add` leaves a conflicted path's stages 1-3 behind, so the index stays
/// unmerged; `add_path` resolves them itself. Drop them before an `add`.
pub(super) fn resolve_conflict(index: &mut git2::Index, path: &Path) -> Result<(), git2::Error> {
    match index.conflict_remove(path) {
        Err(e) if e.code() == git2::ErrorCode::NotFound => Ok(()),
        other => other,
    }
}

/// `index.lock`, held by this process for one update.
struct IndexLock<'r> {
    repo: &'r git2::Repository,
    /// The repository's own index object.
    own: git2::Index,
    /// The repository points at the scratch index.
    swapped: bool,
    /// `index.lock` became the index: it is no longer ours to remove.
    published: bool,
}

impl<'r> IndexLock<'r> {
    /// Create `index.lock`. The handle is closed at once: an open handle
    /// makes the renames onto and of the lock fail on Windows.
    fn take(repo: &'r git2::Repository) -> Result<Self, git2::Error> {
        let own = repo.index()?;
        let lock = repo.path().join("index.lock");
        #[cfg(test)]
        run_index_update_hook();
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock)
            .map_err(|e| match e.kind() {
                ErrorKind::AlreadyExists => locked(&lock, &e),
                _ => io_error("take index.lock", &e),
            })?;
        Ok(Self {
            repo,
            own,
            swapped: false,
            published: false,
        })
    }

    fn path(&self, name: &str) -> PathBuf {
        self.repo.path().join(name)
    }

    /// Read the index again: only under the lock is it the one we replace.
    fn read(&mut self) -> Result<(), git2::Error> {
        #[cfg(test)]
        assert_lock_held(&self.path("index.lock"));
        self.own.read(true)
    }

    /// Write the edited index into `index.lock` and rename that onto
    /// `index`.
    fn publish(&mut self) -> Result<(), git2::Error> {
        let index_path = self.path("index");
        let scratch = self.path(SCRATCH_INDEX);
        // Leftovers of an update that crashed: nobody else uses them now.
        remove_if_present(&self.path(&format!("{SCRATCH_INDEX}.lock")))?;
        remove_if_present(&scratch)?;
        // A copy of the index keeps its extensions (tree cache, resolve-undo).
        if index_path.exists() {
            fs::copy(&index_path, &scratch).map_err(|e| io_error("copy the git index", &e))?;
        }
        let mut out = git2::Index::open(&scratch)?;
        copy_entries(&self.own, &mut out)?;
        // Written as the repository's index, so libgit2 smudges racily
        // clean entries.
        self.repo.set_index(&mut out)?;
        self.swapped = true;
        let written = out.write();
        self.repo.set_index(&mut self.own)?;
        self.swapped = false;
        written?;
        let lock = self.path("index.lock");
        rename_or_locked(&scratch, &lock)?;
        rename_or_locked(&lock, &index_path)?;
        self.published = true;
        Ok(())
    }
}

impl Drop for IndexLock<'_> {
    fn drop(&mut self) {
        if self.swapped
            && let Err(e) = self.repo.set_index(&mut self.own)
        {
            warn_cleanup("the repository's index was not restored", &e);
        }
        if self.published {
            return;
        }
        // Unpublished edits must not linger in the repository's index.
        if let Err(e) = self.own.read(true) {
            warn_cleanup("unpublished index edits were not dropped", &e);
        }
        for name in [SCRATCH_INDEX, "index.lock"] {
            if let Err(e) = remove_if_present(&self.path(name)) {
                warn_cleanup("a git index lock file was not removed", &e);
            }
        }
    }
}

fn warn_cleanup(what: &str, e: &git2::Error) {
    tracing::warn!(target: "signex::library", error = %e, "{what}");
}

/// Make `to` hold exactly `from`'s entries, removing and adding only the
/// ones that differ, so `to`'s tree cache and resolve-undo records stay as
/// read for every untouched path. Resolve-undo records `from` gained are
/// not carried over: git2 has no API for them.
fn copy_entries(from: &git2::Index, to: &mut git2::Index) -> Result<(), git2::Error> {
    let wanted: HashMap<(Vec<u8>, u16), git2::IndexEntry> =
        from.iter().map(|e| (entry_key(&e), e)).collect();
    let mut unchanged = HashSet::new();
    let mut stale = Vec::new();
    for entry in to.iter() {
        let key = entry_key(&entry);
        match wanted.get(&key) {
            Some(want) if same_entry(want, &entry) => {
                unchanged.insert(key);
            }
            Some(_) => {}
            None => stale.push(key),
        }
    }
    for (path, stage) in &stale {
        to.remove(entry_path(path)?, i32::from(*stage))?;
    }
    for (key, entry) in &wanted {
        if !unchanged.contains(key) {
            to.add(entry)?;
        }
    }
    Ok(())
}

/// An entry's path and conflict stage: its identity in an index.
fn entry_key(entry: &git2::IndexEntry) -> (Vec<u8>, u16) {
    (entry.path.clone(), (entry.flags >> 12) & 0x3)
}

fn same_entry(a: &git2::IndexEntry, b: &git2::IndexEntry) -> bool {
    a.id == b.id
        && a.mode == b.mode
        && a.flags == b.flags
        && a.flags_extended == b.flags_extended
        && a.ctime == b.ctime
        && a.mtime == b.mtime
        && a.dev == b.dev
        && a.ino == b.ino
        && a.uid == b.uid
        && a.gid == b.gid
        && a.file_size == b.file_size
}

#[cfg(unix)]
fn entry_path(bytes: &[u8]) -> Result<&Path, git2::Error> {
    use std::os::unix::ffi::OsStrExt;
    Ok(Path::new(std::ffi::OsStr::from_bytes(bytes)))
}

#[cfg(not(unix))]
fn entry_path(bytes: &[u8]) -> Result<&Path, git2::Error> {
    std::str::from_utf8(bytes)
        .map(Path::new)
        .map_err(|_| git2::Error::from_str("git index path is not UTF-8"))
}

/// How a failed rename of the scratch index or `index.lock` is reported.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum RenameFailure {
    /// Another process has the target open: retried as a held lock.
    Locked,
    Failed,
}

/// On Windows a reader holding the target open makes the rename fail with
/// `PermissionDenied` for a moment, as git's own rename of `index.lock` can.
/// Elsewhere that error is real.
pub(super) fn rename_failure(kind: ErrorKind, is_windows: bool) -> RenameFailure {
    if is_windows && kind == ErrorKind::PermissionDenied {
        RenameFailure::Locked
    } else {
        RenameFailure::Failed
    }
}

fn rename_or_locked(from: &Path, to: &Path) -> Result<(), git2::Error> {
    fs::rename(from, to).map_err(|e| match rename_failure(e.kind(), cfg!(windows)) {
        RenameFailure::Locked => locked(to, &e),
        RenameFailure::Failed => io_error(&format!("rename onto {}", to.display()), &e),
    })
}

fn locked(path: &Path, e: &std::io::Error) -> git2::Error {
    git2::Error::new(
        git2::ErrorCode::Locked,
        git2::ErrorClass::Index,
        format!("{} is held by another git process: {e}", path.display()),
    )
}

fn remove_if_present(path: &Path) -> Result<(), git2::Error> {
    match fs::remove_file(path) {
        Err(e) if e.kind() != ErrorKind::NotFound => Err(io_error("remove a stale scratch", &e)),
        _ => Ok(()),
    }
}

fn io_error(what: &str, e: &std::io::Error) -> git2::Error {
    git2::Error::from_str(&format!("{what}: {e}"))
}

/// Test builds check that the index is only ever read under our lock: a
/// second create-new of `index.lock` must find it there.
#[cfg(test)]
fn assert_lock_held(lock: &Path) {
    let again = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(lock);
    assert!(
        matches!(&again, Err(e) if e.kind() == ErrorKind::AlreadyExists),
        "the git index is read without holding {}",
        lock.display()
    );
}

#[cfg(test)]
thread_local! {
    /// Runs once, on this thread, right before the next index update tries
    /// to create `index.lock`: where a test lets another lock holder finish.
    static INDEX_UPDATE: std::cell::RefCell<Option<Box<dyn FnOnce()>>> =
        const { std::cell::RefCell::new(None) };
}

/// Arrange for `hook` to run once inside the next index update this
/// thread makes (see [`INDEX_UPDATE`]).
#[cfg(test)]
pub(super) fn set_index_update_hook(hook: impl FnOnce() + 'static) {
    INDEX_UPDATE.with(|slot| *slot.borrow_mut() = Some(Box::new(hook)));
}

#[cfg(test)]
fn run_index_update_hook() {
    if let Some(hook) = INDEX_UPDATE.with(|slot| slot.borrow_mut().take()) {
        hook();
    }
}
