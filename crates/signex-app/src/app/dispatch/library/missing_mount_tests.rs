//! A project library that cannot be mounted: only a `.snxlib` the
//! filesystem confirms absent raises Library Missing (#1846). Anything
//! else must not offer "Remove from project" for a library still on disk.

use super::*;
use crate::library::LibraryMessage;
use crate::library::mount::{MountIntent, PreparedMountCell, prepare_mount};
use crate::library::recovery::RecoveryDialog;

/// Register an open project whose library list holds `libs` (absolute).
fn add_project(app: &mut Signex, libs: &[&std::path::Path]) -> crate::app::ProjectId {
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
    id
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

fn two_missing(
    app: &mut Signex,
) -> (
    std::path::PathBuf,
    std::path::PathBuf,
    crate::app::ProjectId,
) {
    let dir = tempfile::tempdir().unwrap().keep();
    let (a, b) = (dir.join("a.snxlib"), dir.join("b.snxlib"));
    let id = add_project(app, &[&a, &b]);
    for lib in [&a, &b] {
        finish(app, lib, MountIntent::Silent, prepare_mount(lib));
    }
    (a, b, id)
}

fn open_path(app: &Signex) -> Option<std::path::PathBuf> {
    match &app.library.recovery {
        Some(RecoveryDialog::LibraryMissing { path }) => Some(path.clone()),
        _ => None,
    }
}

#[test]
fn two_missing_libraries_raise_two_dialogs_in_turn() {
    let (mut app, _task) = Signex::new();
    let (a, b, _) = two_missing(&mut app);
    assert_eq!(open_path(&app), Some(a));
    assert_eq!(app.library.missing.waiting_len(), 1);
    let _ = app.update(Message::Library(LibraryMessage::RecoveryLibraryMissing(
        crate::library::recovery::LibraryMissingChoice::Cancel,
    )));
    assert_eq!(open_path(&app), Some(b));
    let _ = app.update(Message::Library(LibraryMessage::RecoveryLibraryMissing(
        crate::library::recovery::LibraryMissingChoice::Cancel,
    )));
    assert!(app.library.recovery.is_none());
}

#[test]
fn closing_the_project_prunes_its_queued_dialogs_and_the_open_one() {
    let (mut app, _task) = Signex::new();
    let (_, _, id) = two_missing(&mut app);
    app.library.forget_missing_libraries_of(id, |_| None);
    assert!(app.library.recovery.is_none());
    assert_eq!(app.library.missing.waiting_len(), 0);
}

#[test]
fn a_closed_projects_dialogs_pass_to_another_project_that_lists_them() {
    let (mut app, _task) = Signex::new();
    let (a, b, first) = two_missing(&mut app);
    let second = add_project(&mut app, &[&a, &b]);
    app.library
        .forget_missing_libraries_of(first, |_| Some(second));
    assert_eq!(open_path(&app), Some(a), "the open dialog stays");
    assert_eq!(app.library.missing.waiting_len(), 1, "the queued one too");
    // Closing the new owner then drops them.
    app.library.forget_missing_libraries_of(second, |_| None);
    assert!(app.library.recovery.is_none());
    assert_eq!(app.library.missing.waiting_len(), 0);
}

#[test]
fn remove_from_project_marks_the_project_dirty_and_shows_the_next_dialog() {
    use crate::library::recovery::LibraryMissingChoice::RemoveFromProject;
    let (mut app, _task) = Signex::new();
    let (a, b, _) = two_missing(&mut app);
    let _ = app.update(Message::Library(LibraryMessage::RecoveryLibraryMissing(
        RemoveFromProject,
    )));
    let project = &app.document_state.projects[0];
    assert_eq!(project.data.libraries.len(), 1);
    assert_eq!(
        project
            .data
            .resolve_library_path(&project.data.libraries[0]),
        b
    );
    let project_path = project.path.clone();
    assert!(app.document_state.is_document_dirty(&project_path));
    assert_ne!(a, b);
    assert_eq!(open_path(&app), Some(b));
}

#[test]
fn locate_rewrites_the_projects_entry_and_marks_it_dirty() {
    use crate::library::recovery::LibraryMissingChoice::Locate;
    let (mut app, _task) = Signex::new();
    let (a, b, _) = two_missing(&mut app);
    let _ = app.update(Message::Library(LibraryMessage::RecoveryLibraryMissing(
        Locate,
    )));
    let moved = a.with_file_name("moved.snxlib");
    crate::library::commands::create_library_file(&moved, &Default::default()).unwrap();
    let _ = app.update(Message::Library(
        LibraryMessage::RecoveryLibraryMissingLocateResult(Some(moved.clone())),
    ));
    assert!(app.library.library_at(&moved).is_some(), "the pick mounted");
    let project = &app.document_state.projects[0];
    assert_eq!(
        project
            .data
            .resolve_library_path(&project.data.libraries[0]),
        moved
    );
    let project_path = project.path.clone();
    assert!(app.document_state.is_document_dirty(&project_path));
    assert_eq!(open_path(&app), Some(b));
}

#[test]
fn a_pick_that_does_not_open_leaves_the_project_entry_alone() {
    use crate::library::recovery::LibraryMissingChoice::Locate;
    let (mut app, _task) = Signex::new();
    let (a, _, _) = two_missing(&mut app);
    let _ = app.update(Message::Library(LibraryMessage::RecoveryLibraryMissing(
        Locate,
    )));
    let bogus = a.with_file_name("not-there.snxlib");
    let _ = app.update(Message::Library(
        LibraryMessage::RecoveryLibraryMissingLocateResult(Some(bogus)),
    ));
    let project = &app.document_state.projects[0];
    assert_eq!(
        project
            .data
            .resolve_library_path(&project.data.libraries[0]),
        a
    );
    let project_path = project.path.clone();
    assert!(!app.document_state.is_document_dirty(&project_path));
}

#[test]
fn a_second_locate_while_picking_is_ignored() {
    use crate::library::recovery::LibraryMissingChoice::Locate;
    let (mut app, _task) = Signex::new();
    let (a, b, _) = two_missing(&mut app);
    let _ = app.update(Message::Library(LibraryMessage::RecoveryLibraryMissing(
        Locate,
    )));
    let _ = app.update(Message::Library(LibraryMessage::RecoveryLibraryMissing(
        Locate,
    )));
    let moved = a.with_file_name("moved.snxlib");
    crate::library::commands::create_library_file(&moved, &Default::default()).unwrap();
    let _ = app.update(Message::Library(
        LibraryMessage::RecoveryLibraryMissingLocateResult(Some(moved)),
    ));
    assert_eq!(open_path(&app), Some(b.clone()));
    // A stray second result never closes the dialog that moved on.
    let _ = app.update(Message::Library(
        LibraryMessage::RecoveryLibraryMissingLocateResult(None),
    ));
    assert_eq!(open_path(&app), Some(b));
}
