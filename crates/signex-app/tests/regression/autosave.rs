//! #104 — opt-in autosave of dirty schematics to a recovery sidecar.
//!
//! Off by default and silent while off; on, a tick writes a sidecar in the
//! recovery directory and never the real file; a normal save (or a close)
//! removes the sidecar, including one whose write was still running.

#[path = "autosave/primitives.rs"]
mod primitives;

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
            rules: None,
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
    // The quit waits for the running write (#1164), so the discard can only
    // finish once the write has landed: run them side by side.
    let (_discard_out, write_out) = run_side_by_side(discard, write);
    for message in write_out {
        let task = f.app.update(message);
        complete(&mut f.app, task);
    }
    assert!(!sidecar_of(&f).exists(), "no sidecar of discarded edits");
}

/// Run `waiting` on another thread and `running` on this one; return what
/// each emitted. For a quit that waits on a write that has not run yet.
fn run_side_by_side(
    waiting: Task<Message>,
    running: Task<Message>,
) -> (Vec<Message>, Vec<Message>) {
    let waiter = std::thread::spawn(move || run(waiting));
    let ran = run(running);
    (waiter.join().unwrap(), ran)
}

// ── #1164: quit waits, and a superseded write cleans up after itself ──

#[test]
fn a_discarded_write_removes_its_own_sidecar_without_any_completion_message() {
    let mut f = fixture(true);
    let write = tick(&mut f);
    quit_modal(&mut f);
    let discard = f.app.update(Message::Project(ProjectMsg::AppQuitConfirm(
        ProjectCloseChoice::DiscardAll,
    )));
    // The app stops processing messages: the write runs, its completion
    // is never fed back to `update`.
    let _ = run_side_by_side(discard, write);
    assert!(
        !sidecar_of(&f).exists(),
        "the task itself must remove what a discarding quit superseded"
    );
}

#[test]
fn quit_waits_for_a_write_that_is_still_running() {
    let mut f = fixture(true);
    let write = tick(&mut f);
    let main = iced::window::Id::unique();
    f.app.ui_state.main_window_id = Some(main);
    let quit = f.app.update(Message::Window(
        signex_app::app::WindowMsg::SecondaryWindowClosed(main),
    ));

    // Whatever the interleaving, a quit that really waits only returns once
    // the write has landed, so the waiter must see the sidecar. No sleep:
    // the outcome never depends on timing, only the chance of catching a
    // regression does.
    let sidecar = sidecar_of(&f);
    let (started_tx, started_rx) = std::sync::mpsc::channel();
    let waiter = std::thread::spawn(move || {
        started_tx.send(()).unwrap();
        let _ = run(quit);
        sidecar.exists()
    });
    started_rx.recv().unwrap();
    let _ = run(write);
    assert!(
        waiter.join().unwrap(),
        "quit must not finish before the sidecar write has landed"
    );
}

// ── #1164: recovery prompt ──

/// A second app (the next run) opens `f.doc` while a sidecar from the
/// first run's autosave is on disk and newer than the file.
fn next_run_opens_with_a_newer_copy() -> (Fixture, Signex, SchematicSheet) {
    let mut f = fixture(true);
    let task = tick(&mut f);
    complete(&mut f.app, task);
    assert!(sidecar_of(&f).exists());
    age(&f.doc, 600);

    let autosaved = f.app.document_state.engines[&f.doc].document().clone();
    let (mut next, _boot) = Signex::new();
    next.document_state
        .autosave
        .set_dir(Some(f.recovery.clone()));
    open_from_disk(&mut next, &f.doc);
    (f, next, autosaved)
}

fn age(path: &Path, secs_ago: u64) {
    let when = std::time::SystemTime::now() - std::time::Duration::from_secs(secs_ago);
    std::fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(when)
        .unwrap();
}

fn open_from_disk(app: &mut Signex, doc: &Path) {
    let sheet = SchematicSheet {
        uuid: uuid::Uuid::new_v4(),
        paper_size: "A4".to_string(),
        ..Default::default()
    };
    let task = app.update(Message::File(FileMsg::SchematicOpenFinished {
        path: doc.to_path_buf(),
        title: "autosave".to_string(),
        result: Ok(Box::new(sheet)),
    }));
    complete(app, task);
}

fn answer(app: &mut Signex, choice: signex_app::autosave::RecoveryChoice) {
    let task = app.update(Message::Autosave(AutosaveMsg::Recovery(choice)));
    complete(app, task);
}

#[test]
fn opening_a_document_with_a_newer_copy_offers_it() {
    let (f, next, _) = next_run_opens_with_a_newer_copy();
    let offer = next.document_state.autosave.current_offer().expect("offer");
    assert_eq!(offer.path, f.doc);
    assert!(sidecar_of(&f).exists(), "offering deletes nothing");
}

#[test]
fn restore_loads_the_copy_as_unsaved_edits_and_never_writes_the_file() {
    let (f, mut next, autosaved) = next_run_opens_with_a_newer_copy();
    answer(&mut next, signex_app::autosave::RecoveryChoice::Restore);

    assert!(next.document_state.autosave.current_offer().is_none());
    assert!(
        next.document_state.dirty_paths.contains(&f.doc),
        "restored content is unsaved"
    );
    assert_eq!(
        next.document_state.engines[&f.doc].document().uuid,
        autosaved.uuid,
        "the engine now holds the recovery copy"
    );
    assert_eq!(std::fs::read_to_string(&f.doc).unwrap(), ORIGINAL);
    assert!(sidecar_of(&f).exists(), "the copy stays until a save");

    // The next save removes it.
    let task = next.update(Message::File(FileMsg::Save));
    complete(&mut next, task);
    assert!(!sidecar_of(&f).exists());
}

#[test]
fn discard_deletes_the_copy_and_leaves_the_document_clean() {
    let (f, mut next, _) = next_run_opens_with_a_newer_copy();
    answer(&mut next, signex_app::autosave::RecoveryChoice::Discard);
    assert!(!sidecar_of(&f).exists());
    assert!(!next.document_state.dirty_paths.contains(&f.doc));
    assert_eq!(std::fs::read_to_string(&f.doc).unwrap(), ORIGINAL);
}

#[test]
fn decide_later_keeps_the_copy_for_the_next_open() {
    let (f, mut next, _) = next_run_opens_with_a_newer_copy();
    answer(&mut next, signex_app::autosave::RecoveryChoice::Later);
    assert!(next.document_state.autosave.current_offer().is_none());
    assert!(sidecar_of(&f).exists());
    assert!(!next.document_state.dirty_paths.contains(&f.doc));
}

#[test]
fn a_copy_identical_to_the_file_is_removed_at_open_and_never_offered() {
    let mut f = fixture(true);
    let task = tick(&mut f);
    complete(&mut f.app, task);
    let copy = std::fs::read(sidecar_of(&f)).unwrap();
    std::fs::write(&f.doc, copy).unwrap();

    let (mut next, _boot) = Signex::new();
    next.document_state
        .autosave
        .set_dir(Some(f.recovery.clone()));
    open_from_disk(&mut next, &f.doc);
    assert!(next.document_state.autosave.current_offer().is_none());
    assert!(!sidecar_of(&f).exists(), "reconciled before any prompt");
}

#[test]
fn a_differing_copy_is_offered_even_when_the_file_mtime_is_newer() {
    let mut f = fixture(true);
    let task = tick(&mut f);
    complete(&mut f.app, task);
    age(&sidecar_of(&f), 600);
    age(&f.doc, 60);

    let (mut next, _boot) = Signex::new();
    next.document_state
        .autosave
        .set_dir(Some(f.recovery.clone()));
    open_from_disk(&mut next, &f.doc);
    assert!(next.document_state.autosave.current_offer().is_some());
    assert!(sidecar_of(&f).exists());
}

#[test]
fn decide_later_then_an_edit_and_a_tick_never_overwrites_the_earlier_copy() {
    let (f, mut next, _) = next_run_opens_with_a_newer_copy();
    let earlier = std::fs::read(sidecar_of(&f)).unwrap();
    answer(&mut next, signex_app::autosave::RecoveryChoice::Later);

    next.ui_state.autosave_enabled = true;
    next.document_state.dirty_paths.insert(f.doc.clone());
    let task = next.update(Message::Autosave(AutosaveMsg::Ticked));
    complete(&mut next, task);
    assert_eq!(
        std::fs::read(sidecar_of(&f)).unwrap(),
        earlier,
        "this session's snapshot must not replace the kept copy"
    );
}

#[test]
fn decide_later_then_a_save_keeps_the_copy_and_it_is_offered_at_the_next_open() {
    let (f, mut next, _) = next_run_opens_with_a_newer_copy();
    answer(&mut next, signex_app::autosave::RecoveryChoice::Later);
    let task = next.update(Message::File(FileMsg::Save));
    complete(&mut next, task);
    assert!(
        sidecar_of(&f).exists(),
        "a save must not drop the kept copy"
    );

    // The save made the file newer than the copy; the next open still
    // offers the copy because its bytes differ.
    age(&sidecar_of(&f), 600);
    let (mut third, _boot) = Signex::new();
    third
        .document_state
        .autosave
        .set_dir(Some(f.recovery.clone()));
    open_from_disk(&mut third, &f.doc);
    assert!(third.document_state.autosave.current_offer().is_some());
}

#[test]
fn restoring_into_a_closed_tab_says_so_and_keeps_the_copy() {
    let (f, mut next, _) = next_run_opens_with_a_newer_copy();
    next.document_state.tabs.clear();
    answer(&mut next, signex_app::autosave::RecoveryChoice::Restore);
    assert!(next.document_state.error_notice.is_some());
    assert!(sidecar_of(&f).exists());
}

#[test]
fn restore_over_edits_made_while_the_prompt_was_open_is_refused() {
    let (f, mut next, _) = next_run_opens_with_a_newer_copy();
    next.document_state.dirty_paths.insert(f.doc.clone());
    answer(&mut next, signex_app::autosave::RecoveryChoice::Restore);
    assert!(next.document_state.error_notice.is_some());
    assert!(sidecar_of(&f).exists(), "the copy is kept");
}

#[test]
fn a_closed_document_withdraws_its_offer() {
    let (_f, mut next, _) = next_run_opens_with_a_newer_copy();
    next.document_state.tabs.clear();
    let task = next.update(Message::Autosave(AutosaveMsg::Ticked));
    complete(&mut next, task);
    assert!(next.document_state.autosave.current_offer().is_none());
}
