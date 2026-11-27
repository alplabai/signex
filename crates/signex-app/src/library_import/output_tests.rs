//! What a new run takes as its output folder, and what a failed one
//! removes (#3117, #3123): only what it created.

use std::path::PathBuf;

use super::error::ImportError;
use super::output::{ClaimedOutput, Held, claim_output};
use super::run::{LibraryRunRequest, ModelsMode, RunMode};

fn request(output: PathBuf, mode: RunMode) -> LibraryRunRequest {
    LibraryRunRequest {
        inputs: vec![],
        output,
        name: None,
        db: None,
        search_paths: vec![],
        models: ModelsMode::Include,
        mode,
    }
}

fn claim(output: &std::path::Path, mode: RunMode) -> Result<ClaimedOutput, ImportError> {
    claim_output(&request(output.to_path_buf(), mode))
}

#[test]
fn a_new_run_creates_its_folder_and_refuses_one_that_holds_anything() {
    let dir = tempfile::tempdir().unwrap();
    let new = dir.path().join("a/b/New");
    let created = claim(&new, RunMode::Import).unwrap();
    assert_eq!(created.held, Held::Created);
    assert_eq!(created.created_parent, Some(dir.path().join("a")));
    assert!(new.is_dir(), "created with its parents");
    // An empty folder that is there is adopted as it is.
    let adopted = claim(&new, RunMode::Preview).unwrap();
    assert_eq!(adopted.held, Held::Adopted);
    assert_eq!(adopted.created_parent, None);
    std::fs::write(new.join("theirs.txt"), "keep").unwrap();
    assert_eq!(
        claim(&new, RunMode::Import),
        Err(ImportError::OutputNotEmpty { path: new.clone() })
    );
    assert!(new.join("theirs.txt").exists());
    let file = dir.path().join("file");
    std::fs::write(&file, "x").unwrap();
    assert!(claim(&file, RunMode::Import).is_err());
    assert_eq!(claim(&new, RunMode::Update).unwrap().held, Held::NotOwned);
}

/// A link is refused as a link, not as a folder that holds files, and
/// neither it nor the folder it points at is touched.
#[cfg(unix)]
#[test]
fn a_new_run_refuses_a_link_as_a_link() {
    let dir = tempfile::tempdir().unwrap();
    let real = dir.path().join("real");
    std::fs::create_dir(&real).unwrap();
    let link = dir.path().join("link");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    assert_eq!(
        claim(&link, RunMode::Import),
        Err(ImportError::OutputIsLink { path: link.clone() })
    );
    assert!(real.is_dir() && link.exists(), "both left as they were");
    let shown = ImportError::OutputIsLink { path: link }.to_string();
    assert!(shown.contains("is a link"), "{shown}");
}

#[test]
fn a_failed_run_removes_its_folder_and_the_parents_it_created() {
    let dir = tempfile::tempdir().unwrap();
    let new = dir.path().join("a/b/New");
    let claimed = claim(&new, RunMode::Import).unwrap();
    std::fs::create_dir(new.join("symbols")).unwrap();
    std::fs::write(new.join("symbols/R.snxsym"), "x").unwrap();
    assert!(claimed.release_after_failure());
    assert!(!dir.path().join("a").exists(), "the parents it made are gone");
    assert!(dir.path().is_dir(), "the folder that was there stays");
}

/// A created parent someone put something in meanwhile is kept, and so is
/// every parent above it.
#[test]
fn a_created_parent_that_is_not_empty_stays() {
    let dir = tempfile::tempdir().unwrap();
    let new = dir.path().join("a/b/New");
    let claimed = claim(&new, RunMode::Import).unwrap();
    std::fs::write(dir.path().join("a/keep.txt"), "keep").unwrap();
    assert!(claimed.release_after_failure());
    assert!(!dir.path().join("a/b").exists());
    assert!(dir.path().join("a/keep.txt").exists());
}

/// #3123: an empty folder that was there is the user's; a failed run
/// empties it of what it wrote and keeps the folder, with its permissions.
#[test]
fn a_failed_run_empties_a_folder_it_adopted_and_keeps_it() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("Mine");
    std::fs::create_dir(&out).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&out, std::fs::Permissions::from_mode(0o750)).unwrap();
    }
    let claimed = claim(&out, RunMode::Import).unwrap();
    std::fs::create_dir_all(out.join("symbols/deep")).unwrap();
    std::fs::write(out.join("symbols/deep/R.snxsym"), "x").unwrap();
    std::fs::write(out.join("Mine.snxlib"), "x").unwrap();
    assert!(claimed.release_after_failure());
    assert!(out.is_dir(), "the folder is kept");
    assert_eq!(std::fs::read_dir(&out).unwrap().count(), 0, "and emptied");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = std::fs::metadata(&out).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o750, "its permissions are its own");
    }
    assert!(dir.path().is_dir());
}

/// Emptying never follows a link the run left in the folder: the link goes,
/// what it points at stays.
#[cfg(unix)]
#[test]
fn emptying_an_adopted_folder_never_follows_a_link() {
    let dir = tempfile::tempdir().unwrap();
    let elsewhere = dir.path().join("elsewhere");
    std::fs::create_dir(&elsewhere).unwrap();
    std::fs::write(elsewhere.join("precious.txt"), "keep").unwrap();
    let out = dir.path().join("Mine");
    std::fs::create_dir(&out).unwrap();
    let claimed = claim(&out, RunMode::Import).unwrap();
    std::os::unix::fs::symlink(&elsewhere, out.join("link")).unwrap();
    std::fs::create_dir(out.join("sub")).unwrap();
    std::os::unix::fs::symlink(&elsewhere, out.join("sub/link")).unwrap();
    assert!(claimed.release_after_failure());
    assert_eq!(std::fs::read_dir(&out).unwrap().count(), 0);
    assert!(elsewhere.join("precious.txt").exists());
}

/// An update's folder is never this run's to remove or empty.
#[test]
fn a_failed_update_leaves_its_folder_to_the_restore() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("Lib");
    std::fs::create_dir(&out).unwrap();
    std::fs::write(out.join("Lib.snxlib"), "x").unwrap();
    let claimed = claim(&out, RunMode::Update).unwrap();
    assert!(!claimed.release_after_failure());
    assert!(out.join("Lib.snxlib").exists());
}
