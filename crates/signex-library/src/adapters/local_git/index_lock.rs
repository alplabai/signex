//! Updates of the repository's on-disk index made under git's own
//! `index.lock` (#2337).

use std::fs;
use std::io::ErrorKind;
use std::path::Path;

/// The scratch index an update is built in, inside the git dir. Only a
/// process holding `index.lock` touches it.
const SCRATCH_INDEX: &str = "index.signex-scratch";

/// Apply `edit` to the repository's on-disk index while holding git's
/// `index.lock`, so the index `edit` changes is the one it replaces: the
/// lock is taken (create-new, as git takes it) before the index is read.
/// libgit2's own `Index::write` locks only for the write, so an index read
/// before it can be one another process replaced meanwhile, and writing it
/// back drops what that process staged.
///
/// A held lock is `ErrorCode::Locked`, which `retry_on_git_lock` waits out.
/// `edit` gets an index with no repository behind it: it adds prepared
/// entries (`staged_entry`) instead of calling `add_path`.
pub(super) fn update_index_locked(
    repo: &git2::Repository,
    edit: impl FnOnce(&mut git2::Index) -> Result<(), git2::Error>,
) -> Result<(), git2::Error> {
    let git_dir = repo.path();
    let lock = git_dir.join("index.lock");
    let taken = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock);
    #[cfg(test)]
    run_index_update_hook();
    match taken {
        Ok(_) => {}
        Err(e) if e.kind() == ErrorKind::AlreadyExists => {
            return Err(git2::Error::new(
                git2::ErrorCode::Locked,
                git2::ErrorClass::Index,
                format!("{} exists; another git process holds it", lock.display()),
            ));
        }
        Err(e) => return Err(io_error("take index.lock", &e)),
    }
    let written = write_under_lock(git_dir, edit);
    let unlocked = fs::remove_file(&lock);
    written?;
    unlocked.map_err(|e| io_error("release index.lock", &e))
}

/// Read the index into the scratch copy, edit it, write it and publish it
/// over the index. The caller holds `index.lock`.
fn write_under_lock(
    git_dir: &Path,
    edit: impl FnOnce(&mut git2::Index) -> Result<(), git2::Error>,
) -> Result<(), git2::Error> {
    let index_path = git_dir.join("index");
    let scratch = git_dir.join(SCRATCH_INDEX);
    // Leftovers of an update that crashed: nobody else uses them now.
    remove_if_present(&git_dir.join(format!("{SCRATCH_INDEX}.lock")))?;
    remove_if_present(&scratch)?;
    if index_path.exists() {
        fs::copy(&index_path, &scratch).map_err(|e| io_error("copy the git index", &e))?;
    }
    let mut index = git2::Index::open(&scratch)?;
    edit(&mut index)?;
    index.write()?;
    fs::rename(&scratch, &index_path).map_err(|e| io_error("publish the git index", &e))
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

#[cfg(test)]
thread_local! {
    /// Runs once, on this thread, inside the next index update attempt,
    /// after that attempt fixed its view of the index and before it writes:
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
pub(super) fn run_index_update_hook() {
    if let Some(hook) = INDEX_UPDATE.with(|slot| slot.borrow_mut().take()) {
        hook();
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use super::super::helpers::stage_and_commit;

    /// Another process holds `index.lock` while our save updates the index,
    /// then publishes an index with a path it staged and unlocks. Our
    /// update must not write back the index it saw before that publish.
    #[test]
    fn an_index_published_by_the_lock_holder_keeps_its_staged_path() {
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        let sig = git2::Signature::now("Test", "test@example.test").unwrap();
        fs::write(dir.path().join("file.txt"), "one\n").unwrap();
        stage_and_commit(&repo, &[], Path::new("file.txt"), None, &sig, "first")
            .unwrap()
            .expect("first commit");

        // The index the lock holder will publish: ours plus `other.txt`.
        let git = repo.path().to_path_buf();
        fs::write(dir.path().join("other.txt"), "other\n").unwrap();
        let original = fs::read(git.join("index")).unwrap();
        {
            let holder = git2::Repository::open(dir.path()).unwrap();
            let mut index = holder.index().unwrap();
            index.add_path(Path::new("other.txt")).unwrap();
            index.write().unwrap();
        }
        let published = fs::read(git.join("index")).unwrap();
        fs::write(git.join("index"), &original).unwrap();
        fs::write(git.join("index.lock"), b"").unwrap();

        let holder_git = git.clone();
        super::set_index_update_hook(move || {
            fs::write(holder_git.join("index.new"), &published).unwrap();
            fs::rename(holder_git.join("index.new"), holder_git.join("index")).unwrap();
            fs::remove_file(holder_git.join("index.lock")).unwrap();
        });
        fs::write(dir.path().join("file.txt"), "two\n").unwrap();
        stage_and_commit(&repo, &[], Path::new("file.txt"), None, &sig, "second")
            .unwrap()
            .expect("second commit");

        let after = git2::Repository::open(dir.path()).unwrap();
        let index = after.index().unwrap();
        assert!(
            index.get_path(Path::new("other.txt"), 0).is_some(),
            "the lock holder's staged path was dropped"
        );
        let head = after.head().unwrap().peel_to_tree().unwrap();
        let committed = head.get_path(Path::new("file.txt")).unwrap().id();
        assert_eq!(
            index.get_path(Path::new("file.txt"), 0).map(|e| e.id),
            Some(committed),
            "the saved path's index entry is not the committed blob"
        );
        assert!(!git.join("index.lock").exists(), "index.lock left behind");
    }
}
