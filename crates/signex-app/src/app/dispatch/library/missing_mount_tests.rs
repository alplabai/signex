//! A project library that cannot be mounted: only a `.snxlib` the
//! filesystem confirms absent raises Library Missing (#1846). Anything
//! else must not offer "Remove from project" for a library still on disk.

use super::*;
use crate::library::LibraryMessage;
use crate::library::mount::{MountIntent, PreparedMountCell, prepare_mount};
use crate::library::recovery::RecoveryDialog;

/// Mount `path` the way the project-open fan-out does and return the app.
fn mount_silently(path: &std::path::Path) -> Signex {
    let (mut app, _task) = Signex::new();
    app.library.request_mount(path, MountIntent::Silent);
    let prepared = PreparedMountCell::new(prepare_mount(path));
    let _ = app.update(Message::Library(LibraryMessage::MountFinished {
        path: path.to_path_buf(),
        prepared,
    }));
    app
}

#[test]
fn a_missing_library_directory_raises_library_missing() {
    let dir = tempfile::tempdir().unwrap();
    let lib = dir.path().join("gone").join("cells.snxlib");
    let app = mount_silently(&lib);
    assert!(
        matches!(
            app.library.recovery,
            Some(RecoveryDialog::LibraryMissing { ref path }) if *path == lib
        ),
        "got {:?}",
        app.library.recovery
    );
}

#[test]
fn a_missing_library_file_in_an_existing_directory_raises_library_missing() {
    let dir = tempfile::tempdir().unwrap();
    let lib = dir.path().join("cells.snxlib");
    let app = mount_silently(&lib);
    assert!(matches!(
        app.library.recovery,
        Some(RecoveryDialog::LibraryMissing { .. })
    ));
}

#[test]
fn a_library_that_fails_to_parse_does_not_offer_remove_from_project() {
    let dir = tempfile::tempdir().unwrap();
    let lib = dir.path().join("cells.snxlib");
    std::fs::write(&lib, "not a library\n").unwrap();
    let app = mount_silently(&lib);
    assert!(app.library.recovery.is_none(), "{:?}", app.library.recovery);
    assert!(lib.exists());
}

#[cfg(unix)]
#[test]
fn an_unreadable_library_folder_does_not_offer_remove_from_project() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let locked = dir.path().join("locked");
    std::fs::create_dir(&locked).unwrap();
    let lib = locked.join("cells.snxlib");
    std::fs::write(&lib, "x").unwrap();
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
    // Root ignores the mode; only assert where the folder is really locked.
    let locked_out =
        std::fs::metadata(&lib).is_err_and(|e| e.kind() == std::io::ErrorKind::PermissionDenied);
    let app = mount_silently(&lib);
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
    if locked_out {
        assert!(app.library.recovery.is_none(), "{:?}", app.library.recovery);
    }
}

#[test]
fn a_not_found_row_error_is_not_a_missing_library() {
    let (mut app, _task) = Signex::new();
    let path = std::path::Path::new("/nowhere/cells.snxlib");
    let err = signex_library::LibraryError::NotFound("no .snxlib at somewhere".into());
    assert!(!route_open_error(&mut app.library, path, &err));
    assert!(app.library.recovery.is_none());
}
