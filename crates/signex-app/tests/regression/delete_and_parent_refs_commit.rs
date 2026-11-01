//! #1849 — deleting a project file and renaming a sheet change files on disk
//! at once, so everything that names them on disk has to change with them:
//! the saved `.snxprj` entry (Delete) and the sheet-symbol filename in every
//! parent sheet (Rename). Discard All, a quit without saving or a crash must
//! never leave a `(missing)` row or a parent that points at the old filename,
//! and the commit must not write other unsaved edits the user may discard.
//!
//! Every assertion reads the files back from disk, as a reopen would.

use signex_app::app::{
    LoadedProject, Message, ProjectId, RemoveChoice, RemoveDialogState, RemoveMsg,
    RenameDialogState, RenameMsg, Signex,
};
use signex_types::format::SnxSchematic;
use signex_types::project::{ProjectData, SheetEntry, parse_project, write_project};
use signex_types::schematic::{ChildSheet, FillType, Point, SchematicSheet};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

struct Fixture {
    app: Signex,
    _tmp: TempDir,
    dir: PathBuf,
    prj: PathBuf,
    project: ProjectId,
}

fn sheet(paper: &str, children: &[&str]) -> SchematicSheet {
    SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
        paper_size: paper.to_string(),
        child_sheets: children
            .iter()
            .map(|f| ChildSheet {
                uuid: uuid::Uuid::now_v7(),
                name: (*f).to_string(),
                filename: (*f).to_string(),
                position: Point::new(0.0, 0.0),
                size: (10.0, 10.0),
                stroke_width: 0.0,
                fill: FillType::None,
                stroke_color: None,
                fill_color: None,
                fields_autoplaced: false,
                pins: Vec::new(),
                instances: Vec::new(),
            })
            .collect(),
        ..Default::default()
    }
}

fn write_sheet(path: &Path, s: SchematicSheet) {
    let text = SnxSchematic::new(s).write_string().expect("serialise");
    std::fs::write(path, text).expect("write sheet");
}

fn entry(filename: &str) -> SheetEntry {
    SheetEntry {
        name: filename.to_string(),
        filename: filename.to_string(),
        symbols_count: 0,
        wires_count: 0,
        labels_count: 0,
        unknown: Default::default(),
    }
}

/// `Root.snxsch` places `Child.snxsch`; `Other.snxsch` is a plain page. All
/// three are listed in the saved `Proj.snxprj`, which is loaded and clean.
fn fixture() -> Fixture {
    let _ = signex_app::diagnostics::init_logging();
    let tmp = TempDir::new().expect("tempdir");
    let dir = tmp.path().to_path_buf();
    let prj = dir.join("Proj.snxprj");
    write_sheet(&dir.join("Root.snxsch"), sheet("A4", &["Child.snxsch"]));
    write_sheet(&dir.join("Child.snxsch"), sheet("A4", &[]));
    write_sheet(&dir.join("Other.snxsch"), sheet("A4", &[]));
    let data = ProjectData {
        description: None,
        name: "Proj".to_string(),
        dir: dir.to_string_lossy().into_owned(),
        schematic_root: Some("Root.snxsch".to_string()),
        pcb_file: None,
        sheets: vec![
            entry("Root.snxsch"),
            entry("Child.snxsch"),
            entry("Other.snxsch"),
        ],
        variant_definitions: Vec::new(),
        active_variant: None,
        libraries: Vec::new(),
        enable_git: false,
        rules: Default::default(),
        net_classes: Default::default(),
        annotation: None,
        parameters: Default::default(),
        unknown: Default::default(),
    };
    write_project(&prj, &data).expect("write project");
    let (mut app, _task) = Signex::new();
    let project = app.document_state.mint_project_id();
    app.document_state.projects.push(LoadedProject {
        id: project,
        path: prj.clone(),
        data,
        pending_libraries: HashMap::new(),
        case_fold: Default::default(),
        case_probed: false,
        case_decided: false,
    });
    app.document_state.active_project = Some(project);
    Fixture {
        app,
        _tmp: tmp,
        dir,
        prj,
        project,
    }
}

fn delete(f: &mut Fixture, name: &str) {
    f.app.ui_state.remove_dialog = Some(RemoveDialogState {
        target_path: f.dir.join(name),
        tree_path: vec![0],
        project_id: Some(f.project),
        display_name: name.to_string(),
        is_outside_project: false,
        unsaved_prompt: false,
    });
    crate::settle_support::settle(
        &mut f.app,
        Message::Remove(RemoveMsg::Confirm(RemoveChoice::DeleteFile)),
    );
}

fn rename(f: &mut Fixture, name: &str, buffer: &str) {
    f.app.ui_state.rename_dialog = Some(RenameDialogState {
        project_id: None,
        target_path: f.dir.join(name),
        tree_path: vec![0],
        buffer: buffer.to_string(),
        error: None,
        is_project_rename: false,
    });
    crate::settle_support::settle(&mut f.app, Message::Rename(RenameMsg::Submit));
}

/// The project as a reopen would read it.
fn reopened(f: &Fixture) -> ProjectData {
    parse_project(&f.prj).expect("the saved project parses")
}

fn names(data: &ProjectData) -> Vec<String> {
    data.sheets.iter().map(|s| s.filename.clone()).collect()
}

/// Every page the reopened project lists must exist on disk: no `(missing)`.
fn assert_no_missing_rows(f: &Fixture) {
    for page in names(&reopened(f)) {
        assert!(f.dir.join(&page).exists(), "{page} is listed but missing");
    }
}

/// The sheet file as a reopen would read it.
fn sheet_on_disk(path: &Path) -> SchematicSheet {
    let text = std::fs::read_to_string(path).expect("read sheet");
    SnxSchematic::parse(&text).expect("parse sheet").sheet
}

fn child_files(path: &Path) -> Vec<String> {
    sheet_on_disk(path)
        .child_sheets
        .iter()
        .map(|c| c.filename.clone())
        .collect()
}

/// Open `Root.snxsch` in an engine whose in-memory sheet is `memory`.
fn open_root(f: &mut Fixture, memory: SchematicSheet) {
    let root = f.dir.join("Root.snxsch");
    let engine = signex_engine::Engine::new_with_path(memory, Some(root.clone())).expect("engine");
    f.app.document_state.engines.insert(root, engine);
}

// ---- delete ---------------------------------------------------------------

#[test]
fn delete_then_discard_all_reopens_without_a_missing_row() {
    let mut f = fixture();
    delete(&mut f, "Other.snxsch");

    assert!(!f.dir.join("Other.snxsch").exists());
    assert_eq!(
        names(&reopened(&f)),
        ["Root.snxsch", "Child.snxsch"],
        "the saved project no longer lists the deleted sheet"
    );
    assert_no_missing_rows(&f);
    assert!(
        !f.app.document_state.is_document_dirty(&f.prj),
        "the entry was committed with the delete, so nothing is left unsaved"
    );
    assert_eq!(
        names(&f.app.document_state.projects[0].data),
        ["Root.snxsch", "Child.snxsch"]
    );
}

#[test]
fn delete_does_not_write_other_unsaved_project_edits() {
    let mut f = fixture();
    f.app.document_state.projects[0].data.description = Some("unsaved note".to_string());
    f.app.document_state.mark_document_dirty(&f.prj);

    delete(&mut f, "Other.snxsch");

    let saved = reopened(&f);
    assert_eq!(names(&saved), ["Root.snxsch", "Child.snxsch"]);
    assert_eq!(saved.description, None, "the unsaved note stays unsaved");
    assert_eq!(
        f.app.document_state.projects[0].data.description.as_deref(),
        Some("unsaved note"),
        "the edit is still held in memory"
    );
    assert!(f.app.document_state.is_document_dirty(&f.prj));
}

/// The saved project cannot be written: nothing is deleted, so disk stays
/// consistent (the file and the entry both stay).
#[cfg(unix)]
#[test]
fn delete_keeps_the_file_when_the_saved_project_cannot_be_written() {
    use std::os::unix::fs::PermissionsExt;
    let mut f = fixture();
    let before = std::fs::read(&f.prj).expect("read project");
    std::fs::set_permissions(&f.prj, std::fs::Permissions::from_mode(0o444)).expect("chmod");

    delete(&mut f, "Other.snxsch");

    assert!(f.dir.join("Other.snxsch").exists(), "the file is kept");
    assert_eq!(std::fs::read(&f.prj).unwrap(), before, "project unchanged");
    assert_eq!(
        names(&f.app.document_state.projects[0].data),
        ["Root.snxsch", "Child.snxsch", "Other.snxsch"],
        "memory still lists it"
    );
    assert!(
        f.app.document_state.error_notice.is_some(),
        "the user is told"
    );
    assert!(f.app.document_state.pending_path_ops.is_empty());
}

#[test]
fn delete_with_an_unreadable_saved_project_falls_back_to_an_unsaved_removal() {
    let mut f = fixture();
    std::fs::write(&f.prj, b"{ broken").expect("damage the project");

    delete(&mut f, "Other.snxsch");

    assert!(!f.dir.join("Other.snxsch").exists());
    assert_eq!(
        std::fs::read(&f.prj).unwrap(),
        b"{ broken",
        "not overwritten"
    );
    assert_eq!(
        names(&f.app.document_state.projects[0].data),
        ["Root.snxsch", "Child.snxsch"]
    );
    assert!(
        f.app.document_state.is_document_dirty(&f.prj),
        "the removal waits for a save of the in-memory project"
    );
}

#[test]
fn deleting_an_orphan_row_drops_it_from_the_saved_project_too() {
    let mut f = fixture();
    std::fs::remove_file(f.dir.join("Other.snxsch")).expect("remove outside Signex");

    delete(&mut f, "Other.snxsch");

    assert_eq!(names(&reopened(&f)), ["Root.snxsch", "Child.snxsch"]);
    assert!(!f.app.document_state.is_document_dirty(&f.prj));
}

#[test]
fn the_project_file_is_released_when_the_delete_lands() {
    let mut f = fixture();
    delete(&mut f, "Other.snxsch");
    assert!(f.app.document_state.pending_path_ops.is_empty());
}

// ---- rename: parent references ------------------------------------------

#[test]
fn rename_child_then_discard_all_reopens_with_the_parent_on_the_new_file() {
    let mut f = fixture();

    rename(&mut f, "Child.snxsch", "Kid.snxsch");

    assert!(f.dir.join("Kid.snxsch").exists());
    assert_eq!(
        child_files(&f.dir.join("Root.snxsch")),
        ["Kid.snxsch"],
        "the saved parent names the new file: no MissingChild on reopen"
    );
    assert!(names(&reopened(&f)).contains(&"Kid.snxsch".to_string()));
    assert_no_missing_rows(&f);
    assert!(
        f.dir
            .join(&child_files(&f.dir.join("Root.snxsch"))[0])
            .exists(),
        "the reference resolves to a file"
    );
}

#[test]
fn an_open_clean_parent_follows_the_rename_without_being_left_dirty() {
    let mut f = fixture();
    open_root(&mut f, sheet("A4", &["Child.snxsch"]));

    rename(&mut f, "Child.snxsch", "Kid.snxsch");

    let root = f.dir.join("Root.snxsch");
    assert_eq!(child_files(&root), ["Kid.snxsch"]);
    let in_memory = f.app.document_state.engines[&root].document().child_sheets[0]
        .filename
        .clone();
    assert_eq!(in_memory, "Kid.snxsch");
    assert!(
        !f.app.document_state.is_document_dirty(&root),
        "memory equals disk, so there is nothing to save"
    );
}

#[test]
fn rename_does_not_write_a_parents_other_unsaved_edits() {
    let mut f = fixture();
    // The open parent differs from its file by an unrelated edit (paper size).
    open_root(&mut f, sheet("A3", &["Child.snxsch"]));
    let root = f.dir.join("Root.snxsch");
    f.app.document_state.mark_document_dirty(&root);

    rename(&mut f, "Child.snxsch", "Kid.snxsch");

    let on_disk = sheet_on_disk(&root);
    assert_eq!(on_disk.child_sheets[0].filename, "Kid.snxsch");
    assert_eq!(on_disk.paper_size, "A4", "the unsaved A3 is not written");
    let engine = &f.app.document_state.engines[&root];
    assert_eq!(engine.document().paper_size, "A3", "still held in memory");
    assert_eq!(engine.document().child_sheets[0].filename, "Kid.snxsch");
    assert!(f.app.document_state.is_document_dirty(&root));
}

#[test]
fn a_read_only_parent_refuses_the_rename_and_nothing_is_written() {
    let mut f = fixture();
    let root = f.dir.join("Root.snxsch");
    // A parent from a newer Signex cannot be re-pointed; the rename is refused
    // before anything moves (#1402), so nothing on disk changes.
    let mut newer = sheet("A4", &["Child.snxsch"]);
    newer.unknown.newer_minor_source = Some("snxsch/1.7".into());
    open_root(&mut f, newer);
    let before = std::fs::read(&root).expect("read root");

    rename(&mut f, "Child.snxsch", "Kid.snxsch");

    assert!(f.dir.join("Child.snxsch").exists(), "nothing moved");
    assert_eq!(std::fs::read(&root).unwrap(), before);
}

// ---- review follow-ups ------------------------------------------------------

/// A reference the open parent no longer holds in memory (its symbol was
/// deleted, unsaved) still names the file in the saved parent: it is
/// re-pointed on disk too, so Discard All cannot strand it.
#[test]
fn rename_re_points_a_saved_reference_the_open_parent_no_longer_holds() {
    let mut f = fixture();
    open_root(&mut f, sheet("A4", &[]));
    let root = f.dir.join("Root.snxsch");
    // The engine matches its file except for the deleted symbol, and sits at
    // its save point: only the external mark can keep it dirty.
    f.app
        .document_state
        .engines
        .get_mut(&root)
        .unwrap()
        .mark_saved();
    f.app.document_state.mark_document_clean(&root);

    rename(&mut f, "Child.snxsch", "Kid.snxsch");

    assert_eq!(child_files(&root), ["Kid.snxsch"]);
    assert!(
        f.app.document_state.engines[&root]
            .document()
            .child_sheets
            .is_empty(),
        "the open engine still holds its own state"
    );
    // Undoing back to the save point must not read clean: memory and disk differ.
    f.app.document_state.sync_history_dirty(&root);
    assert!(f.app.document_state.is_document_dirty(&root));
}

/// An open of the parent that started before the rename read the old bytes and
/// would install them as a clean engine: the parent follows in memory instead
/// and the stale open is reported.
#[test]
fn a_parent_whose_open_is_in_flight_is_not_patched_behind_it() {
    for spelling in ["Root.snxsch", "root.snxsch"] {
        in_flight_open_is_not_patched_behind(spelling);
    }
}

fn in_flight_open_is_not_patched_behind(spelling: &str) {
    let mut f = fixture();
    let root = f.dir.join("Root.snxsch");
    let before = std::fs::read(&root).expect("read root");
    // The open may be spelled differently in case on a case-insensitive volume.
    f.app
        .document_state
        .pending_opens
        .insert(f.dir.join(spelling));

    rename(&mut f, "Child.snxsch", "Kid.snxsch");

    assert_eq!(
        std::fs::read(&root).unwrap(),
        before,
        "the saved parent is not rewritten under an open that already read it"
    );
    assert!(
        f.app.document_state.error_notice.is_some(),
        "the user is told"
    );
    assert!(!f.app.document_state.engines.contains_key(&root));
}

/// The owning project closes while the rename worker runs. An engine another
/// project still holds must follow the saved parent, or a later save of it
/// would write the old name back.
#[test]
fn a_project_closed_during_the_rename_leaves_open_parents_matching_disk() {
    use crate::settle_support::settle_task;
    let mut f = fixture();
    open_root(&mut f, sheet("A4", &["Child.snxsch"]));
    let root = f.dir.join("Root.snxsch");
    f.app.ui_state.rename_dialog = Some(RenameDialogState {
        project_id: None,
        target_path: f.dir.join("Child.snxsch"),
        tree_path: vec![0],
        buffer: "Kid.snxsch".to_string(),
        error: None,
        is_project_rename: false,
    });
    let task = f.app.update(Message::Rename(RenameMsg::Submit));
    f.app.document_state.projects.clear();
    settle_task(&mut f.app, task);

    assert_eq!(child_files(&root), ["Kid.snxsch"]);
    let held = &f.app.document_state.engines[&root].document().child_sheets[0].filename;
    assert_eq!(held, "Kid.snxsch", "memory equals disk");
    assert!(!f.app.document_state.is_document_dirty(&root));
}

/// The file cannot be deleted after the project entry was removed: the entry
/// is put back, so the saved project is byte for byte what it was.
#[cfg(unix)]
#[test]
fn a_failed_delete_restores_the_saved_project_entry() {
    use std::os::unix::fs::PermissionsExt;
    let mut f = fixture();
    let sub = f.dir.join("sub");
    std::fs::create_dir(&sub).expect("mkdir");
    write_sheet(&sub.join("Deep.snxsch"), sheet("A4", &[]));
    f.app.document_state.projects[0]
        .data
        .sheets
        .push(entry("sub/Deep.snxsch"));
    write_project(&f.prj, &f.app.document_state.projects[0].data).expect("write project");
    let before = std::fs::read(&f.prj).expect("read project");
    std::fs::set_permissions(&sub, std::fs::Permissions::from_mode(0o555)).expect("chmod");
    let writable_anyway = std::fs::File::create(sub.join("probe")).is_ok();

    delete(&mut f, "sub/Deep.snxsch");

    std::fs::set_permissions(&sub, std::fs::Permissions::from_mode(0o755)).expect("chmod back");
    if writable_anyway {
        return; // running as a user the permission bits do not bind
    }
    assert!(sub.join("Deep.snxsch").exists(), "the file is kept");
    assert_eq!(std::fs::read(&f.prj).unwrap(), before, "entry restored");
    assert!(names(&f.app.document_state.projects[0].data).contains(&"sub/Deep.snxsch".to_string()));
    assert!(!f.app.document_state.is_document_dirty(&f.prj));
    assert!(f.app.document_state.error_notice.is_some());
}
