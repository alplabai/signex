//! #731 — a project-tree action on a library row must act on exactly the
//! library that row represents, even when two libraries share a file name.
//!
//! The row used to be resolved by `.snxlib` file name, so right-clicking the
//! Shared `Power.snxlib` targeted the first same-named entry (the project-local
//! one) and Remove pruned both entries.

use signex_app::app::{
    ContextMenuMsg, LoadedProject, Message, ProjectTreeAction, RemoveChoice, RemoveMsg, Signex,
    UiMsg,
};
use signex_types::project::{LibraryEntry, LibraryEntryKind, ProjectData};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

/// Row paths in the Libraries branch: the project has no sheets, so the
/// branch is the project's only child.
const LOCAL_ROW: [usize; 3] = [0, 0, 0];
const SHARED_ROW: [usize; 3] = [0, 0, 1];

struct Fixture {
    app: Signex,
    _project_tmp: TempDir,
    _shared_tmp: TempDir,
    local_lib: PathBuf,
    shared_lib: PathBuf,
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

    let (mut app, _boot) = Signex::new();
    let id = app.document_state.mint_project_id();
    app.document_state.projects.push(LoadedProject {
        id,
        path: project_path,
        data: ProjectData {
            name: "Board".into(),
            dir: project_tmp.path().to_string_lossy().into_owned(),
            schematic_root: None,
            pcb_file: None,
            sheets: Vec::new(),
            variant_definitions: Vec::new(),
            active_variant: None,
            libraries: vec![
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
            enable_git: false,
        },
        pending_libraries: HashMap::new(),
    });
    app.document_state.active_project = Some(id);
    // Any message that republishes the panel context builds the tree.
    let _ = app.update(Message::Ui(UiMsg::GridPickerSelect(1.0)));

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
    let mut f = two_libraries_one_name();

    tree_action(
        &mut f.app,
        ProjectTreeAction::OpenRemoveDialog(SHARED_ROW.to_vec()),
    );
    let _ = f.app.update(Message::Remove(RemoveMsg::Confirm(
        RemoveChoice::DeleteFile,
    )));

    assert_eq!(
        fs::read(&f.local_lib).expect("project-local library must survive"),
        b"local-library"
    );
    assert!(!f.shared_lib.exists(), "the clicked library is deleted");
    assert_eq!(library_paths(&f.app), vec![f.local_lib.clone()]);
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
