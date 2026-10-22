//! #731 — a project-tree action on a library row must act on exactly the
//! library that row represents, even when two libraries share a file name.
//!
//! Rows resolve by position in the library list and are refused when the
//! cached tree no longer matches it.

use signex_app::app::{
    ContextMenuMsg, LoadedProject, Message, ProjectTreeAction, RemoveChoice, RemoveMsg, Signex,
    UiMsg,
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
        },
        pending_libraries: HashMap::new(),
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
            },
            LibraryEntry {
                path: shared_lib.clone(),
                kind: LibraryEntryKind::Shared,
                library_id: None,
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
fn stale_row_after_entry_removed_is_refused() {
    let mut f = two_libraries_one_name();
    // Entry removed behind the tree's back: the cached rows are now stale.
    f.app.document_state.projects[0].data.libraries.remove(0);

    for row in [LOCAL_ROW, SHARED_ROW] {
        tree_action(
            &mut f.app,
            ProjectTreeAction::OpenRemoveDialog(row.to_vec()),
        );
        assert!(
            f.app.ui_state.remove_dialog.is_none(),
            "stale row {row:?} must not open a destructive dialog"
        );
    }
}

#[test]
fn version_control_targets_the_clicked_row_with_a_pending_library() {
    let mut f = two_libraries_one_name();
    let pending_path = f._project_tmp.path().join("Pending.snxlib");
    f.app.document_state.projects[0].pending_libraries.insert(
        uuid::Uuid::new_v4(),
        signex_app::library::commands::PendingLibrarySpec {
            lib_path: pending_path,
            enable_git: false,
            use_lfs: false,
            display_name: "Pending".into(),
        },
    );
    let _ = f.app.update(Message::Ui(UiMsg::GridPickerSelect(1.0)));

    tree_action(
        &mut f.app,
        ProjectTreeAction::OpenLibraryEnableVersionControl(SHARED_ROW.to_vec()),
    );

    let vc = f
        .app
        .ui_state
        .enable_version_control
        .as_ref()
        .expect("dialog open");
    assert_eq!(Some(vc.project_dir.as_path()), f.shared_lib.parent());

    // The pending row is not on disk yet, so there is nothing to track.
    f.app.ui_state.enable_version_control = None;
    tree_action(
        &mut f.app,
        ProjectTreeAction::OpenLibraryEnableVersionControl(vec![0, 0, 2]),
    );
    assert!(f.app.ui_state.enable_version_control.is_none());
}
