//! #670 — a deleted dirty sheet must not be written back, and Close Project
//! must only prompt for and discard the closing project's own documents.

use signex_app::app::{
    ContextMenuMsg, LoadedProject, Message, ProjectCloseChoice, ProjectId, ProjectMsg,
    ProjectTreeAction, RemoveChoice, RemoveDialogState, RemoveMsg, Signex, TabInfo, TabKind,
    WindowMsg,
};
use signex_types::project::{ProjectData, SheetEntry};
use signex_types::schematic::SchematicSheet;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

fn empty_sheet() -> SchematicSheet {
    SchematicSheet {
        uuid: uuid::Uuid::new_v4(),
        paper_size: "A4".to_string(),
        ..Default::default()
    }
}

/// Load a project whose only sheet is `sheet`, living next to `prj_path`.
fn add_project(app: &mut Signex, prj_path: &Path, sheet: &str) -> ProjectId {
    let id = app.document_state.mint_project_id();
    let dir = prj_path.parent().expect("project dir");
    app.document_state.projects.push(LoadedProject {
        id,
        path: prj_path.to_path_buf(),
        data: ProjectData {
            name: prj_path.file_stem().unwrap().to_string_lossy().into_owned(),
            dir: dir.to_string_lossy().into_owned(),
            schematic_root: Some(sheet.to_string()),
            pcb_file: None,
            sheets: vec![SheetEntry {
                name: sheet.to_string(),
                filename: sheet.to_string(),
                symbols_count: 0,
                wires_count: 0,
                labels_count: 0,
            }],
            variant_definitions: Vec::new(),
            active_variant: None,
            libraries: Vec::new(),
            enable_git: false,
        },
        pending_libraries: HashMap::new(),
    });
    id
}

/// Open a schematic tab + engine at `path` owned by `project`, flagged dirty.
fn open_dirty_sheet(app: &mut Signex, path: &Path, project: ProjectId) {
    let mut engine = signex_engine::Engine::new(empty_sheet()).expect("engine");
    engine.set_path(Some(path.to_path_buf()));
    app.document_state
        .engines
        .insert(path.to_path_buf(), engine);
    app.document_state.tabs.push(TabInfo {
        title: path.file_stem().unwrap().to_string_lossy().into_owned(),
        path: path.to_path_buf(),
        cached_document: None,
        dirty: true,
        project_id: Some(project),
        kind: TabKind::Schematic,
    });
    app.document_state.dirty_paths.insert(path.to_path_buf());
}

fn close_first_project(app: &mut Signex) {
    let _ = app.update(Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(
        ProjectTreeAction::CloseProject(vec![0]),
    )));
}

#[test]
fn deleting_a_dirty_sheet_is_not_resurrected_by_save_all_on_quit() {
    let tmp = TempDir::new().expect("tempdir");
    let sheet = tmp.path().join("X.snxsch");
    std::fs::write(&sheet, b"x").expect("write sheet");
    let (mut app, _task) = Signex::new();
    let id = add_project(&mut app, &tmp.path().join("P.snxprj"), "X.snxsch");
    open_dirty_sheet(&mut app, &sheet, id);

    app.ui_state.remove_dialog = Some(RemoveDialogState {
        target_path: sheet.clone(),
        tree_path: vec![0, 0],
        display_name: "X.snxsch".to_string(),
        is_outside_project: false,
    });
    let _ = app.update(Message::Remove(RemoveMsg::Confirm(
        RemoveChoice::DeleteFile,
    )));
    assert!(!sheet.exists(), "Delete File must remove the file");

    // A second dirty sheet keeps the quit prompt alive so Save All really runs.
    let other = tmp.path().join("Y.snxsch");
    open_dirty_sheet(&mut app, &other, id);
    let _ = app.update(Message::Window(WindowMsg::CloseMainWindow));
    let modal = app
        .ui_state
        .app_quit_confirm
        .as_ref()
        .expect("the other dirty sheet must raise the quit prompt");
    assert!(modal.dirty_paths.contains(&other));
    assert!(!modal.dirty_paths.contains(&sheet));
    let _ = app.update(Message::Project(ProjectMsg::AppQuitConfirm(
        ProjectCloseChoice::SaveAll,
    )));

    assert!(other.exists(), "Save All must still save the live sheet");
    assert!(!sheet.exists(), "Save All on quit must not recreate it");
    assert!(!app.document_state.dirty_paths.contains(&sheet));
    assert!(!app.document_state.engines.contains_key(&sheet));
}

#[test]
fn closing_a_project_keeps_a_nested_projects_dirty_sheet() {
    let tmp = TempDir::new().expect("tempdir");
    let sub = tmp.path().join("sub");
    std::fs::create_dir(&sub).expect("mkdir");
    let b_sheet = sub.join("b.snxsch");
    let (mut app, _task) = Signex::new();
    add_project(&mut app, &tmp.path().join("A.snxprj"), "a.snxsch");
    let b = add_project(&mut app, &sub.join("B.snxprj"), "b.snxsch");
    open_dirty_sheet(&mut app, &b_sheet, b);

    close_first_project(&mut app);

    assert!(
        app.ui_state.project_close_confirm.is_none(),
        "A has no dirty documents of its own, so closing it must not prompt",
    );
    assert!(app.document_state.dirty_paths.contains(&b_sheet));
    assert!(app.document_state.engines.contains_key(&b_sheet));
    assert_eq!(app.document_state.projects.len(), 1);
}

#[test]
fn discard_all_does_not_touch_a_sibling_projects_dirty_sheet() {
    let tmp = TempDir::new().expect("tempdir");
    let (mut app, _task) = Signex::new();
    let a = add_project(&mut app, &tmp.path().join("A.snxprj"), "a.snxsch");
    let b = add_project(&mut app, &tmp.path().join("B.snxprj"), "b.snxsch");
    let a_sheet: PathBuf = tmp.path().join("a.snxsch");
    let b_sheet: PathBuf = tmp.path().join("b.snxsch");
    open_dirty_sheet(&mut app, &a_sheet, a);
    open_dirty_sheet(&mut app, &b_sheet, b);

    close_first_project(&mut app);
    let modal = app
        .ui_state
        .project_close_confirm
        .as_ref()
        .expect("A is dirty, so close must prompt");
    assert_eq!(modal.dirty_paths, vec![a_sheet.clone()]);

    let _ = app.update(Message::Project(ProjectMsg::CloseConfirm(
        ProjectCloseChoice::DiscardAll,
    )));

    assert!(!app.document_state.dirty_paths.contains(&a_sheet));
    assert!(app.document_state.dirty_paths.contains(&b_sheet));
    assert!(app.document_state.engines.contains_key(&b_sheet));
    assert!(app.document_state.tabs.iter().any(|t| t.path == b_sheet));
}

#[test]
fn closing_a_clean_project_keeps_a_sibling_projects_clean_engine() {
    let tmp = TempDir::new().expect("tempdir");
    let (mut app, _task) = Signex::new();
    add_project(&mut app, &tmp.path().join("A.snxprj"), "a.snxsch");
    let b = add_project(&mut app, &tmp.path().join("B.snxprj"), "b.snxsch");
    let b_sheet = tmp.path().join("b.snxsch");
    open_dirty_sheet(&mut app, &b_sheet, b);
    app.document_state.dirty_paths.clear();

    close_first_project(&mut app);

    assert!(app.document_state.engines.contains_key(&b_sheet));
}

fn delete_file(app: &mut Signex, target: &Path) {
    app.ui_state.remove_dialog = Some(RemoveDialogState {
        target_path: target.to_path_buf(),
        tree_path: vec![0, 0],
        display_name: target.file_name().unwrap().to_string_lossy().into_owned(),
        is_outside_project: false,
    });
    let _ = app.update(Message::Remove(RemoveMsg::Confirm(
        RemoveChoice::DeleteFile,
    )));
}

#[test]
fn deleting_a_dirty_sheet_logs_the_discarded_edits_to_the_messages_panel() {
    let _ = signex_app::diagnostics::init_logging();
    let tmp = TempDir::new().expect("tempdir");
    let sheet = tmp.path().join("Logged.snxsch");
    std::fs::write(&sheet, b"x").expect("write sheet");
    let (mut app, _task) = Signex::new();
    let id = add_project(&mut app, &tmp.path().join("P.snxprj"), "Logged.snxsch");
    open_dirty_sheet(&mut app, &sheet, id);

    delete_file(&mut app, &sheet);

    assert!(
        app.document_state
            .panel_ctx
            .diagnostics
            .iter()
            .any(|e| e.message.contains("Logged.snxsch") && e.message.contains("discarded")),
        "the discard must reach the Messages panel",
    );
}

#[test]
fn deleting_a_missing_file_still_drops_its_parked_dirty_engine() {
    let tmp = TempDir::new().expect("tempdir");
    let sheet = tmp.path().join("Gone.snxsch");
    let (mut app, _task) = Signex::new();
    let id = add_project(&mut app, &tmp.path().join("P.snxprj"), "Gone.snxsch");
    open_dirty_sheet(&mut app, &sheet, id);

    delete_file(&mut app, &sheet);

    assert!(!app.document_state.dirty_paths.contains(&sheet));
    assert!(!app.document_state.engines.contains_key(&sheet));
}

#[cfg(unix)]
#[test]
fn a_failed_delete_keeps_the_dirty_sheet_and_its_state() {
    use std::os::unix::fs::PermissionsExt;
    let tmp = TempDir::new().expect("tempdir");
    let locked = tmp.path().join("locked");
    std::fs::create_dir(&locked).expect("mkdir");
    let sheet = locked.join("X.snxsch");
    std::fs::write(&sheet, b"x").expect("write sheet");
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o555)).expect("chmod");
    let restore = || std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755));
    if std::fs::write(locked.join("probe"), b"").is_ok() {
        restore().expect("restore");
        return; // permissions are not enforced (running as root)
    }
    let (mut app, _task) = Signex::new();
    let id = add_project(&mut app, &tmp.path().join("P.snxprj"), "locked/X.snxsch");
    open_dirty_sheet(&mut app, &sheet, id);

    delete_file(&mut app, &sheet);
    restore().expect("restore");

    assert!(sheet.exists(), "the delete must have failed");
    assert!(app.document_state.dirty_paths.contains(&sheet));
    assert!(app.document_state.engines.contains_key(&sheet));
}

#[test]
fn deleting_a_library_directory_drops_the_editors_and_tabs_inside_it() {
    let tmp = TempDir::new().expect("tempdir");
    let lib = tmp.path().join("Lib.snxlib");
    let fp = lib.join("footprints").join("g.snxfpt");
    std::fs::create_dir_all(fp.parent().unwrap()).expect("mkdir");
    std::fs::write(&fp, b"x").expect("write footprint");
    let (mut app, _task) = Signex::new();
    add_project(&mut app, &tmp.path().join("P.snxprj"), "a.snxsch");
    let file =
        signex_library::FootprintFile::from_footprint(signex_library::Footprint::empty("fp"));
    app.document_state.footprint_editors.insert(
        fp.clone(),
        signex_app::app::FootprintEditorState::new(fp.clone(), file),
    );
    app.document_state.tabs.push(TabInfo {
        title: "fp".into(),
        path: fp.clone(),
        cached_document: None,
        dirty: true,
        project_id: None,
        kind: TabKind::FootprintEditor(fp.clone()),
    });
    app.document_state.dirty_paths.insert(fp.clone());

    delete_file(&mut app, &lib);

    assert!(!lib.exists());
    assert!(app.document_state.footprint_editors.is_empty());
    assert!(app.document_state.dirty_paths.is_empty());
    assert!(
        app.document_state.tabs.is_empty(),
        "no tab may outlive its file"
    );
}

#[test]
fn closing_a_project_keeps_a_same_directory_projects_dirty_child_sheet() {
    let tmp = TempDir::new().expect("tempdir");
    let (mut app, _task) = Signex::new();
    add_project(&mut app, &tmp.path().join("A.snxprj"), "a.snxsch");
    let b = add_project(&mut app, &tmp.path().join("B.snxprj"), "b.snxsch");
    let b_root = tmp.path().join("b.snxsch");
    let child = tmp.path().join("b_child.snxsch");
    let mut root_sheet = empty_sheet();
    root_sheet
        .child_sheets
        .push(signex_types::schematic::ChildSheet {
            uuid: uuid::Uuid::nil(),
            name: String::new(),
            filename: "b_child.snxsch".to_string(),
            position: signex_types::schematic::Point::new(0.0, 0.0),
            size: (0.0, 0.0),
            stroke_width: 0.0,
            fill: signex_types::schematic::FillType::None,
            stroke_color: None,
            fill_color: None,
            fields_autoplaced: false,
            pins: Vec::new(),
            instances: Vec::new(),
        });
    let root = signex_engine::Engine::new(root_sheet).expect("engine");
    app.document_state.engines.insert(b_root, root);
    open_dirty_sheet(&mut app, &child, b);

    close_first_project(&mut app);

    assert!(
        app.ui_state.project_close_confirm.is_none(),
        "B's child sheet is not A's, so closing A must not prompt",
    );
    assert!(app.document_state.dirty_paths.contains(&child));
    assert!(app.document_state.engines.contains_key(&child));
}
