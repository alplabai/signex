//! The double-failure rollback of a library rename (#739): the reverse
//! patch of the saved `.snxprj` fails after the mount could not follow.

use super::*;

/// A corrupt project file makes `retarget_library_entry` fail (it parses
/// before it writes), which is how the reverse patch fails here.
#[test]
fn a_failed_reverse_patch_reports_the_stale_project_without_dirtying_it() {
    let tmp = tempfile::TempDir::new().unwrap();
    let project_path = tmp.path().join("proj.snxprj");
    std::fs::write(&project_path, b"{ not json").unwrap();
    let old_path = tmp.path().join("alpha/alpha.snxlib");
    let new_path = tmp.path().join("alpha/alpha2.snxlib");
    let (mut app, _task) = Signex::new();

    let suffix = app
        .undo_project_patch(&project_path, &old_path, &new_path)
        .unwrap_err();

    assert!(suffix.contains("Save the project"), "got: {suffix}");
    assert!(
        app.document_state.dirty_paths.is_empty(),
        "dirtying waits until the file is back at the old path"
    );
}

#[test]
fn the_stale_project_is_dirtied_only_when_the_file_moves_back() {
    let tmp = tempfile::TempDir::new().unwrap();
    let project_path = tmp.path().join("proj.snxprj");
    let old_path = tmp.path().join("alpha.snxlib");
    let new_path = tmp.path().join("alpha2.snxlib");
    std::fs::write(&new_path, b"x").unwrap();
    let (mut app, _task) = Signex::new();

    app.undo_library_file_rename(&old_path, &new_path, "r", Some(&project_path));
    assert!(old_path.exists());
    assert!(app.document_state.dirty_paths.contains(&project_path));

    // Move-back fails (source missing): the project stays clean.
    app.document_state.dirty_paths.clear();
    app.undo_library_file_rename(
        &old_path,
        &tmp.path().join("gone.snxlib"),
        "r",
        Some(&project_path),
    );
    assert!(app.document_state.dirty_paths.is_empty());
}

#[test]
fn a_successful_reverse_patch_leaves_the_project_clean() {
    let tmp = tempfile::TempDir::new().unwrap();
    let project_path = tmp.path().join("missing.snxprj");
    let old_path = tmp.path().join("alpha/alpha.snxlib");
    let new_path = tmp.path().join("alpha/alpha2.snxlib");
    let (mut app, _task) = Signex::new();

    assert!(
        app.undo_project_patch(&project_path, &old_path, &new_path)
            .is_ok()
    );
    assert!(app.document_state.dirty_paths.is_empty());
}
