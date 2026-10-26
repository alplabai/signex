//! #104 — opt-in autosave of dirty schematics to a recovery sidecar.
//!
//! Off by default and silent while off; on, a tick writes a sidecar in the
//! recovery directory and never the real file; a normal save (or a close)
//! removes the sidecar, including one whose write was still running.

use std::path::{Path, PathBuf};

use futures::StreamExt;
use iced::Task;
use signex_app::app::{
    ContextMenuMsg, FileMsg, LoadedProject, Message, ProjectCloseChoice, ProjectMsg, Signex,
    TabContextAction, TabInfo, TabKind,
};
use signex_app::autosave::{AutosaveMsg, sidecar};
use signex_types::project::{ProjectData, SheetEntry};
use signex_types::schematic::SchematicSheet;

const ORIGINAL: &str = "original bytes the user saved\n";

struct Fixture {
    app: Signex,
    doc: PathBuf,
    recovery: PathBuf,
    _tmp: tempfile::TempDir,
}

fn fixture(autosave_on: bool) -> Fixture {
    let tmp = tempfile::tempdir().unwrap();
    let doc = tmp.path().join("autosave.snxsch");
    let recovery = tmp.path().join("recovery");
    std::fs::write(&doc, ORIGINAL).unwrap();
    let sheet = SchematicSheet {
        uuid: uuid::Uuid::new_v4(),
        paper_size: "A4".to_string(),
        ..Default::default()
    };
    let (mut app, _boot) = Signex::new();
    let mut engine = signex_engine::Engine::new(sheet).unwrap();
    engine.set_path(Some(doc.clone()));
    app.document_state.engines.insert(doc.clone(), engine);
    app.document_state.tabs.push(TabInfo {
        title: "autosave".to_string(),
        path: doc.clone(),
        cached_document: None,
        dirty: true,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(doc.clone());
    app.document_state.dirty_paths.insert(doc.clone());
    app.document_state.autosave.set_dir(Some(recovery.clone()));
    app.ui_state.autosave_enabled = autosave_on;
    Fixture {
        app,
        doc,
        recovery,
        _tmp: tmp,
    }
}

fn run(task: Task<Message>) -> Vec<Message> {
    let Some(mut stream) = iced_runtime::task::into_stream(task) else {
        return Vec::new();
    };
    let runtime = tokio::runtime::Runtime::new().unwrap();
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

/// Run a task and feed every completion back into `update`, to a fixpoint.
fn complete(app: &mut Signex, task: Task<Message>) {
    let mut pending = run(task);
    while let Some(message) = pending.pop() {
        pending.extend(run(app.update(message)));
    }
}

fn sidecar_of(f: &Fixture) -> PathBuf {
    sidecar::sidecar_path(&f.recovery, &f.doc)
}

fn tick(f: &mut Fixture) -> Task<Message> {
    f.app.update(Message::Autosave(AutosaveMsg::Ticked))
}

fn recovery_dir_is_empty(dir: &Path) -> bool {
    !dir.exists() || std::fs::read_dir(dir).unwrap().next().is_none()
}

#[test]
fn autosave_is_off_by_default() {
    let (app, _boot) = Signex::new();
    assert!(!app.ui_state.autosave_enabled);
}

#[test]
fn autosave_off_writes_nothing() {
    let mut f = fixture(false);
    let task = tick(&mut f);
    complete(&mut f.app, task);
    assert!(recovery_dir_is_empty(&f.recovery));
    assert_eq!(std::fs::read_to_string(&f.doc).unwrap(), ORIGINAL);
}

#[test]
fn autosave_on_writes_the_sidecar_and_never_the_real_file() {
    let mut f = fixture(true);
    let task = tick(&mut f);
    assert!(
        recovery_dir_is_empty(&f.recovery),
        "update must not write inline"
    );
    complete(&mut f.app, task);

    let engine = f.app.document_state.engines.get(&f.doc).unwrap();
    let expected = engine
        .serialize_with_net_colors(&f.app.ui_state.wire_color_overrides)
        .unwrap();
    assert_eq!(std::fs::read_to_string(sidecar_of(&f)).unwrap(), expected);
    assert_eq!(std::fs::read_to_string(&f.doc).unwrap(), ORIGINAL);
    assert!(
        f.app.document_state.dirty_paths.contains(&f.doc),
        "a recovery copy is not a save"
    );
}

#[test]
fn a_clean_document_gets_no_sidecar() {
    let mut f = fixture(true);
    f.app.document_state.dirty_paths.clear();
    let task = tick(&mut f);
    complete(&mut f.app, task);
    assert!(recovery_dir_is_empty(&f.recovery));
}

#[test]
fn normal_save_removes_the_sidecar() {
    let mut f = fixture(true);
    let task = tick(&mut f);
    complete(&mut f.app, task);
    assert!(sidecar_of(&f).exists());

    let task = f.app.update(Message::File(FileMsg::Save));
    complete(&mut f.app, task);

    assert!(!sidecar_of(&f).exists(), "save must drop the recovery copy");
    assert_ne!(std::fs::read_to_string(&f.doc).unwrap(), ORIGINAL);
}

#[test]
fn a_save_that_lands_while_the_sidecar_write_runs_leaves_no_sidecar() {
    let mut f = fixture(true);
    let write = tick(&mut f);
    let save = f.app.update(Message::File(FileMsg::Save));
    complete(&mut f.app, save);
    // The document is edited again before the write finishes, so only the
    // superseded flag (not "no longer dirty") says the copy is stale.
    f.app.document_state.dirty_paths.insert(f.doc.clone());
    // The autosave write finishes after the save.
    complete(&mut f.app, write);
    assert!(
        !sidecar_of(&f).exists(),
        "a late write must not resurrect a stale copy"
    );
}

#[test]
fn project_close_discard_all_removes_the_sidecar() {
    let mut f = fixture(true);
    let task = tick(&mut f);
    complete(&mut f.app, task);
    assert!(sidecar_of(&f).exists());

    // The real project-close Discard All throws the dirty edits away.
    // Close Project reads the sheet off disk to attribute ownership, so it
    // must parse (the fixture's marker bytes would block Discard All).
    let valid = f.app.document_state.engines[&f.doc]
        .serialize_with_net_colors(&f.app.ui_state.wire_color_overrides)
        .unwrap();
    std::fs::write(&f.doc, valid).unwrap();
    let prj = f.doc.with_file_name("P.snxprj");
    let id = f.app.document_state.mint_project_id();
    f.app.document_state.projects.push(LoadedProject {
        id,
        path: prj,
        data: ProjectData {
            name: "P".to_string(),
            dir: f.doc.parent().unwrap().to_string_lossy().into_owned(),
            schematic_root: Some("autosave.snxsch".to_string()),
            pcb_file: None,
            sheets: vec![SheetEntry {
                name: "autosave.snxsch".to_string(),
                filename: "autosave.snxsch".to_string(),
                symbols_count: 0,
                wires_count: 0,
                labels_count: 0,
                unknown: Default::default(),
            }],
            variant_definitions: Vec::new(),
            active_variant: None,
            libraries: Vec::new(),
            enable_git: false,
            net_classes: Vec::new(),
            unknown: Default::default(),
        },
        pending_libraries: std::collections::HashMap::new(),
        case_fold: Default::default(),
        case_probed: false,
    });
    f.app.document_state.tabs[0].project_id = Some(id);
    crate::close_support::close_first_project(&mut f.app);
    assert!(f.app.ui_state.project_close_confirm.is_some());
    let task = f.app.update(Message::Project(ProjectMsg::CloseConfirm(
        ProjectCloseChoice::DiscardAll,
    )));
    complete(&mut f.app, task);
    assert!(!sidecar_of(&f).exists());
}

#[test]
fn an_unchanged_document_is_not_rewritten() {
    let mut f = fixture(true);
    let task = tick(&mut f);
    complete(&mut f.app, task);
    std::fs::remove_file(sidecar_of(&f)).unwrap();
    // Same revision: nothing is owed, so no new write.
    let task = tick(&mut f);
    complete(&mut f.app, task);
    assert!(!sidecar_of(&f).exists());
}

#[test]
fn a_failing_write_frees_the_slot_so_a_later_tick_can_write() {
    let mut f = fixture(true);
    // A file where the recovery directory should be makes every write fail.
    std::fs::write(&f.recovery, b"not a directory").unwrap();
    let task = tick(&mut f);
    complete(&mut f.app, task);
    std::fs::remove_file(&f.recovery).unwrap();
    let task = tick(&mut f);
    complete(&mut f.app, task);
    assert!(sidecar_of(&f).exists());
}

#[test]
fn turning_autosave_off_then_closing_the_document_removes_the_sidecar() {
    let mut f = fixture(true);
    let task = tick(&mut f);
    complete(&mut f.app, task);
    assert!(sidecar_of(&f).exists());

    // Autosave goes off; the dirty edits are then discarded and the tab closed.
    f.app.ui_state.autosave_enabled = false;
    f.app.document_state.dirty_paths.remove(&f.doc);
    let close = f.app.update(Message::ContextMenu(ContextMenuMsg::TabAction(
        TabContextAction::Close(0),
    )));
    complete(&mut f.app, close);

    assert!(
        !sidecar_of(&f).exists(),
        "cleanup must not depend on the preference"
    );
}

#[test]
fn a_tick_with_autosave_off_sweeps_stale_sidecars_but_never_writes() {
    let mut f = fixture(true);
    let task = tick(&mut f);
    complete(&mut f.app, task);
    assert!(sidecar_of(&f).exists());

    f.app.ui_state.autosave_enabled = false;
    f.app.document_state.engines.remove(&f.doc);
    f.app.document_state.dirty_paths.remove(&f.doc);
    let task = tick(&mut f);
    complete(&mut f.app, task);
    assert!(!sidecar_of(&f).exists());
    assert!(recovery_dir_is_empty(&f.recovery));
}

#[test]
fn quitting_with_discard_all_removes_the_sidecar_within_the_first_minute() {
    let mut f = fixture(true);
    let task = tick(&mut f);
    complete(&mut f.app, task);
    assert!(sidecar_of(&f).exists());

    // The document is still dirty: the quit modal's Discard All is what
    // throws the edits away.
    f.app.ui_state.app_quit_confirm = Some(signex_app::app::AppQuitConfirmState {
        dirty_paths: vec![f.doc.clone()],
        dirty_rows: Vec::new(),
    });
    let task = f.app.update(Message::Project(ProjectMsg::AppQuitConfirm(
        ProjectCloseChoice::DiscardAll,
    )));
    complete(&mut f.app, task);
    assert!(!sidecar_of(&f).exists());
}

#[test]
fn a_new_write_waits_for_a_removal_of_the_same_sidecar() {
    let mut f = fixture(true);
    let task = tick(&mut f);
    complete(&mut f.app, task);
    assert!(sidecar_of(&f).exists());

    // A removal is started but its completion has not come back yet.
    f.app.ui_state.app_quit_confirm = Some(signex_app::app::AppQuitConfirmState {
        dirty_paths: vec![f.doc.clone()],
        dirty_rows: Vec::new(),
    });
    let _removal = f.app.update(Message::Project(ProjectMsg::AppQuitConfirm(
        ProjectCloseChoice::DiscardAll,
    )));
    let task = tick(&mut f);
    assert!(
        run(task).is_empty(),
        "a write must not start while a removal of the same path runs"
    );
}

fn quit_modal(f: &mut Fixture) {
    f.app.ui_state.app_quit_confirm = Some(signex_app::app::AppQuitConfirmState {
        dirty_paths: vec![f.doc.clone()],
        dirty_rows: Vec::new(),
    });
}

#[test]
fn quitting_with_save_all_removes_the_sidecar() {
    let mut f = fixture(true);
    let task = tick(&mut f);
    complete(&mut f.app, task);
    assert!(sidecar_of(&f).exists());

    quit_modal(&mut f);
    let task = f.app.update(Message::Project(ProjectMsg::AppQuitConfirm(
        ProjectCloseChoice::SaveAll,
    )));
    complete(&mut f.app, task);

    assert_ne!(std::fs::read_to_string(&f.doc).unwrap(), ORIGINAL);
    assert!(!sidecar_of(&f).exists(), "Save All must drop the copy");
}

#[test]
fn discard_all_removes_a_sidecar_whose_write_was_still_running() {
    let mut f = fixture(true);
    let write = tick(&mut f);
    quit_modal(&mut f);
    let discard = f.app.update(Message::Project(ProjectMsg::AppQuitConfirm(
        ProjectCloseChoice::DiscardAll,
    )));
    complete(&mut f.app, discard);
    // The write lands after the discard.
    complete(&mut f.app, write);
    assert!(!sidecar_of(&f).exists(), "no sidecar of discarded edits");
}
