//! #859 with #1401 — a project Replace reads unopened sheets off the UI
//! thread. A rename or delete of one of them while that read is in flight
//! must not let the Replace install the old bytes at the old path, where
//! Save All would write the moved or deleted sheet back. A Replace started
//! while the rename or delete still runs is refused outright.

use signex_app::app::{
    LoadedProject, Message, ProjectCloseChoice, ProjectMsg, RemoveChoice, RemoveDialogState,
    RemoveMsg, RenameDialogState, RenameMsg, Signex, WindowMsg,
};
use signex_app::find_replace::{FindReplaceMsg, FindScope};
use signex_types::project::{ProjectData, SheetEntry};
use signex_types::schematic::{HAlign, Label, LabelType, Point, SchematicSheet, VAlign};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

struct Fixture {
    app: Signex,
    _tmp: TempDir,
    child: PathBuf,
    renamed: PathBuf,
}

fn sheet_with_label(text: &str) -> SchematicSheet {
    let mut sheet = SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
        paper_size: "A4".to_string(),
        ..Default::default()
    };
    sheet.labels.push(Label {
        unknown_columns: Default::default(),
        uuid: uuid::Uuid::now_v7(),
        text: text.to_string(),
        position: Point::new(0.0, 0.0),
        rotation: 0.0,
        label_type: LabelType::Global,
        shape: String::new(),
        font_size: 1.27,
        justify: HAlign::default(),
        justify_v: VAlign::default(),
        font_family: None,
        locked: false,
    });
    sheet
}

fn write_sheet(path: &Path, sheet: SchematicSheet) {
    let text = signex_types::format::SnxSchematic::new(sheet)
        .write_string()
        .expect("serialise sheet");
    fs::write(path, text).expect("write sheet");
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

/// A loaded project of two unopened pages on disk, `root.snxsch` and
/// `child.snxsch`; only the child carries `CHILD_NET`.
fn fixture() -> Fixture {
    let _ = signex_app::diagnostics::init_logging();
    let tmp = TempDir::new().expect("tempdir");
    let dir = tmp.path().to_path_buf();
    let child = dir.join("child.snxsch");
    write_sheet(&dir.join("root.snxsch"), sheet_with_label("ROOT_NET"));
    write_sheet(&child, sheet_with_label("CHILD_NET"));
    let data = ProjectData {
        description: None,
        name: "proj".to_string(),
        dir: dir.to_string_lossy().into_owned(),
        schematic_root: Some("root.snxsch".to_string()),
        pcb_file: None,
        sheets: vec![entry("root.snxsch"), entry("child.snxsch")],
        variant_definitions: Vec::new(),
        active_variant: None,
        libraries: Vec::new(),
        enable_git: false,
        net_classes: Vec::new(),
        rules: None,
        annotation: None,
        parameters: Default::default(),
        unknown: Default::default(),
    };
    let prj = dir.join("proj.snxprj");
    fs::write(&prj, serde_json::to_vec(&data).expect("serialise prj")).expect("write prj");
    let (mut app, _task) = Signex::new();
    let id = app.document_state.mint_project_id();
    app.document_state.projects.push(LoadedProject {
        id,
        path: prj,
        data,
        pending_libraries: HashMap::new(),
        case_fold: Default::default(),
        case_probed: false,
        case_decided: false,
    });
    app.document_state.active_project = Some(id);
    Fixture {
        app,
        _tmp: tmp,
        renamed: dir.join("moved.snxsch"),
        child,
    }
}

/// The messages `task` emits, run to completion but not fed back.
fn outputs(task: iced::Task<Message>) -> Vec<Message> {
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

fn find(app: &mut Signex, msg: FindReplaceMsg) {
    crate::settle_support::settle(app, Message::FindReplaceMsg(msg));
}

/// Search the project for the child's label, ready to Replace All.
fn search_child(app: &mut Signex) {
    find(app, FindReplaceMsg::ScopeChanged(FindScope::Project));
    find(app, FindReplaceMsg::QueryChanged("child_net".into()));
    find(app, FindReplaceMsg::ReplacementChanged("X".into()));
    assert!(
        !app.ui_state.find_replace.matches.is_empty(),
        "precondition: the closed child is found"
    );
}

/// Start Replace All and run its disk read: the child's bytes are read now,
/// the answer is returned undelivered.
fn prepare_replace(app: &mut Signex) -> Vec<Message> {
    let prepared = outputs(app.update(Message::FindReplaceMsg(FindReplaceMsg::ReplaceAll)));
    assert!(!prepared.is_empty(), "precondition: the read came back");
    prepared
}

fn start_rename(app: &mut Signex, target: &Path) -> iced::Task<Message> {
    app.ui_state.rename_dialog = Some(RenameDialogState {
        project_id: None,
        target_path: target.to_path_buf(),
        tree_path: vec![0, 1],
        buffer: "moved.snxsch".to_string(),
        error: None,
        is_project_rename: false,
    });
    app.update(Message::Rename(RenameMsg::Submit))
}

fn start_delete(app: &mut Signex, target: &Path) -> iced::Task<Message> {
    app.ui_state.remove_dialog = Some(RemoveDialogState {
        project_id: None,
        target_path: target.to_path_buf(),
        tree_path: vec![0, 1],
        display_name: "child.snxsch".to_string(),
        is_outside_project: false,
        unsaved_prompt: false,
    });
    app.update(Message::Remove(RemoveMsg::Confirm(
        RemoveChoice::DeleteFile,
    )))
}

fn deliver(app: &mut Signex, messages: Vec<Message>) {
    for message in messages {
        crate::settle_support::settle(app, message);
    }
}

/// Quit with Save All, the way a user leaving the app saves everything.
fn save_all(app: &mut Signex) {
    let _close = app.update(Message::Window(WindowMsg::CloseMainWindow));
    if app.ui_state.app_quit_confirm.is_none() {
        // No prompt is only right when there is nothing left to write.
        assert!(
            !app.document_state.has_dirty_documents(),
            "quit skipped Save All with dirty documents: {:?}",
            app.document_state.dirty_document_paths()
        );
        return;
    }
    crate::settle_support::settle(
        app,
        Message::Project(ProjectMsg::AppQuitConfirm(ProjectCloseChoice::SaveAll)),
    );
}

fn assert_nothing_held_at(app: &Signex, old: &Path) {
    let ds = &app.document_state;
    assert!(
        !ds.engines.contains_key(old),
        "the Replace installed an engine at the old path"
    );
    assert!(!ds.is_document_dirty(old), "the old path is dirty");
    assert!(ds.tabs.iter().all(|t| t.path != old), "a tab holds it");
}

#[test]
fn a_replace_read_before_a_rename_is_not_applied_at_the_old_path() {
    let mut f = fixture();
    search_child(&mut f.app);
    let prepared = prepare_replace(&mut f.app);

    let rename = start_rename(&mut f.app, &f.child.clone());
    crate::settle_support::settle_task(&mut f.app, rename);
    assert!(f.renamed.exists() && !f.child.exists(), "the file moved");
    deliver(&mut f.app, prepared);

    assert_nothing_held_at(&f.app, &f.child);
    save_all(&mut f.app);
    assert!(!f.child.exists(), "Save All wrote the old file back");
    assert!(f.renamed.exists());
}

#[test]
fn a_replace_read_before_a_delete_does_not_resurrect_the_sheet() {
    let mut f = fixture();
    search_child(&mut f.app);
    let prepared = prepare_replace(&mut f.app);

    let delete = start_delete(&mut f.app, &f.child.clone());
    crate::settle_support::settle_task(&mut f.app, delete);
    assert!(!f.child.exists(), "the file was deleted");
    deliver(&mut f.app, prepared);

    assert_nothing_held_at(&f.app, &f.child);
    save_all(&mut f.app);
    assert!(!f.child.exists(), "Save All resurrected the deleted sheet");
}

/// The Replace started while the move or delete runs is refused up front,
/// with a card and a Messages line; nothing is read or installed.
fn assert_replace_refused(app: &mut Signex) {
    let since = signex_app::diagnostics::newest_entry_id();
    let task = app.update(Message::FindReplaceMsg(FindReplaceMsg::ReplaceAll));
    let notice = app
        .document_state
        .error_notice
        .clone()
        .expect("a Replace Refused card");
    assert_eq!(notice.title, "Replace Refused");
    assert!(notice.detail.contains("child.snxsch"), "{}", notice.detail);
    assert!(!app.ui_state.find_replace.is_busy(), "no replace job waits");
    assert!(outputs(task).is_empty(), "nothing was read");
    assert!(
        signex_app::diagnostics::recent_entries()
            .iter()
            .any(|e| Some(e.id) > since && e.message.contains("Replace was not run")),
        "the refusal reaches the Messages panel"
    );
}

#[test]
fn a_replace_is_refused_while_a_rename_runs() {
    let mut f = fixture();
    search_child(&mut f.app);
    let rename = start_rename(&mut f.app, &f.child.clone());

    assert_replace_refused(&mut f.app);

    crate::settle_support::settle_task(&mut f.app, rename);
    assert_nothing_held_at(&f.app, &f.child);
    save_all(&mut f.app);
    assert!(!f.child.exists() && f.renamed.exists());
}

#[test]
fn a_replace_is_refused_while_a_delete_runs() {
    let mut f = fixture();
    search_child(&mut f.app);
    let delete = start_delete(&mut f.app, &f.child.clone());

    assert_replace_refused(&mut f.app);

    crate::settle_support::settle_task(&mut f.app, delete);
    assert_nothing_held_at(&f.app, &f.child);
    save_all(&mut f.app);
    assert!(!f.child.exists(), "the deleted sheet stays deleted");
}
