//! Updates of the repository's on-disk index made under git's own
//! `index.lock` protocol (#2337).

use std::fs;
use std::io::ErrorKind;
use std::path::Path;

/// The scratch index an update is built in, inside the git dir. libgit2
/// writes an index only to its own path, so the edit is written here and
/// then moved onto `index.lock`. Only the holder of `index.lock` touches it.
const SCRATCH_INDEX: &str = "index.signex-scratch";

/// Apply `edit` to the repository's on-disk index under git's lock
/// protocol, so the index `edit` changes is the one it replaces.
/// libgit2's `Index::write` locks only for the write; an index read before
/// it can be one another process replaced meanwhile, and writing it back
/// drops what that process staged.
///
/// `index.lock` is created first (create-new, as git does), the index is
/// read and edited as the repository's index, written into `index.lock`,
/// and `index.lock` is renamed onto `index`: one step that publishes and
/// unlocks. `Err` is a failure of the update itself; a held lock is
/// `ErrorCode::Locked`, which `retry_on_git_lock` waits out. `Ok(Err(_))`
/// is `edit`'s own failure. Nothing is published on either, and only the
/// lock this call created is removed.
pub(super) fn update_index_locked<E>(
    repo: &git2::Repository,
    edit: impl FnOnce(&mut git2::Index) -> Result<(), E>,
) -> Result<Result<(), E>, git2::Error> {
    let git_dir = repo.path();
    let lock = git_dir.join("index.lock");
    take_lock(&lock)?;
    let updated = edit_and_publish(repo, edit);
    if !matches!(updated, Ok(Ok(()))) {
        let cleaned =
            remove_if_present(&git_dir.join(SCRATCH_INDEX)).and_then(|()| remove_if_present(&lock));
        if let Err(e) = cleaned {
            tracing::warn!(target: "signex::library", error = %e, "git index lock not cleaned up");
        }
    }
    updated
}

/// `add` leaves a conflicted path's stages 1-3 behind, so the index stays
/// unmerged; `add_path` resolves them itself. Drop them before an `add`.
pub(super) fn resolve_conflict(index: &mut git2::Index, path: &Path) -> Result<(), git2::Error> {
    match index.conflict_remove(path) {
        Err(e) if e.code() == git2::ErrorCode::NotFound => Ok(()),
        other => other,
    }
}

/// Create `index.lock`. The handle is closed at once: an open handle makes
/// the renames onto and of the lock fail on Windows.
fn take_lock(lock: &Path) -> Result<(), git2::Error> {
    let taken = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(lock)
        .map(drop);
    #[cfg(test)]
    run_index_update_hook();
    match taken {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == ErrorKind::AlreadyExists => Err(locked(lock)),
        Err(e) => Err(io_error("take index.lock", &e)),
    }
}

/// Read, edit and write the index, then publish it. The caller holds
/// `index.lock`.
fn edit_and_publish<E>(
    repo: &git2::Repository,
    edit: impl FnOnce(&mut git2::Index) -> Result<(), E>,
) -> Result<Result<(), E>, git2::Error> {
    let git_dir = repo.path();
    let scratch = git_dir.join(SCRATCH_INDEX);
    let mut index = read_into_scratch(git_dir)?;
    // Edited as the repository's index, so `add_path` sees the working tree
    // and its filters and `write` smudges racily clean entries. The
    // repository's own index object is put back afterwards.
    let mut own = repo.index()?;
    repo.set_index(&mut index)?;
    let edited = edit(&mut index);
    let written = if edited.is_ok() {
        index.write()
    } else {
        Ok(())
    };
    let restored = repo.set_index(&mut own);
    written?;
    restored?;
    if edited.is_err() {
        return Ok(edited);
    }
    let lock = git_dir.join("index.lock");
    rename_or_locked(&scratch, &lock, "fill index.lock")?;
    rename_or_locked(&lock, &git_dir.join("index"), "publish the git index")?;
    Ok(edited)
}

/// Copy the index into the scratch file and open it. Only while holding
/// `index.lock`: the copy is the index the update replaces.
fn read_into_scratch(git_dir: &Path) -> Result<git2::Index, git2::Error> {
    #[cfg(test)]
    assert_lock_held(&git_dir.join("index.lock"));
    let index_path = git_dir.join("index");
    let scratch = git_dir.join(SCRATCH_INDEX);
    // Leftovers of an update that crashed: nobody else uses them now.
    remove_if_present(&git_dir.join(format!("{SCRATCH_INDEX}.lock")))?;
    remove_if_present(&scratch)?;
    if index_path.exists() {
        fs::copy(&index_path, &scratch).map_err(|e| io_error("copy the git index", &e))?;
    }
    git2::Index::open(&scratch)
}

/// Rename `from` onto `to`. On Windows a reader holding `to` open makes the
/// rename fail with `PermissionDenied` for a moment; that is reported as
/// `Locked`, so `retry_on_git_lock` redoes the update.
fn rename_or_locked(from: &Path, to: &Path, what: &str) -> Result<(), git2::Error> {
    fs::rename(from, to).map_err(|e| {
        if cfg!(windows) && e.kind() == ErrorKind::PermissionDenied {
            locked(to)
        } else {
            io_error(what, &e)
        }
    })
}

fn locked(path: &Path) -> git2::Error {
    git2::Error::new(
        git2::ErrorCode::Locked,
        git2::ErrorClass::Index,
        format!("{} is held by another git process", path.display()),
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
    /// Runs once, on this thread, right after the next index update's
    /// attempt to create `index.lock`, whether or not it got the lock:
    /// where a test lets another lock holder finish.
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
