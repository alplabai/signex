//! A project library that cannot be mounted: only a `.snxlib` the
//! filesystem confirms absent raises Library Missing (#1846). Anything
//! else must not offer "Remove from project" for a library still on disk.

use super::*;
use crate::library::LibraryMessage;
use crate::library::mount::{MountIntent, PreparedMountCell, prepare_mount};
use crate::library::recovery::RecoveryDialog;

/// Register an open project whose library list holds `libs` (absolute).
fn add_project(app: &mut Signex, libs: &[&std::path::Path]) {
    use signex_types::project::{LibraryEntry, LibraryEntryKind, ProjectData};
    let id = app.document_state.mint_project_id();
    app.document_state
        .projects
        .push(crate::app::state::LoadedProject {
            id,
            path: std::path::PathBuf::from("/nowhere/P.snxprj"),
            data: ProjectData {
                description: None,
                name: "P".to_string(),
                dir: "/nowhere".to_string(),
                schematic_root: None,
                pcb_file: None,
                sheets: Vec::new(),
                variant_definitions: Vec::new(),
                active_variant: None,
                libraries: libs
                    .iter()
                    .map(|p| LibraryEntry {
                        path: p.to_path_buf(),
                        kind: LibraryEntryKind::Shared,
                        library_id: None,
                        unknown: Default::default(),
                    })
                    .collect(),
                enable_git: false,
                net_classes: Vec::new(),
                rules: None,
                annotation: None,
                parameters: Default::default(),
                unknown: Default::default(),
            },
            pending_libraries: Default::default(),
            case_fold: Default::default(),
            case_probed: false,
            case_decided: false,
        });
}

/// Deliver a finished mount of `path` carrying `result` into `app`.
fn finish(
    app: &mut Signex,
    path: &std::path::Path,
    intent: MountIntent,
    result: Result<crate::library::mount::PreparedMount, signex_library::LibraryError>,
) {
    app.library.request_mount(path, intent);
    let _ = app.update(Message::Library(LibraryMessage::MountFinished {
        path: path.to_path_buf(),
        prepared: PreparedMountCell::new(result),
    }));
}

/// Mount `path` as a library of an open project, the way the project-open
/// fan-out does, and return the app.
fn mount_silently(path: &std::path::Path) -> Signex {
    let (mut app, _task) = Signex::new();
    add_project(&mut app, &[path]);
    finish(&mut app, path, MountIntent::Silent, prepare_mount(path));
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
    } else {
        eprintln!("skipped: the folder mode is not enforced here (running as root?)");
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

#[test]
fn a_missing_global_library_at_startup_raises_no_dialog_but_a_project_one_does() {
    let dir = tempfile::tempdir().unwrap();
    let global = dir.path().join("global.snxlib");
    let project_lib = dir.path().join("proj.snxlib");
    let (mut app, _task) = Signex::new();
    add_project(&mut app, &[&project_lib]);
    finish(
        &mut app,
        &global,
        MountIntent::Silent,
        prepare_mount(&global),
    );
    assert!(app.library.recovery.is_none(), "{:?}", app.library.recovery);
    finish(
        &mut app,
        &project_lib,
        MountIntent::Silent,
        prepare_mount(&project_lib),
    );
    assert!(matches!(
        app.library.recovery,
        Some(RecoveryDialog::LibraryMissing { ref path }) if *path == project_lib
    ));
}

#[test]
fn a_silent_mount_with_version_control_trouble_only_logs() {
    let (mut app, _task) = Signex::new();
    let lib = std::path::Path::new("/nowhere/cells.snxlib");
    add_project(&mut app, &[lib]);
    let err = signex_library::LibraryError::Backend("git open: broken gitlink".into());
    finish(&mut app, lib, MountIntent::Silent, Err(err));
    assert!(app.library.recovery.is_none(), "{:?}", app.library.recovery);
}

#[test]
fn a_browser_open_of_a_missing_file_raises_the_dialog_not_an_error_card() {
    let dir = tempfile::tempdir().unwrap();
    let lib = dir.path().join("cells.snxlib");
    let (mut app, _task) = Signex::new();
    finish(
        &mut app,
        &lib,
        MountIntent::OpenBrowserTab,
        prepare_mount(&lib),
    );
    assert!(matches!(
        app.library.recovery,
        Some(RecoveryDialog::LibraryMissing { .. })
    ));
    assert!(app.document_state.error_notice.is_none());
}
