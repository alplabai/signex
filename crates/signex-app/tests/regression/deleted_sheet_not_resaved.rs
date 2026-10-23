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
                unknown: Default::default(),
            }],
            variant_definitions: Vec::new(),
            active_variant: None,
            libraries: Vec::new(),
            enable_git: false,
            unknown: Default::default(),
        },
        pending_libraries: HashMap::new(),
    });
    id
}

/// Add `filename` to the sheet list of `projects[idx]`.
fn list_sheet(app: &mut Signex, idx: usize, filename: &str) {
    app.document_state.projects[idx]
        .data
        .sheets
        .push(SheetEntry {
            name: filename.to_string(),
            filename: filename.to_string(),
            symbols_count: 0,
            wires_count: 0,
            labels_count: 0,
            unknown: Default::default(),
        });
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

use crate::close_support::close_first_project;

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

/// Write `parent` to disk referencing `child_filename` as a hierarchical child.
fn write_sheet_with_child(parent: &Path, child_filename: &str) {
    let mut sheet = empty_sheet();
    sheet
        .child_sheets
        .push(signex_types::schematic::ChildSheet {
            uuid: uuid::Uuid::nil(),
            name: String::new(),
            filename: child_filename.to_string(),
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
    let text = signex_types::format::SnxSchematic::new(sheet)
        .write_string()
        .expect("serialise parent sheet");
    std::fs::write(parent, text).expect("write parent sheet");
}

#[test]
fn closing_a_project_keeps_a_sibling_child_whose_parent_sheet_is_unloaded() {
    let tmp = TempDir::new().expect("tempdir");
    let (mut app, _task) = Signex::new();
    add_project(&mut app, &tmp.path().join("A.snxprj"), "a.snxsch");
    let b = add_project(&mut app, &tmp.path().join("B.snxprj"), "b.snxsch");
    let b_root = tmp.path().join("b.snxsch");
    let child = tmp.path().join("b_child.snxsch");
    write_sheet_with_child(&b_root, "b_child.snxsch");
    // B's root tab was closed clean: only the dirty child's engine remains.
    open_dirty_sheet(&mut app, &child, b);

    close_first_project(&mut app);

    assert!(
        app.ui_state.project_close_confirm.is_none(),
        "b_child belongs to B, so closing A must not prompt for it",
    );
    assert!(app.document_state.dirty_paths.contains(&child));
    assert!(app.document_state.engines.contains_key(&child));
}

#[test]
fn discard_all_keeps_a_sibling_child_whose_parent_sheet_is_unloaded() {
    let tmp = TempDir::new().expect("tempdir");
    let (mut app, _task) = Signex::new();
    let a = add_project(&mut app, &tmp.path().join("A.snxprj"), "a.snxsch");
    let b = add_project(&mut app, &tmp.path().join("B.snxprj"), "b.snxsch");
    let a_sheet = tmp.path().join("a.snxsch");
    let child = tmp.path().join("b_child.snxsch");
    write_sheet_with_child(&tmp.path().join("b.snxsch"), "b_child.snxsch");
    open_dirty_sheet(&mut app, &a_sheet, a);
    open_dirty_sheet(&mut app, &child, b);

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

    assert!(app.document_state.dirty_paths.contains(&child));
    assert!(app.document_state.engines.contains_key(&child));
    assert!(app.document_state.tabs.iter().any(|t| t.path == child));
}

#[test]
fn an_unreadable_sibling_sheet_is_logged_to_the_messages_panel() {
    let _ = signex_app::diagnostics::init_logging();
    let tmp = TempDir::new().expect("tempdir");
    let (mut app, _task) = Signex::new();
    let a = add_project(&mut app, &tmp.path().join("A.snxprj"), "a.snxsch");
    add_project(&mut app, &tmp.path().join("B.snxprj"), "b.snxsch");
    std::fs::write(tmp.path().join("b.snxsch"), b"not a schematic").expect("write corrupt sheet");
    open_dirty_sheet(&mut app, &tmp.path().join("a.snxsch"), a);

    close_first_project(&mut app);

    assert!(
        app.document_state.panel_ctx.diagnostics.iter().any(|e| {
            e.message.contains("b.snxsch") && e.message.contains("Discard All is disabled")
        }),
        "an unparsable sibling sheet must reach the Messages panel and say why",
    );
}

#[test]
fn closing_a_project_collects_a_sheet_it_lists_outside_its_directory() {
    let tmp = TempDir::new().expect("tempdir");
    let a_dir = tmp.path().join("A");
    let b_dir = tmp.path().join("B");
    std::fs::create_dir_all(&a_dir).expect("mkdir A");
    std::fs::create_dir_all(&b_dir).expect("mkdir B");
    let orphan = a_dir.join("x.snxsch");
    let (mut app, _task) = Signex::new();
    add_project(&mut app, &a_dir.join("A.snxprj"), "a.snxsch");
    // B lists A's sheet by absolute path, so it sits outside B's directory.
    let b = add_project(
        &mut app,
        &b_dir.join("B.snxprj"),
        orphan.to_str().expect("utf8 path"),
    );
    open_dirty_sheet(&mut app, &orphan, b);

    close_first_project(&mut app);
    assert!(
        app.ui_state.project_close_confirm.is_none(),
        "x is B's, so closing A must not prompt for it",
    );
    assert!(app.document_state.dirty_paths.contains(&orphan));

    close_first_project(&mut app);
    let modal = app
        .ui_state
        .project_close_confirm
        .as_ref()
        .expect("closing B must prompt for the sheet it owns");
    assert_eq!(modal.dirty_paths, vec![orphan]);
}

#[test]
fn closing_a_project_keeps_a_child_shared_by_two_projects_outside_both_directories() {
    let tmp = TempDir::new().expect("tempdir");
    let (a_dir, b_dir, shared_dir) = (
        tmp.path().join("A"),
        tmp.path().join("B"),
        tmp.path().join("shared"),
    );
    for dir in [&a_dir, &b_dir, &shared_dir] {
        std::fs::create_dir_all(dir).expect("mkdir");
    }
    let shared = shared_dir.join("s.snxsch");
    let shared_ref = shared.to_str().expect("utf8 path");
    write_sheet_with_child(&a_dir.join("a.snxsch"), shared_ref);
    write_sheet_with_child(&b_dir.join("b.snxsch"), shared_ref);
    let (mut app, _task) = Signex::new();
    let a = add_project(&mut app, &a_dir.join("A.snxprj"), "a.snxsch");
    let b = add_project(&mut app, &b_dir.join("B.snxprj"), "b.snxsch");
    open_dirty_sheet(&mut app, &a_dir.join("a.snxsch"), a);
    // Open and dirty under B; A reaches it too, but B does as well.
    open_dirty_sheet(&mut app, &shared, b);

    close_first_project(&mut app);
    let modal = app
        .ui_state
        .project_close_confirm
        .as_ref()
        .expect("A is dirty, so close must prompt");
    assert_eq!(
        modal.dirty_paths,
        vec![a_dir.join("a.snxsch")],
        "a sheet B also reaches must not be offered for A's Discard All",
    );
    let _ = app.update(Message::Project(ProjectMsg::CloseConfirm(
        ProjectCloseChoice::DiscardAll,
    )));

    assert!(app.document_state.dirty_paths.contains(&shared));
    assert!(app.document_state.engines.contains_key(&shared));
}

#[test]
fn an_unreadable_sheet_disables_discard_all_and_a_forced_discard_is_refused() {
    let _ = signex_app::diagnostics::init_logging();
    let tmp = TempDir::new().expect("tempdir");
    let (mut app, _task) = Signex::new();
    let a = add_project(&mut app, &tmp.path().join("A.snxprj"), "a.snxsch");
    let b = add_project(
        &mut app,
        &tmp.path().join("B.snxprj"),
        "failclosed_b.snxsch",
    );
    // B's root is corrupt, so which sheets it parents is unknowable.
    std::fs::write(tmp.path().join("failclosed_b.snxsch"), b"not a schematic")
        .expect("write corrupt sheet");
    let a_sheet = tmp.path().join("a.snxsch");
    let unknown = tmp.path().join("failclosed_child.snxsch");
    open_dirty_sheet(&mut app, &a_sheet, a);
    open_dirty_sheet(&mut app, &unknown, b);

    close_first_project(&mut app);
    let modal = app
        .ui_state
        .project_close_confirm
        .as_ref()
        .expect("A is dirty, so close must prompt");
    assert_eq!(modal.dirty_paths, vec![a_sheet.clone()]);
    let reason = modal
        .sheet_refs
        .discard_blocked_reason()
        .expect("the modal must not offer Discard All");
    assert!(reason.contains("failclosed_b.snxsch"), "{reason}");
    assert!(
        app.document_state.panel_ctx.diagnostics.iter().any(|e| {
            e.message.contains("failclosed_b.snxsch")
                && e.message.contains("Discard All is disabled")
        }),
        "the decision must be explained in the Messages panel",
    );

    // A forced Discard All message is refused, logged, and discards nothing.
    let _ = app.update(Message::Project(ProjectMsg::CloseConfirm(
        ProjectCloseChoice::DiscardAll,
    )));

    assert!(app.ui_state.project_close_confirm.is_some(), "modal stays");
    assert!(app.document_state.dirty_paths.contains(&a_sheet));
    assert!(app.document_state.dirty_paths.contains(&unknown));
    assert!(app.document_state.engines.contains_key(&unknown));
    assert_eq!(app.document_state.projects.len(), 2, "A is still open");
    assert!(
        app.document_state
            .panel_ctx
            .diagnostics
            .iter()
            .any(|e| e.message.contains("Discard All refused")),
        "the refusal must reach the Messages panel",
    );
}

#[test]
fn a_dotdot_reference_cycle_terminates_without_a_warning() {
    let _ = signex_app::diagnostics::init_logging();
    let tmp = TempDir::new().expect("tempdir");
    let dir = tmp.path().join("dotdot_loop_dir");
    std::fs::create_dir_all(&dir).expect("mkdir");
    // `../dotdot_loop_dir/dotdot_cycle.snxsch` from inside `dotdot_loop_dir` is the sheet itself.
    write_sheet_with_child(
        &dir.join("dotdot_cycle.snxsch"),
        "../dotdot_loop_dir/dotdot_cycle.snxsch",
    );
    let (mut app, _task) = Signex::new();
    // The root stays unloaded, so the cycle is walked from disk.
    add_project(&mut app, &dir.join("A.snxprj"), "dotdot_cycle.snxsch");
    add_project(&mut app, &tmp.path().join("B.snxprj"), "b.snxsch");

    let started = std::time::Instant::now();
    close_first_project(&mut app);
    let elapsed = started.elapsed();
    assert!(
        elapsed.as_secs() < 2,
        "the cycle must fold, took {elapsed:?}"
    );
    assert!(
        !app.document_state
            .panel_ctx
            .diagnostics
            .iter()
            .any(|e| e.message.contains("dotdot_loop_dir")),
        "a loop is not an unreadable sheet and must not warn",
    );
}

/// `tmp/A` and `tmp/B`, created empty. Returns `(a_dir, b_dir)`.
fn two_project_dirs(tmp: &TempDir) -> (PathBuf, PathBuf) {
    let (a_dir, b_dir) = (tmp.path().join("A"), tmp.path().join("B"));
    std::fs::create_dir_all(&a_dir).expect("mkdir A");
    std::fs::create_dir_all(&b_dir).expect("mkdir B");
    (a_dir, b_dir)
}

/// B's root reaches A's `s.snxsch` through `reference`, and `s` parents
/// `g.snxsch` in A's directory. `s` stays unloaded; `g` is dirty under A.
/// Returns `(app, a_sheet, g)` with A first in the project list.
fn escape_fixture(
    tmp: &TempDir,
    reference: &dyn Fn(&Path) -> String,
) -> (Signex, PathBuf, PathBuf) {
    let (a_dir, b_dir) = two_project_dirs(tmp);
    let s = a_dir.join("s.snxsch");
    write_sheet_with_child(&s, "g.snxsch");
    write_sheet_with_child(&b_dir.join("b.snxsch"), &reference(&s));
    let (mut app, _task) = Signex::new();
    let a = add_project(&mut app, &a_dir.join("A.snxprj"), "a.snxsch");
    add_project(&mut app, &b_dir.join("B.snxprj"), "b.snxsch");
    let a_sheet = a_dir.join("a.snxsch");
    let g = a_dir.join("g.snxsch");
    open_dirty_sheet(&mut app, &a_sheet, a);
    open_dirty_sheet(&mut app, &g, a);
    (app, a_sheet, g)
}

fn assert_discard_all_keeps_the_kept_sheet(mut app: Signex, a_sheet: &Path, g: &Path) {
    close_first_project(&mut app);
    let modal = app
        .ui_state
        .project_close_confirm
        .as_ref()
        .expect("A is dirty, so close must prompt");
    assert_eq!(modal.dirty_paths, vec![a_sheet.to_path_buf()]);
    assert_eq!(modal.kept_paths, vec![g.to_path_buf()]);
    let _ = app.update(Message::Project(ProjectMsg::CloseConfirm(
        ProjectCloseChoice::DiscardAll,
    )));

    assert!(app.document_state.dirty_paths.contains(g));
    assert!(app.document_state.engines.contains_key(g));
    assert!(
        app.document_state.tabs.iter().any(|t| t.path == g),
        "the kept sheet's tab must stay open, never orphaned",
    );
    assert!(!app.document_state.dirty_paths.contains(a_sheet));
    assert_eq!(app.document_state.projects.len(), 1, "A itself is closed");
}

#[test]
fn discard_all_keeps_a_sheet_reached_through_a_siblings_dotdot_escape() {
    let tmp = TempDir::new().expect("tempdir");
    let (app, a_sheet, g) = escape_fixture(&tmp, &|_| "../A/s.snxsch".to_string());
    assert_discard_all_keeps_the_kept_sheet(app, &a_sheet, &g);
}

#[test]
fn discard_all_keeps_a_sheet_reached_through_a_siblings_absolute_reference() {
    let tmp = TempDir::new().expect("tempdir");
    let (app, a_sheet, g) = escape_fixture(&tmp, &|s| s.to_str().expect("utf8 path").to_string());
    assert_discard_all_keeps_the_kept_sheet(app, &a_sheet, &g);
}

#[test]
fn every_dirty_document_of_the_closing_project_is_saved_or_kept_and_named() {
    let _ = signex_app::diagnostics::init_logging();
    let tmp = TempDir::new().expect("tempdir");
    let (a_dir, b_dir) = two_project_dirs(&tmp);
    let shared_dir = tmp.path().join("shared");
    std::fs::create_dir_all(&shared_dir).expect("mkdir shared");
    // A's root is clean and unloaded; it reaches `x` past a `../` escape.
    write_sheet_with_child(&a_dir.join("a.snxsch"), "../shared/x.snxsch");
    write_sheet_with_child(&a_dir.join("s.snxsch"), "g.snxsch");
    write_sheet_with_child(&b_dir.join("b.snxsch"), "../A/s.snxsch");
    let (mut app, _task) = Signex::new();
    let a = add_project(&mut app, &a_dir.join("A.snxprj"), "a.snxsch");
    let b = add_project(&mut app, &b_dir.join("B.snxprj"), "b.snxsch");
    let x = shared_dir.join("x.snxsch");
    let g = a_dir.join("g.snxsch");
    let draft = a_dir.join("lib.snxlib").join("symbols").join("d.snxsym");
    let b_sheet = b_dir.join("b2.snxsch");
    open_dirty_sheet(&mut app, &x, a);
    open_dirty_sheet(&mut app, &g, a);
    open_dirty_sheet(&mut app, &b_sheet, b);
    app.document_state.dirty_paths.insert(draft.clone());

    close_first_project(&mut app);

    let modal = app
        .ui_state
        .project_close_confirm
        .as_ref()
        .expect("A has unsaved work, so close must prompt");
    // `x` is reached by A alone. `g` is reached by B, and the library draft is
    // reached by nobody: where a file sits is not evidence, so it is kept.
    let mut expected_kept = vec![g.clone(), draft.clone()];
    expected_kept.sort();
    assert_eq!(modal.dirty_paths, vec![x.clone()]);
    assert_eq!(modal.kept_paths, expected_kept);
    // A's dirty documents are exactly mine + kept, with no overlap and none
    // dropped; B's own sheet is neither.
    for path in [&x, &g, &draft] {
        let in_mine = modal.dirty_paths.contains(path);
        let in_kept = modal.kept_paths.contains(path);
        assert!(in_mine ^ in_kept, "{path:?} must be in exactly one list");
    }
    assert!(!modal.dirty_paths.contains(&b_sheet) && !modal.kept_paths.contains(&b_sheet));
    assert!(
        app.document_state
            .panel_ctx
            .diagnostics
            .iter()
            .any(|e| e.message.contains("g.snxsch") && e.message.contains("kept")),
        "the Messages panel must name A's kept file",
    );
}

#[test]
fn a_library_draft_and_a_tab_only_file_under_the_closing_projects_folder_are_kept() {
    let _ = signex_app::diagnostics::init_logging();
    let tmp = TempDir::new().expect("tempdir");
    let (a_dir, b_dir) = two_project_dirs(&tmp);
    let (mut app, _task) = Signex::new();
    let a = add_project(&mut app, &a_dir.join("A.snxprj"), "a.snxsch");
    add_project(&mut app, &b_dir.join("B.snxprj"), "b.snxsch");
    // Even a `.snxlib` mounted by A is not evidence: only the walk is.
    app.document_state.projects[0]
        .data
        .libraries
        .push(signex_types::project::LibraryEntry {
            path: PathBuf::from("lib.snxlib"),
            kind: signex_types::project::LibraryEntryKind::ProjectLocal,
            library_id: None,
            unknown: Default::default(),
        });
    let a_sheet = a_dir.join("a.snxsch");
    let tab_only = a_dir.join("stray.snxsch");
    let draft = a_dir.join("lib.snxlib").join("symbols").join("d.snxsym");
    open_dirty_sheet(&mut app, &a_sheet, a);
    open_dirty_sheet(&mut app, &tab_only, a);
    app.document_state.dirty_paths.insert(draft.clone());

    close_first_project(&mut app);

    let modal = app
        .ui_state
        .project_close_confirm
        .as_ref()
        .expect("A is dirty, so close must prompt");
    assert_eq!(modal.dirty_paths, vec![a_sheet.clone()]);
    let mut expected_kept = vec![tab_only.clone(), draft.clone()];
    expected_kept.sort();
    assert_eq!(modal.kept_paths, expected_kept);
    assert!(
        app.document_state
            .panel_ctx
            .diagnostics
            .iter()
            .any(|e| e.message.contains("stray.snxsch") && e.message.contains("kept")),
        "the Messages panel must name the kept files",
    );

    let _ = app.update(Message::Project(ProjectMsg::CloseConfirm(
        ProjectCloseChoice::DiscardAll,
    )));

    assert!(!app.document_state.dirty_paths.contains(&a_sheet));
    assert!(app.document_state.dirty_paths.contains(&tab_only));
    assert!(app.document_state.dirty_paths.contains(&draft));
    assert!(
        app.document_state.tabs.iter().any(|t| t.path == tab_only),
        "a kept file's tab stays open",
    );
}

#[test]
fn save_all_under_an_unreadable_sheet_saves_only_what_the_closing_project_reaches() {
    let _ = signex_app::diagnostics::init_logging();
    let tmp = TempDir::new().expect("tempdir");
    let (a_dir, b_dir) = two_project_dirs(&tmp);
    // B lists `../b.snxsch`: above A's directory and corrupt on disk. `g` sits
    // in A's folder but nothing A reaches leads to it.
    std::fs::write(tmp.path().join("b.snxsch"), b"not a schematic").expect("write corrupt sheet");
    let (mut app, _task) = Signex::new();
    let a = add_project(&mut app, &a_dir.join("A.snxprj"), "a.snxsch");
    add_project(&mut app, &b_dir.join("B.snxprj"), "../b.snxsch");
    let a_sheet = a_dir.join("a.snxsch");
    let g = a_dir.join("g.snxsch");
    open_dirty_sheet(&mut app, &a_sheet, a);
    app.document_state.dirty_paths.insert(g.clone());

    close_first_project(&mut app);

    let modal = app
        .ui_state
        .project_close_confirm
        .as_ref()
        .expect("A is dirty, so close must prompt");
    assert_eq!(modal.dirty_paths, vec![a_sheet.clone()]);
    assert_eq!(modal.kept_paths, vec![g.clone()]);
    let _ = app.update(Message::Project(ProjectMsg::CloseConfirm(
        ProjectCloseChoice::SaveAll,
    )));
    assert!(a_sheet.exists(), "Save All writes what A reaches");
    assert!(!app.document_state.dirty_paths.contains(&a_sheet));
    assert!(app.document_state.dirty_paths.contains(&g), "g is kept");
    assert_eq!(app.document_state.projects.len(), 1, "A is closed");
}

#[test]
fn an_engine_opened_through_a_raw_dotdot_path_still_marks_its_children_as_the_siblings() {
    let tmp = TempDir::new().expect("tempdir");
    let (a_dir, b_dir) = two_project_dirs(&tmp);
    let (mut app, _task) = Signex::new();
    // A lists `a` and `../c.snxsch`. B lists `../b.snxsch`, whose engine was
    // opened under the RAW path `B/../b.snxsch` and adds `c` as a child.
    let a = add_project(&mut app, &a_dir.join("A.snxprj"), "a.snxsch");
    list_sheet(&mut app, 0, "../c.snxsch");
    add_project(&mut app, &b_dir.join("B.snxprj"), "../b.snxsch");
    let raw_b = b_dir.join("..").join("b.snxsch");
    let mut sheet = empty_sheet();
    sheet
        .child_sheets
        .push(signex_types::schematic::ChildSheet {
            uuid: uuid::Uuid::nil(),
            name: String::new(),
            filename: "c.snxsch".to_string(),
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
    let mut engine = signex_engine::Engine::new(sheet).expect("engine");
    engine.set_path(Some(raw_b.clone()));
    app.document_state.engines.insert(raw_b, engine);
    let c = tmp.path().join("c.snxsch");
    let a_sheet = a_dir.join("a.snxsch");
    open_dirty_sheet(&mut app, &c, a);
    open_dirty_sheet(&mut app, &a_sheet, a);
    // c is reached by A AND by B (through B's raw-path engine).

    close_first_project(&mut app);

    let modal = app
        .ui_state
        .project_close_confirm
        .as_ref()
        .expect("A is dirty, so close must prompt");
    assert_eq!(modal.kept_paths, vec![c.clone()], "B reaches c: kept");
    assert!(!modal.dirty_paths.contains(&c));
    let _ = app.update(Message::Project(ProjectMsg::CloseConfirm(
        ProjectCloseChoice::DiscardAll,
    )));
    assert!(app.document_state.dirty_paths.contains(&c));
    assert!(app.document_state.engines.contains_key(&c));
}

#[test]
fn confirm_closes_the_project_by_id_after_the_list_shifted() {
    let tmp = TempDir::new().expect("tempdir");
    let (mut app, _task) = Signex::new();
    let a = add_project(&mut app, &tmp.path().join("A.snxprj"), "a.snxsch");
    let b = add_project(&mut app, &tmp.path().join("B.snxprj"), "b.snxsch");
    open_dirty_sheet(&mut app, &tmp.path().join("a.snxsch"), a);
    close_first_project(&mut app);
    assert!(app.ui_state.project_close_confirm.is_some());
    app.document_state.projects.reverse();

    let _ = app.update(Message::Project(ProjectMsg::CloseConfirm(
        ProjectCloseChoice::DiscardAll,
    )));

    let left: Vec<ProjectId> = app.document_state.projects.iter().map(|p| p.id).collect();
    assert_eq!(left, vec![b], "A closes by id, not by its old index");
}

#[test]
fn confirm_aborts_with_a_log_line_when_the_project_is_gone() {
    let _ = signex_app::diagnostics::init_logging();
    let tmp = TempDir::new().expect("tempdir");
    let (mut app, _task) = Signex::new();
    let a = add_project(&mut app, &tmp.path().join("A.snxprj"), "a.snxsch");
    add_project(&mut app, &tmp.path().join("B.snxprj"), "b.snxsch");
    let a_sheet = tmp.path().join("a.snxsch");
    open_dirty_sheet(&mut app, &a_sheet, a);
    close_first_project(&mut app);
    app.document_state.projects.retain(|p| p.id != a);

    let _ = app.update(Message::Project(ProjectMsg::CloseConfirm(
        ProjectCloseChoice::DiscardAll,
    )));

    assert!(
        app.document_state.dirty_paths.contains(&a_sheet),
        "nothing discarded",
    );
    assert!(
        app.document_state
            .panel_ctx
            .diagnostics
            .iter()
            .any(|e| e.message.contains("no longer open")),
        "the abort must reach the Messages panel",
    );
}

#[test]
fn confirm_re_resolves_when_a_project_was_opened_since_the_refs_were_read() {
    let tmp = TempDir::new().expect("tempdir");
    let (mut app, _task) = Signex::new();
    let a = add_project(&mut app, &tmp.path().join("A.snxprj"), "a.snxsch");
    let a_sheet = tmp.path().join("a.snxsch");
    open_dirty_sheet(&mut app, &a_sheet, a);
    // A lists c_child. C is opened later; its root sits unloaded on disk and
    // parents c_child, so once C is open A no longer reaches it alone.
    list_sheet(&mut app, 0, "c_child.snxsch");
    let c_child = tmp.path().join("c_child.snxsch");
    write_sheet_with_child(&tmp.path().join("c.snxsch"), "c_child.snxsch");
    open_dirty_sheet(&mut app, &c_child, a);
    close_first_project(&mut app);
    let first = app.ui_state.project_close_confirm.as_ref().expect("prompt");
    assert!(first.dirty_paths.contains(&c_child), "C is not open yet");
    add_project(&mut app, &tmp.path().join("C.snxprj"), "c.snxsch");

    let task = app.update(Message::Project(ProjectMsg::CloseConfirm(
        ProjectCloseChoice::DiscardAll,
    )));

    assert_eq!(task.units(), 1, "stale refs must trigger a fresh resolve");
    assert!(
        app.document_state.dirty_paths.contains(&a_sheet),
        "nothing discarded yet",
    );
    assert_eq!(app.document_state.projects.len(), 2, "A is still open");
    let project_id = app.document_state.projects[0].id;
    let result = app.resolve_sheet_refs_blocking();
    let _ = app.update(Message::Project(ProjectMsg::CloseRefsResolved {
        project_id,
        result,
    }));
    let modal = app
        .ui_state
        .project_close_confirm
        .as_ref()
        .expect("re-prompt");
    assert_eq!(modal.dirty_paths, vec![a_sheet]);
    assert_eq!(modal.kept_paths, vec![c_child], "C's child is kept now");
}

#[test]
fn a_project_opened_while_the_resolve_runs_triggers_a_fresh_resolve() {
    let tmp = TempDir::new().expect("tempdir");
    let (mut app, _task) = Signex::new();
    let a = add_project(&mut app, &tmp.path().join("A.snxprj"), "a.snxsch");
    open_dirty_sheet(&mut app, &tmp.path().join("a.snxsch"), a);
    let project_id = app.document_state.projects[0].id;
    let _ = app.update(Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(
        ProjectTreeAction::CloseProject(vec![0]),
    )));
    let stale = app.resolve_sheet_refs_blocking();
    add_project(&mut app, &tmp.path().join("C.snxprj"), "c.snxsch");

    let task = app.update(Message::Project(ProjectMsg::CloseRefsResolved {
        project_id,
        result: stale,
    }));

    assert_eq!(
        task.units(),
        1,
        "the gate must not run on refs that predate C"
    );
    assert!(app.ui_state.project_close_confirm.is_none());
}

/// An engine at `path` whose sheet references `child` (unsaved, in memory).
fn engine_referencing(path: &Path, child: &str) -> signex_engine::Engine {
    let mut sheet = empty_sheet();
    sheet
        .child_sheets
        .push(signex_types::schematic::ChildSheet {
            uuid: uuid::Uuid::nil(),
            name: String::new(),
            filename: child.to_string(),
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
    let mut engine = signex_engine::Engine::new(sheet).expect("engine");
    engine.set_path(Some(path.to_path_buf()));
    engine
}

/// A lists `listed` (dirty in A's folder); B's root reaches the same file
/// through `reference`. Returns `(app, a_sheet, listed_path)`.
fn identity_fixture(
    tmp: &TempDir,
    listed: &str,
    reference: &dyn Fn(&Path) -> String,
) -> (Signex, PathBuf, PathBuf) {
    let (a_dir, b_dir) = two_project_dirs(tmp);
    let shared = a_dir.join(listed);
    write_sheet_with_child(&shared, "nothing.snxsch");
    write_sheet_with_child(&b_dir.join("b.snxsch"), &reference(&shared));
    let (mut app, _task) = Signex::new();
    let a = add_project(&mut app, &a_dir.join("A.snxprj"), "a.snxsch");
    list_sheet(&mut app, 0, listed);
    add_project(&mut app, &b_dir.join("B.snxprj"), "b.snxsch");
    let a_sheet = a_dir.join("a.snxsch");
    open_dirty_sheet(&mut app, &a_sheet, a);
    open_dirty_sheet(&mut app, &shared, a);
    (app, a_sheet, shared)
}

#[test]
fn a_case_variant_reference_to_the_same_file_keeps_it_for_the_sibling() {
    let tmp = TempDir::new().expect("tempdir");
    // Only meaningful where `s.snxsch` and `S.snxsch` are one file.
    std::fs::write(tmp.path().join("Probe.tmp"), b"x").expect("probe");
    if !tmp.path().join("probe.tmp").exists() {
        eprintln!("case-sensitive volume: the case-variant file identity cannot occur here");
        return;
    }
    let (app, a_sheet, shared) =
        identity_fixture(&tmp, "S.snxsch", &|_| "../A/s.snxsch".to_string());
    assert_discard_all_keeps_the_kept_sheet(app, &a_sheet, &shared);
}

#[cfg(unix)]
#[test]
fn a_reference_through_a_symlinked_directory_keeps_the_file_for_the_sibling() {
    let tmp = TempDir::new().expect("tempdir");
    let (a_dir, _) = two_project_dirs(&tmp);
    std::os::unix::fs::symlink(&a_dir, tmp.path().join("Alink")).expect("symlink");
    let (app, a_sheet, shared) =
        identity_fixture(&tmp, "s.snxsch", &|_| "../Alink/s.snxsch".to_string());
    assert_discard_all_keeps_the_kept_sheet(app, &a_sheet, &shared);
}

/// A lists `../c.snxsch` (dirty); B's saved `b.snxsch` lists it as a child.
/// Returns `(app, a_sheet, c, b_path)`.
fn saved_reference_fixture(tmp: &TempDir) -> (Signex, PathBuf, PathBuf, PathBuf) {
    let (a_dir, b_dir) = two_project_dirs(tmp);
    let c = tmp.path().join("c.snxsch");
    let b_path = b_dir.join("b.snxsch");
    write_sheet_with_child(&b_path, "../c.snxsch");
    let (mut app, _task) = Signex::new();
    let a = add_project(&mut app, &a_dir.join("A.snxprj"), "a.snxsch");
    list_sheet(&mut app, 0, "../c.snxsch");
    add_project(&mut app, &b_dir.join("B.snxprj"), "b.snxsch");
    let a_sheet = a_dir.join("a.snxsch");
    open_dirty_sheet(&mut app, &a_sheet, a);
    open_dirty_sheet(&mut app, &c, a);
    (app, a_sheet, c, b_path)
}

#[test]
fn an_open_unsaved_sheet_that_dropped_a_reference_still_reaches_it_through_its_saved_file() {
    let tmp = TempDir::new().expect("tempdir");
    let (mut app, a_sheet, c, b_path) = saved_reference_fixture(&tmp);
    // B's open copy dropped the reference; its saved file still lists `c`.
    let mut engine = signex_engine::Engine::new(empty_sheet()).expect("engine");
    engine.set_path(Some(b_path.clone()));
    app.document_state.engines.insert(b_path, engine);

    assert_discard_all_keeps_the_kept_sheet(app, &a_sheet, &c);
}

#[test]
fn an_engine_edit_made_while_the_resolve_ran_is_seen_at_completion() {
    let tmp = TempDir::new().expect("tempdir");
    let (a_dir, b_dir) = two_project_dirs(&tmp);
    let c = tmp.path().join("c.snxsch");
    let b_path = b_dir.join("b.snxsch");
    let (mut app, _task) = Signex::new();
    let a = add_project(&mut app, &a_dir.join("A.snxprj"), "a.snxsch");
    list_sheet(&mut app, 0, "../c.snxsch");
    add_project(&mut app, &b_dir.join("B.snxprj"), "b.snxsch");
    let a_sheet = a_dir.join("a.snxsch");
    open_dirty_sheet(&mut app, &a_sheet, a);
    open_dirty_sheet(&mut app, &c, a);
    let mut quiet = signex_engine::Engine::new(empty_sheet()).expect("engine");
    quiet.set_path(Some(b_path.clone()));
    app.document_state.engines.insert(b_path.clone(), quiet);
    let _ = app.update(Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(
        ProjectTreeAction::CloseProject(vec![0]),
    )));
    let result = app.resolve_sheet_refs_blocking();
    // The gap: B's open sheet now references `c`.
    let edited = engine_referencing(&b_path, "../c.snxsch");
    app.document_state.engines.insert(b_path, edited);

    let project_id = app.document_state.projects[0].id;
    let _ = app.update(Message::Project(ProjectMsg::CloseRefsResolved {
        project_id,
        result,
    }));

    let modal = app.ui_state.project_close_confirm.as_ref().expect("prompt");
    assert_eq!(modal.dirty_paths, vec![a_sheet]);
    assert_eq!(modal.kept_paths, vec![c], "the gap edit makes `c` B's too");
}

#[test]
fn an_engine_edit_made_while_the_modal_is_up_re_prompts_instead_of_discarding() {
    let tmp = TempDir::new().expect("tempdir");
    let (mut app, a_sheet, c, b_path) = saved_reference_fixture(&tmp);
    // Nothing on disk references `c` this time: only the edit below will.
    write_sheet_with_child(&b_path, "unrelated.snxsch");
    close_first_project(&mut app);
    let first = app.ui_state.project_close_confirm.as_ref().expect("prompt");
    assert!(first.dirty_paths.contains(&c), "c is A's alone so far");

    let edited = engine_referencing(&b_path, "../c.snxsch");
    app.document_state.engines.insert(b_path, edited);
    let _ = app.update(Message::Project(ProjectMsg::CloseConfirm(
        ProjectCloseChoice::DiscardAll,
    )));

    assert!(
        app.document_state.dirty_paths.contains(&c),
        "c is not discarded"
    );
    assert!(app.document_state.engines.contains_key(&c));
    assert_eq!(app.document_state.projects.len(), 2, "A is still open");
    let modal = app
        .ui_state
        .project_close_confirm
        .as_ref()
        .expect("re-prompt");
    assert_eq!(modal.dirty_paths, vec![a_sheet]);
    assert_eq!(modal.kept_paths, vec![c]);
}
