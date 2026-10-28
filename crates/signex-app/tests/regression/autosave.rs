//! #104 — opt-in autosave of dirty schematics to a recovery sidecar.
//!
//! Off by default and silent while off; on, a tick writes a sidecar in the
//! recovery directory and never the real file; a normal save (or a close)
//! removes the sidecar, including one whose write was still running.

#[path = "autosave/pcb.rs"]
mod pcb;
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
            description: None,
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
            annotation: None,
            unknown: Default::default(),
        },
        pending_libraries: std::collections::HashMap::new(),
        case_fold: Default::default(),
        case_probed: false,
        case_decided: false,
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
    // offers the copy because its bytes differ, and says the file is newer.
    age(&sidecar_of(&f), 600);
    let (mut third, _boot) = Signex::new();
    third
        .document_state
        .autosave
        .set_dir(Some(f.recovery.clone()));
    open_from_disk(&mut third, &f.doc);
    let offer = third
        .document_state
        .autosave
        .current_offer()
        .expect("offer");
    assert!(
        offer.older_than_file,
        "the prompt must warn that Restore replaces newer saved work"
    );
}

/// What the next run does with the earlier run's copy while the user has
/// not answered: edit the document and let the autosave timer fire.
fn edit_and_tick(app: &mut Signex, doc: &Path) {
    app.ui_state.autosave_enabled = true;
    app.document_state.dirty_paths.insert(doc.to_path_buf());
    let task = app.update(Message::Autosave(AutosaveMsg::Ticked));
    complete(app, task);
}

#[test]
fn an_unanswered_offer_blocks_autosave_from_replacing_the_earlier_copy() {
    let (f, mut next, _) = next_run_opens_with_a_newer_copy();
    let earlier = std::fs::read(sidecar_of(&f)).unwrap();
    assert!(next.document_state.autosave.current_offer().is_some());

    edit_and_tick(&mut next, &f.doc);

    assert!(
        std::fs::read(sidecar_of(&f)).unwrap() == earlier,
        "this session's snapshot must not replace a copy still waiting for an answer"
    );
}

#[test]
fn a_tick_between_open_and_the_recovery_check_never_replaces_the_earlier_copy() {
    let mut f = fixture(true);
    let task = tick(&mut f);
    complete(&mut f.app, task);
    age(&f.doc, 600);
    let earlier = std::fs::read(sidecar_of(&f)).unwrap();

    let (mut next, _boot) = Signex::new();
    next.document_state
        .autosave
        .set_dir(Some(f.recovery.clone()));
    let sheet = SchematicSheet {
        uuid: uuid::Uuid::new_v4(),
        paper_size: "A4".to_string(),
        ..Default::default()
    };
    // The open has landed, but its recovery check has not run yet.
    let check = next.update(Message::File(FileMsg::SchematicOpenFinished {
        path: f.doc.clone(),
        title: "autosave".to_string(),
        result: Ok(Box::new(sheet)),
    }));
    edit_and_tick(&mut next, &f.doc);
    assert!(
        std::fs::read(sidecar_of(&f)).unwrap() == earlier,
        "no write may run between open and the recovery check"
    );

    complete(&mut next, check);
    assert!(next.document_state.autosave.current_offer().is_some());
}

#[test]
fn quit_waits_for_the_copy_removal_that_follows_a_save() {
    let mut f = fixture(true);
    let task = tick(&mut f);
    complete(&mut f.app, task);
    assert!(sidecar_of(&f).exists());
    let save = f.app.update(Message::File(FileMsg::Save));
    let barrier = f.app.document_state.autosave.barrier();

    // The write has landed but its completion is not handled yet: the copy
    // removal it owes is not registered, so the quit barrier must be shut.
    let saved = run(save);
    assert!(
        !barrier.wait_timeout(std::time::Duration::ZERO),
        "quit would pass its autosave wait before the copy removal exists"
    );
    assert!(sidecar_of(&f).exists());

    for message in saved {
        let task = f.app.update(message);
        complete(&mut f.app, task);
    }
    assert!(barrier.wait_timeout(std::time::Duration::ZERO));
    assert!(!sidecar_of(&f).exists());
}

#[test]
fn quit_does_not_hang_when_a_save_completion_is_never_handled() {
    let mut f = fixture(true);
    let save = f.app.update(Message::File(FileMsg::Save));
    let main = iced::window::Id::unique();
    f.app.ui_state.main_window_id = Some(main);
    let quit = f.app.update(Message::Window(
        signex_app::app::WindowMsg::SecondaryWindowClosed(main),
    ));
    // The save runs, its completion message is dropped unhandled.
    drop(run(save));

    let (done_tx, done_rx) = std::sync::mpsc::channel();
    let quitting = std::thread::spawn(move || {
        let _ = run(quit);
        done_tx.send(()).unwrap();
    });
    done_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("a dropped save completion must release the quit wait");
    quitting.join().unwrap();
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
    let earlier = std::fs::read(sidecar_of(&f)).unwrap();
    next.document_state.dirty_paths.insert(f.doc.clone());
    answer(&mut next, signex_app::autosave::RecoveryChoice::Restore);
    assert!(next.document_state.error_notice.is_some());
    assert!(sidecar_of(&f).exists(), "the copy is kept");

    // Neither version may be lost: the copy is offered again and autosave
    // stays off until it is answered, then Discard hands the slot back.
    assert!(next.document_state.autosave.current_offer().is_some());
    edit_and_tick(&mut next, &f.doc);
    assert!(
        std::fs::read(sidecar_of(&f)).unwrap() == earlier,
        "autosave must not replace the copy while it awaits an answer"
    );
    answer(&mut next, signex_app::autosave::RecoveryChoice::Discard);
    edit_and_tick(&mut next, &f.doc);
    assert!(
        sidecar_of(&f).exists(),
        "after Discard autosave protects this session's edits again"
    );
}

#[test]
fn a_restore_that_cannot_read_the_copy_lets_autosave_resume() {
    let (f, mut next, _) = next_run_opens_with_a_newer_copy();
    let broken = b"this is not a schematic".to_vec();
    std::fs::write(sidecar_of(&f), &broken).unwrap();
    answer(&mut next, signex_app::autosave::RecoveryChoice::Restore);
    assert!(next.document_state.error_notice.is_some());

    edit_and_tick(&mut next, &f.doc);
    assert!(
        std::fs::read(sidecar_of(&f)).unwrap() != broken,
        "autosave must protect this session's edits once the copy proved unreadable"
    );
}

#[test]
fn a_restore_waiting_for_its_read_blocks_autosave_from_replacing_the_copy() {
    let (f, mut next, _) = next_run_opens_with_a_newer_copy();
    let earlier = std::fs::read(sidecar_of(&f)).unwrap();
    let read = next.update(Message::Autosave(AutosaveMsg::Recovery(
        signex_app::autosave::RecoveryChoice::Restore,
    )));
    edit_and_tick(&mut next, &f.doc);
    assert!(
        std::fs::read(sidecar_of(&f)).unwrap() == earlier,
        "the copy being restored must not be replaced before it is read"
    );
    drop(read);
}

#[test]
fn a_restore_into_a_tab_that_cannot_be_activated_keeps_and_re_offers_the_copy() {
    let (f, mut next, _) = next_run_opens_with_a_newer_copy();
    let earlier = std::fs::read(sidecar_of(&f)).unwrap();
    let read = next.update(Message::Autosave(AutosaveMsg::Recovery(
        signex_app::autosave::RecoveryChoice::Restore,
    )));
    // The tab goes away while the copy is read; the engine stays parked.
    next.document_state.tabs.clear();
    complete(&mut next, read);
    assert!(next.document_state.error_notice.is_some());
    assert!(sidecar_of(&f).exists(), "the copy is kept");
    assert!(
        next.document_state.autosave.current_offer().is_some(),
        "the copy is offered again"
    );
    edit_and_tick(&mut next, &f.doc);
    assert!(
        std::fs::read(sidecar_of(&f)).unwrap() == earlier,
        "autosave stays off while the copy waits for an answer"
    );
}

#[test]
fn a_closed_document_withdraws_its_offer() {
    let (f, mut next, _) = next_run_opens_with_a_newer_copy();
    // A clean close drops the engine with the tab.
    close_first_tab(&mut next);
    assert!(!next.document_state.engines.contains_key(&f.doc));
    let task = next.update(Message::Autosave(AutosaveMsg::Ticked));
    complete(&mut next, task);
    assert!(next.document_state.autosave.current_offer().is_none());
}

/// Close the first tab. A dirty tab parks its engine, so the document is
/// still being autosaved although no tab shows it.
fn close_first_tab(app: &mut Signex) {
    let close = app.update(Message::ContextMenu(ContextMenuMsg::TabAction(
        TabContextAction::Close(0),
    )));
    complete(app, close);
}

#[test]
fn closing_the_tab_of_an_unanswered_offer_keeps_the_copy_safe_from_autosave() {
    let (f, mut next, _) = next_run_opens_with_a_newer_copy();
    let earlier = std::fs::read(sidecar_of(&f)).unwrap();
    next.document_state.dirty_paths.insert(f.doc.clone());
    close_first_tab(&mut next);
    assert!(
        next.document_state.engines.contains_key(&f.doc),
        "the dirty document stays parked"
    );

    edit_and_tick(&mut next, &f.doc);
    assert!(
        std::fs::read(sidecar_of(&f)).unwrap() == earlier,
        "a parked document must not overwrite the copy still waiting for an answer"
    );
}

#[test]
fn closing_the_tab_after_decide_later_keeps_the_copy_safe_from_autosave() {
    let (f, mut next, _) = next_run_opens_with_a_newer_copy();
    let earlier = std::fs::read(sidecar_of(&f)).unwrap();
    answer(&mut next, signex_app::autosave::RecoveryChoice::Later);
    next.document_state.dirty_paths.insert(f.doc.clone());
    close_first_tab(&mut next);

    edit_and_tick(&mut next, &f.doc);
    assert!(
        std::fs::read(sidecar_of(&f)).unwrap() == earlier,
        "a parked document must not overwrite a copy the user chose to keep"
    );
}

#[cfg(unix)]
#[test]
fn a_restore_that_cannot_open_the_copy_keeps_autosave_off_and_offers_it_again() {
    use std::os::unix::fs::PermissionsExt;
    let (f, mut next, _) = next_run_opens_with_a_newer_copy();
    let earlier = std::fs::read(sidecar_of(&f)).unwrap();
    let sidecar = sidecar_of(&f);
    std::fs::set_permissions(&sidecar, std::fs::Permissions::from_mode(0o000)).unwrap();
    if std::fs::read(&sidecar).is_ok() {
        // Running as a user that ignores permissions (root): nothing to test.
        return;
    }
    answer(&mut next, signex_app::autosave::RecoveryChoice::Restore);
    std::fs::set_permissions(&sidecar, std::fs::Permissions::from_mode(0o644)).unwrap();

    assert!(next.document_state.autosave.current_offer().is_some());
    edit_and_tick(&mut next, &f.doc);
    assert!(
        std::fs::read(&sidecar).unwrap() == earlier,
        "an intact copy that could not be read must not be replaced"
    );
}

#[test]
fn restoring_after_the_tab_was_closed_keeps_the_copy_safe_from_autosave() {
    let (f, mut next, _) = next_run_opens_with_a_newer_copy();
    let earlier = std::fs::read(sidecar_of(&f)).unwrap();
    next.document_state.dirty_paths.insert(f.doc.clone());
    close_first_tab(&mut next);
    assert!(next.document_state.engines.contains_key(&f.doc));

    answer(&mut next, signex_app::autosave::RecoveryChoice::Restore);
    assert!(next.document_state.error_notice.is_some());
    assert!(
        next.document_state.autosave.current_offer().is_some(),
        "the copy is offered again for when the document is reopened"
    );
    edit_and_tick(&mut next, &f.doc);
    assert!(
        std::fs::read(sidecar_of(&f)).unwrap() == earlier,
        "a refused Restore of a parked document must not let autosave replace the copy"
    );
}

/// A previous run's copy plus a next run whose recovery check cannot judge
/// it. `break_it` runs after the copy exists and before the document opens.
fn next_run_opens_with(f: &mut Fixture, break_it: impl FnOnce(&Fixture)) -> Signex {
    let task = tick(f);
    complete(&mut f.app, task);
    assert!(sidecar_of(f).exists());
    age(&f.doc, 600);
    break_it(f);
    let (mut next, _boot) = Signex::new();
    next.document_state
        .autosave
        .set_dir(Some(f.recovery.clone()));
    open_from_disk(&mut next, &f.doc);
    next
}

#[cfg(unix)]
#[test]
fn a_recovery_check_that_cannot_read_the_copy_keeps_autosave_off_for_it() {
    use std::os::unix::fs::PermissionsExt;
    let mut f = fixture(true);
    let mut earlier = Vec::new();
    let mut readable_anyway = false;
    let mut next = next_run_opens_with(&mut f, |f| {
        earlier = std::fs::read(sidecar_of(f)).unwrap();
        std::fs::set_permissions(sidecar_of(f), std::fs::Permissions::from_mode(0o000)).unwrap();
        readable_anyway = std::fs::read(sidecar_of(f)).is_ok();
    });
    std::fs::set_permissions(sidecar_of(&f), std::fs::Permissions::from_mode(0o644)).unwrap();
    if readable_anyway {
        // Running as a user that ignores permissions (root): nothing to test.
        return;
    }

    edit_and_tick(&mut next, &f.doc);
    assert!(
        std::fs::read(sidecar_of(&f)).unwrap() == earlier,
        "a copy the check could not read must not be replaced by autosave"
    );
}

#[test]
fn a_recovery_check_that_cannot_read_the_document_keeps_autosave_off_for_the_copy() {
    let mut f = fixture(true);
    let mut earlier = Vec::new();
    let mut next = next_run_opens_with(&mut f, |f| {
        earlier = std::fs::read(sidecar_of(f)).unwrap();
        std::fs::remove_file(&f.doc).unwrap();
    });

    edit_and_tick(&mut next, &f.doc);
    assert!(
        std::fs::read(sidecar_of(&f)).unwrap() == earlier,
        "a copy that was never compared with its document must not be replaced"
    );
}

#[test]
fn a_restore_of_a_copy_that_is_gone_resumes_autosave_and_says_so() {
    let (f, mut next, _) = next_run_opens_with_a_newer_copy();
    std::fs::remove_file(sidecar_of(&f)).unwrap();
    answer(&mut next, signex_app::autosave::RecoveryChoice::Restore);

    assert!(next.document_state.autosave.current_offer().is_none());
    let notice = next.document_state.error_notice.as_ref().expect("notice");
    assert!(
        notice.detail.contains("is gone"),
        "the dialog says the copy is gone: {}",
        notice.detail
    );
    edit_and_tick(&mut next, &f.doc);
    assert!(
        sidecar_of(&f).exists(),
        "with nothing left to protect, autosave writes again"
    );
}
