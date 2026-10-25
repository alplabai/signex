//! #731 — a project-tree action on a library row must act on exactly the
//! library that row represents, even when two libraries share a file name.
//!
//! Rows resolve by the library identity they carry, and are refused when the
//! library has left the project.

use signex_app::app::{
    ContextMenuMsg, LibraryRowRef, LoadedProject, Message, ProjectTreeAction, RemoveChoice,
    RemoveMsg, Signex, UiMsg,
};
use signex_types::project::{LibraryEntry, LibraryEntryKind, ProjectData};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// Row paths in the Libraries branch: the project has no sheets, so the
/// branch is the project's only child.
const LOCAL_ROW: [usize; 3] = [0, 0, 0];
const SHARED_ROW: [usize; 3] = [0, 0, 1];
/// Second row of a two-library project (the Shared row in the mixed fixture).
const SECOND_ROW: [usize; 3] = SHARED_ROW;

struct Fixture {
    app: Signex,
    _project_tmp: TempDir,
    _shared_tmp: TempDir,
    local_lib: PathBuf,
    shared_lib: PathBuf,
}

/// An app holding one project whose library list is `libraries`, with the
/// project tree built.
fn app_with_libraries(
    project_path: &Path,
    project_dir: &Path,
    libraries: Vec<LibraryEntry>,
) -> Signex {
    let (mut app, _boot) = Signex::new();
    let id = app.document_state.mint_project_id();
    app.document_state.projects.push(LoadedProject {
        id,
        path: project_path.to_path_buf(),
        data: ProjectData {
            name: "Board".into(),
            dir: project_dir.to_string_lossy().into_owned(),
            schematic_root: None,
            pcb_file: None,
            sheets: Vec::new(),
            variant_definitions: Vec::new(),
            active_variant: None,
            libraries,
            enable_git: false,
            unknown: Default::default(),
        },
        pending_libraries: HashMap::new(),
        case_fold: Default::default(),
        case_probed: false,
    });
    app.document_state.active_project = Some(id);
    // Any message that republishes the panel context builds the tree.
    let _ = app.update(Message::Ui(UiMsg::GridPickerSelect(1.0)));
    app
}

/// A project listing a ProjectLocal and a Shared library that are both named
/// `Power.snxlib`, with the project tree built.
fn two_libraries_one_name() -> Fixture {
    let project_tmp = TempDir::new().expect("project tempdir");
    let shared_tmp = TempDir::new().expect("shared tempdir");
    let local_lib = project_tmp.path().join("Power.snxlib");
    let shared_lib = shared_tmp.path().join("Power.snxlib");
    fs::write(&local_lib, b"local-library").expect("write local library");
    fs::write(&shared_lib, b"shared-library").expect("write shared library");
    let project_path = project_tmp.path().join("Board.snxprj");
    fs::write(&project_path, b"{}").expect("write .snxprj");

    let app = app_with_libraries(
        &project_path,
        project_tmp.path(),
        vec![
            LibraryEntry {
                path: PathBuf::from("Power.snxlib"),
                kind: LibraryEntryKind::ProjectLocal,
                library_id: None,
                unknown: Default::default(),
            },
            LibraryEntry {
                path: shared_lib.clone(),
                kind: LibraryEntryKind::Shared,
                library_id: None,
                unknown: Default::default(),
            },
        ],
    );

    Fixture {
        app,
        _project_tmp: project_tmp,
        _shared_tmp: shared_tmp,
        local_lib,
        shared_lib,
    }
}

fn tree_action(app: &mut Signex, action: ProjectTreeAction) {
    let _ = app.update(Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(
        action,
    )));
}

/// The stable identity a row carries once the context menu is open.
fn row_ref(f: &Fixture, root: &Path) -> LibraryRowRef {
    LibraryRowRef {
        project_id: f.app.document_state.projects[0].id,
        root: root.to_path_buf(),
    }
}

fn library_paths(app: &Signex) -> Vec<PathBuf> {
    let project = &app.document_state.projects[0];
    project
        .data
        .libraries
        .iter()
        .map(|e| project.data.resolve_library_path(e))
        .collect()
}

#[test]
fn remove_dialog_targets_the_clicked_library_row() {
    let mut f = two_libraries_one_name();

    tree_action(
        &mut f.app,
        ProjectTreeAction::OpenRemoveDialog(SHARED_ROW.to_vec()),
    );

    let dialog = f.app.ui_state.remove_dialog.as_ref().expect("dialog open");
    assert_eq!(dialog.target_path, f.shared_lib);
}

#[test]
fn rename_dialog_targets_the_clicked_library_row() {
    let mut f = two_libraries_one_name();

    tree_action(
        &mut f.app,
        ProjectTreeAction::OpenRenameDialog(SHARED_ROW.to_vec()),
    );

    let dialog = f.app.ui_state.rename_dialog.as_ref().expect("dialog open");
    assert_eq!(dialog.target_path, f.shared_lib);
}

#[test]
fn delete_on_second_library_row_leaves_the_first_untouched() {
    // Two ProjectLocal libraries with the same file name in different folders.
    let project_tmp = TempDir::new().expect("project tempdir");
    let first_lib = project_tmp.path().join("Power.snxlib");
    let second_lib = project_tmp.path().join("sub").join("Power.snxlib");
    fs::create_dir(project_tmp.path().join("sub")).expect("create sub dir");
    fs::write(&first_lib, b"first-library").expect("write first library");
    fs::write(&second_lib, b"second-library").expect("write second library");
    let project_path = project_tmp.path().join("Board.snxprj");
    fs::write(&project_path, b"{}").expect("write .snxprj");
    let local = |rel: &str| LibraryEntry {
        path: PathBuf::from(rel),
        kind: LibraryEntryKind::ProjectLocal,
        library_id: None,
        unknown: Default::default(),
    };
    let mut app = app_with_libraries(
        &project_path,
        project_tmp.path(),
        vec![local("Power.snxlib"), local("sub/Power.snxlib")],
    );

    tree_action(
        &mut app,
        ProjectTreeAction::OpenRemoveDialog(SECOND_ROW.to_vec()),
    );
    let _ = app.update(Message::Remove(RemoveMsg::Confirm(
        RemoveChoice::DeleteFile,
    )));

    assert_eq!(
        fs::read(&first_lib).expect("the first library must survive"),
        b"first-library"
    );
    assert!(!second_lib.exists(), "the clicked library is deleted");
    assert_eq!(library_paths(&app), vec![first_lib]);
}

#[test]
fn exclude_on_first_library_row_keeps_the_second_entry() {
    let mut f = two_libraries_one_name();

    tree_action(
        &mut f.app,
        ProjectTreeAction::OpenRemoveDialog(LOCAL_ROW.to_vec()),
    );
    let _ = f.app.update(Message::Remove(RemoveMsg::Confirm(
        RemoveChoice::ExcludeFromProject,
    )));

    assert!(f.local_lib.exists() && f.shared_lib.exists());
    assert_eq!(library_paths(&f.app), vec![f.shared_lib.clone()]);
}

#[test]
fn stale_row_is_refused_and_a_surviving_row_keeps_its_own_library() {
    let mut f = two_libraries_one_name();
    // Entry removed behind the tree's back: the cached rows are now stale.
    f.app.document_state.projects[0].data.libraries.remove(0);

    // The removed library's row no longer resolves: no destructive dialog.
    tree_action(
        &mut f.app,
        ProjectTreeAction::OpenRemoveDialog(LOCAL_ROW.to_vec()),
    );
    assert!(
        f.app.ui_state.remove_dialog.is_none(),
        "a row whose library left the project must not open a dialog"
    );

    // The surviving row is addressed by identity, so it still targets its
    // own library even though its position shifted.
    tree_action(
        &mut f.app,
        ProjectTreeAction::OpenRemoveDialog(SHARED_ROW.to_vec()),
    );
    let dialog = f.app.ui_state.remove_dialog.as_ref().expect("dialog open");
    assert_eq!(dialog.target_path, f.shared_lib);
}

fn add_pending_library(f: &mut Fixture) -> PathBuf {
    let pending_path = f._project_tmp.path().join("Pending.snxlib");
    f.app.document_state.projects[0].pending_libraries.insert(
        uuid::Uuid::new_v4(),
        signex_app::library::commands::PendingLibrarySpec {
            lib_path: pending_path.clone(),
            enable_git: false,
            use_lfs: false,
            display_name: "Pending".into(),
        },
    );
    let _ = f.app.update(Message::Ui(UiMsg::GridPickerSelect(1.0)));
    pending_path
}

#[test]
fn version_control_targets_the_clicked_row_with_a_pending_library() {
    let mut f = two_libraries_one_name();
    add_pending_library(&mut f);

    let shared_row = row_ref(&f, &f.shared_lib);
    tree_action(
        &mut f.app,
        ProjectTreeAction::OpenLibraryEnableVersionControl(shared_row),
    );

    let vc = f
        .app
        .ui_state
        .enable_version_control
        .as_ref()
        .expect("dialog open");
    assert_eq!(Some(vc.project_dir.as_path()), f.shared_lib.parent());
}

/// Version control on a library that is only pending (not on disk yet) has
/// nothing to track; trunk opens the dialog anyway.
#[test]
#[ignore = "possible remaining bug: Enable Version Control opens for a pending library with no directory on disk"]
fn version_control_refuses_a_pending_library() {
    let mut f = two_libraries_one_name();
    let pending_path = add_pending_library(&mut f);

    let pending_row = row_ref(&f, &pending_path);
    tree_action(
        &mut f.app,
        ProjectTreeAction::OpenLibraryEnableVersionControl(pending_row),
    );
    assert!(f.app.ui_state.enable_version_control.is_none());
}
