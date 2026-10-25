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
