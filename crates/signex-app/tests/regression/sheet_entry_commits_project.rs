//! #1821 — renaming a sheet or adding a new one changes a file on disk at
//! once, so the saved `.snxprj` must name it too. Discard All, a quit without
//! saving or a crash must never leave the project pointing at a missing file
//! or leave a sheet file the project does not list, and the commit must not
//! write other unsaved project edits the user may want to discard.

use signex_app::app::{
    LoadedProject, Message, ProjectId, ProjectMsg, RenameDialogState, RenameMsg, Signex,
};
use signex_types::project::{ProjectData, SheetEntry, parse_project, write_project};
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

fn sheet_entry(filename: &str) -> SheetEntry {
    SheetEntry {
        name: filename.to_string(),
        filename: filename.to_string(),
        symbols_count: 0,
        wires_count: 0,
        labels_count: 0,
        unknown: Default::default(),
    }
}

fn project_data(dir: &Path) -> ProjectData {
    ProjectData {
        description: None,
        name: "Proj".to_string(),
        dir: dir.to_string_lossy().into_owned(),
        schematic_root: Some("Second.snxsch".to_string()),
        pcb_file: None,
        sheets: vec![sheet_entry("Second.snxsch")],
        variant_definitions: Vec::new(),
        active_variant: None,
        libraries: Vec::new(),
        enable_git: false,
        rules: Default::default(),
        net_classes: Default::default(),
        annotation: None,
        parameters: Default::default(),
        unknown: Default::default(),
    }
}

/// A saved project listing `Second.snxsch` (which exists on disk) and loaded
/// in the app, clean. `on_disk: false` leaves no `.snxprj` behind (a project
/// never saved).
fn fixture(on_disk: bool) -> Fixture {
    let _ = signex_app::diagnostics::init_logging();
    let tmp = TempDir::new().expect("tempdir");
    let dir = tmp.path().to_path_buf();
    let prj = dir.join("Proj.snxprj");
    std::fs::write(dir.join("Second.snxsch"), b"x").expect("write sheet");
    let data = project_data(&dir);
    if on_disk {
        write_project(&prj, &data).expect("write project");
    }
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

fn run(task: iced::Task<Message>) -> Vec<Message> {
    use futures::StreamExt as _;
    let Some(mut stream) = iced_runtime::task::into_stream(task) else {
        return Vec::new();
    };
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    runtime.block_on(async {
        let mut out = Vec::new();
        while let Some(action) = stream.next().await {
            if let iced_runtime::Action::Output(message) = action {
                out.push(message);
            }
        }
        out
    })
}

fn complete(app: &mut Signex, task: iced::Task<Message>) {
    for message in run(task) {
        let _follow_up = app.update(message);
    }
}

fn rename(app: &mut Signex, target: &Path, buffer: &str) {
    app.ui_state.rename_dialog = Some(RenameDialogState {
        project_id: None,
        target_path: target.to_path_buf(),
        tree_path: vec![0],
        buffer: buffer.to_string(),
        error: None,
        is_project_rename: false,
    });
    let task = app.update(Message::Rename(RenameMsg::Submit));
    complete(app, task);
}

fn add_new(f: &mut Fixture, name: &str) -> PathBuf {
    let path = f.dir.join(name);
    let task = f
        .app
        .update(Message::Project(ProjectMsg::AddNewSchematicPicked {
            project_id: f.project,
            path: Some(path.clone()),
        }));
    complete(&mut f.app, task);
    path
}

fn logged(fragment: &str) -> bool {
    signex_app::diagnostics::recent_entries()
        .iter()
        .any(|e| e.message.contains(fragment))
}

/// What Discard All then reopen shows: the project as saved on disk.
fn reopened(f: &Fixture) -> ProjectData {
    parse_project(&f.prj).expect("the saved project parses")
}

fn names(data: &ProjectData) -> Vec<String> {
    data.sheets.iter().map(|s| s.filename.clone()).collect()
}

#[test]
fn rename_then_discard_all_reopens_with_the_renamed_sheet() {
    let mut f = fixture(true);
    rename(&mut f.app, &f.dir.join("Second.snxsch"), "Third.snxsch");

    assert!(f.dir.join("Third.snxsch").exists());
    let saved = reopened(&f);
    assert_eq!(names(&saved), ["Third.snxsch"], "no (missing) sheet");
    assert_eq!(saved.schematic_root.as_deref(), Some("Third.snxsch"));
    assert!(
        !f.app.document_state.is_document_dirty(&f.prj),
        "the project was committed with the move, so nothing is left unsaved"
    );
    assert!(logged("the saved project file now lists the new name"));
}

#[test]
fn rename_does_not_write_other_unsaved_project_edits() {
    let mut f = fixture(true);
    f.app.document_state.projects[0].data.description = Some("unsaved note".to_string());
    f.app.document_state.mark_document_dirty(&f.prj);

    rename(&mut f.app, &f.dir.join("Second.snxsch"), "Third.snxsch");

    let saved = reopened(&f);
    assert_eq!(names(&saved), ["Third.snxsch"]);
    assert_eq!(
        saved.description, None,
        "the unsaved description stays unsaved"
    );
    assert_eq!(
        f.app.document_state.projects[0].data.description.as_deref(),
        Some("unsaved note"),
        "the edit is still held in memory"
    );
    assert!(
        f.app.document_state.is_document_dirty(&f.prj),
        "the project still has the unsaved edit"
    );
}

#[test]
fn rename_in_a_project_never_saved_leaves_it_dirty_and_says_so() {
    let mut f = fixture(false);
    rename(&mut f.app, &f.dir.join("Second.snxsch"), "Third.snxsch");

    assert!(!f.prj.exists(), "no project file is created by a rename");
    assert_eq!(
        names(&f.app.document_state.projects[0].data),
        ["Third.snxsch"]
    );
    assert!(f.app.document_state.is_document_dirty(&f.prj));
    assert!(logged("Save the project to keep the new name"));
}

#[test]
fn rename_is_undone_when_the_saved_project_cannot_be_updated() {
    let mut f = fixture(true);
    std::fs::write(&f.prj, b"{ broken").expect("corrupt the project");
    rename(&mut f.app, &f.dir.join("Second.snxsch"), "Third.snxsch");

    assert!(f.dir.join("Second.snxsch").exists(), "the file moved back");
    assert!(!f.dir.join("Third.snxsch").exists());
    assert_eq!(
        std::fs::read(&f.prj).unwrap(),
        b"{ broken",
        "an unreadable project is never overwritten"
    );
    assert_eq!(
        names(&f.app.document_state.projects[0].data),
        ["Second.snxsch"],
        "memory still names the old file"
    );
}

#[test]
fn add_new_schematic_then_discard_all_reopens_with_the_sheet() {
    let mut f = fixture(true);
    let path = add_new(&mut f, "Added.snxsch");

    assert!(path.exists());
    assert_eq!(names(&reopened(&f)), ["Second.snxsch", "Added.snxsch"]);
    assert!(!f.app.document_state.is_document_dirty(&f.prj));
    assert!(logged("the saved project file now lists it"));
}

#[test]
fn add_new_schematic_does_not_write_other_unsaved_project_edits() {
    let mut f = fixture(true);
    f.app.document_state.projects[0].data.description = Some("unsaved note".to_string());
    f.app.document_state.mark_document_dirty(&f.prj);
    add_new(&mut f, "Added.snxsch");

    let saved = reopened(&f);
    assert_eq!(names(&saved), ["Second.snxsch", "Added.snxsch"]);
    assert_eq!(saved.description, None);
    assert!(f.app.document_state.is_document_dirty(&f.prj));
}

#[test]
fn add_new_schematic_leaves_no_orphan_when_the_project_cannot_be_updated() {
    let mut f = fixture(true);
    std::fs::write(&f.prj, b"{ broken").expect("corrupt the project");
    let path = add_new(&mut f, "Added.snxsch");

    assert!(!path.exists(), "no sheet file is left unlisted");
    assert_eq!(
        names(&f.app.document_state.projects[0].data),
        ["Second.snxsch"]
    );
    assert!(logged(
        "the project file could not be updated, so Added.snxsch was not kept"
    ));
}

#[test]
fn a_rename_whose_move_back_also_fails_tells_the_user_where_the_file_is() {
    use signex_app::app::path_ops::RenameIoError;

    let mut f = fixture(true);
    let from = f.dir.join("Second.snxsch");
    f.app.ui_state.rename_dialog = Some(RenameDialogState {
        project_id: None,
        target_path: from.clone(),
        tree_path: vec![0],
        buffer: "Third.snxsch".to_string(),
        error: None,
        is_project_rename: false,
    });
    let task = f.app.update(Message::Rename(RenameMsg::Submit));
    let mut messages = run(task);
    if let Some(Message::Rename(RenameMsg::FileRenamed(done))) = messages.first_mut() {
        done.result = Err(RenameIoError::ProjectNotPatched {
            reason: "the project file could not be updated".to_string(),
            moved_back: Err("denied".to_string()),
        });
    }
    f.app.ui_state.rename_dialog = None;
    for message in messages {
        let _ = f.app.update(message);
    }
    assert!(
        f.app.document_state.error_notice.is_some(),
        "the stranded file raises a card"
    );
}
