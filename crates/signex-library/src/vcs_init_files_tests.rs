use super::*;

#[test]
fn a_git_dir_that_was_never_created_is_not_a_rollback_failure() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(remove_new_git_dir(dir.path()), None);
}

#[test]
fn the_new_git_dir_is_removed_without_a_note() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".git/objects")).unwrap();
    assert_eq!(remove_new_git_dir(dir.path()), None);
    assert!(!dir.path().join(".git").exists());
}

#[test]
fn a_git_dir_that_cannot_be_removed_is_reported() {
    // A plain file named `.git` cannot be removed as a directory tree.
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(".git"), "x").unwrap();
    let note = remove_new_git_dir(dir.path()).expect("a note");
    assert!(
        note.starts_with("could not remove the new .git directory"),
        "{note}"
    );
}

fn plan(dir: &Path) -> VcsFilePlan {
    VcsFilePlan::new(dir, true, true, None).unwrap()
}

#[test]
fn a_second_file_failing_puts_the_first_back() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(".gitattributes"), "*.x text\n").unwrap();
    std::fs::write(dir.path().join(".gitignore"), "mine\n").unwrap();
    let mut files = plan(dir.path());
    // Swap the second file (.gitignore) for a directory after planning, so
    // its atomic write fails once the first (.gitattributes) is on disk.
    std::fs::remove_file(dir.path().join(".gitignore")).unwrap();
    std::fs::create_dir(dir.path().join(".gitignore")).unwrap();

    let err = files.write().unwrap_err();

    assert!(err.to_string().contains(".gitignore"), "{err}");
    assert_ne!(
        std::fs::read_to_string(dir.path().join(".gitattributes")).unwrap(),
        "*.x text\n",
        "the first file was written before the failure"
    );
    assert_eq!(files.roll_back(), Vec::<String>::new());
    assert_eq!(
        std::fs::read_to_string(dir.path().join(".gitattributes")).unwrap(),
        "*.x text\n"
    );
}

#[test]
fn rolling_back_a_file_that_did_not_exist_removes_it() {
    let dir = tempfile::tempdir().unwrap();
    let mut files = plan(dir.path());
    std::fs::create_dir(dir.path().join(".gitignore")).unwrap();

    assert!(files.write().is_err());
    assert!(files.roll_back().is_empty());
    assert!(!dir.path().join(".gitattributes").exists());
}

#[test]
fn a_file_the_user_edited_after_the_write_survives_the_rollback() {
    let dir = tempfile::tempdir().unwrap();
    let ignore = dir.path().join(".gitignore");
    let attrs = dir.path().join(".gitattributes");
    std::fs::write(&ignore, "mine\n").unwrap();
    let mut files = plan(dir.path());
    files.write().unwrap();
    std::fs::write(&ignore, "edited meanwhile\n").unwrap();

    let notes = files.roll_back();

    assert_eq!(
        std::fs::read_to_string(&ignore).unwrap(),
        "edited meanwhile\n"
    );
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert!(notes[0].contains("left as is"), "{notes:?}");
    assert!(!attrs.exists(), "the untouched file is still rolled back");
}

#[cfg(unix)]
#[test]
fn rolling_back_keeps_the_files_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(".gitattributes");
    std::fs::write(&path, "*.x text\n").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    let mut files = plan(dir.path());
    files.write().unwrap();
    assert!(files.roll_back().is_empty());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "*.x text\n");
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
}
