//! The double-failure rollback of a library rename (#739): the reverse
//! patch of the saved `.snxprj` fails after the mount could not follow.

use super::*;

/// A corrupt project file makes `retarget_library_entry` fail (it parses
/// before it writes), which is how the reverse patch fails here.
#[test]
fn a_failed_reverse_patch_dirties_the_project_so_a_save_rewrites_it() {
    let tmp = tempfile::TempDir::new().unwrap();
    let project_path = tmp.path().join("proj.snxprj");
    std::fs::write(&project_path, b"{ not json").unwrap();
    let old_path = tmp.path().join("alpha/alpha.snxlib");
    let new_path = tmp.path().join("alpha/alpha2.snxlib");
    let (mut app, _task) = Signex::new();

    let suffix = app.undo_project_patch(&project_path, &old_path, &new_path);

    assert!(
        app.document_state.dirty_paths.contains(&project_path),
        "the project must be dirty so a save restores the old-path entry"
    );
    assert!(suffix.contains("Save the project"), "got: {suffix}");
}

#[test]
fn a_successful_reverse_patch_leaves_the_project_clean() {
    let tmp = tempfile::TempDir::new().unwrap();
    let project_path = tmp.path().join("missing.snxprj");
    let old_path = tmp.path().join("alpha/alpha.snxlib");
    let new_path = tmp.path().join("alpha/alpha2.snxlib");
    let (mut app, _task) = Signex::new();

    let suffix = app.undo_project_patch(&project_path, &old_path, &new_path);

    assert!(suffix.is_empty());
    assert!(app.document_state.dirty_paths.is_empty());
}
