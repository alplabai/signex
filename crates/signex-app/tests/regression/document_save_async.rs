//! #859 — a schematic save serialises on the UI thread and writes off it.
//!
//! The dirty marker clears only when the document is unchanged since the
//! snapshot, a failed write keeps it and reaches the Messages panel, two
//! quick saves of one file land in order, and quit waits for a write that
//! is still running.
//!
//! `DocumentSink` is the seam: tests swap a recorder into
//! `document_state.document_saves`. Tasks are driven the way the iced
//! runtime would, with `iced_runtime::task::into_stream` on tokio.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread::ThreadId;
use std::time::Duration;

use futures::StreamExt;
use iced::Task;
use signex_app::app::{
    ContextMenuMsg, FileMsg, Message, Signex, TabContextAction, TabInfo, TabKind, WindowMsg,
};
use signex_app::diagnostics;
use signex_app::persist::{DocumentSaves, DocumentSink};
use signex_types::schematic::SchematicSheet;

#[derive(Default)]
struct Recorder {
    writes: Mutex<Vec<(ThreadId, String)>>,
    fail_with: Mutex<Option<String>>,
}

impl DocumentSink for Recorder {
    fn write(&self, _path: &Path, bytes: &[u8]) -> io::Result<()> {
        if let Some(reason) = self.fail_with.lock().unwrap().clone() {
            return Err(io::Error::other(reason));
        }
        self.writes.lock().unwrap().push((
            std::thread::current().id(),
            String::from_utf8_lossy(bytes).into_owned(),
        ));
        Ok(())
    }
}

impl Recorder {
    fn contents(&self) -> Vec<String> {
        self.writes
            .lock()
            .unwrap()
            .iter()
            .map(|(_, text)| text.clone())
            .collect()
    }
}

fn app_with_dirty_sheet(sink: Arc<dyn DocumentSink>) -> (Signex, PathBuf) {
    let path = PathBuf::from("document-save-async.snxsch");
    let sheet = SchematicSheet {
        uuid: uuid::Uuid::new_v4(),
        paper_size: "A4".to_string(),
        ..Default::default()
    };
    let (mut app, _boot) = Signex::new();
    app.document_state
        .engines
        .insert(path.clone(), signex_engine::Engine::new(sheet).unwrap());
    app.document_state.tabs.push(TabInfo {
        title: "document-save-async".to_string(),
        path: path.clone(),
        cached_document: None,
        dirty: true,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path.clone());
    app.document_state.dirty_paths.insert(path.clone());
    app.document_state.document_saves = DocumentSaves::new(sink);
    (app, path)
}

fn edit(app: &mut Signex, path: &Path, paper: &str) {
    app.document_state
        .engines
        .get_mut(path)
        .unwrap()
        .execute(signex_engine::Command::SetPaperSize {
            paper_size: paper.to_string(),
        })
        .unwrap();
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

/// Run a save task and feed its completion back into `update`.
fn complete(app: &mut Signex, task: Task<Message>) {
    for message in run(task) {
        let _ = app.update(message);
    }
}

fn is_dirty(app: &Signex, path: &Path) -> bool {
    app.document_state.dirty_paths.contains(path) && app.document_state.tabs[0].dirty
}

#[test]
fn update_returns_a_task_and_the_write_happens_off_the_ui_thread() {
    let recorder = Arc::new(Recorder::default());
    let (mut app, path) = app_with_dirty_sheet(recorder.clone());

    let task = app.update(Message::File(FileMsg::Save));

    assert!(
        recorder.contents().is_empty(),
        "nothing may be written inside update"
    );
    assert!(is_dirty(&app, &path), "dirty until the write is reported");
    complete(&mut app, task);

    let writes = recorder.writes.lock().unwrap();
    assert_eq!(writes.len(), 1);
    assert_ne!(writes[0].0, std::thread::current().id());
    drop(writes);
    assert!(!is_dirty(&app, &path), "a clean write clears the marker");
}

#[test]
fn an_edit_made_while_the_write_runs_keeps_the_document_dirty() {
    let recorder = Arc::new(Recorder::default());
    let (mut app, path) = app_with_dirty_sheet(recorder.clone());

    let task = app.update(Message::File(FileMsg::Save));
    edit(&mut app, &path, "A3");
    complete(&mut app, task);

    assert_eq!(recorder.contents().len(), 1);
    assert!(
        is_dirty(&app, &path),
        "the snapshot predates the edit, so the edit is still unsaved"
    );
}

#[test]
fn a_failed_write_keeps_the_document_dirty_and_reaches_the_messages_panel() {
    let _ = diagnostics::init_logging();
    let recorder = Arc::new(Recorder::default());
    let reason = "document_save_async disk-full marker 4c1d";
    *recorder.fail_with.lock().unwrap() = Some(reason.to_string());
    let (mut app, path) = app_with_dirty_sheet(recorder.clone());

    let task = app.update(Message::File(FileMsg::Save));
    complete(&mut app, task);

    assert!(is_dirty(&app, &path));
    assert!(app.document_state.error_notice.is_some());
    assert!(
        app.document_state
            .panel_ctx
            .diagnostics
            .iter()
            .any(|entry| entry.message.contains(reason)),
        "the failure must be in the Messages panel snapshot"
    );
}

#[test]
fn two_quick_saves_land_in_order_even_when_scheduled_the_other_way_round() {
    let recorder = Arc::new(Recorder::default());
    let (mut app, path) = app_with_dirty_sheet(recorder.clone());

    let first = app.update(Message::File(FileMsg::Save));
    edit(&mut app, &path, "A3");
    let second = app.update(Message::File(FileMsg::Save));

    // The newer snapshot is scheduled first; the older one must not
    // overwrite it afterwards.
    complete(&mut app, second);
    complete(&mut app, first);

    let contents = recorder.contents();
    assert_eq!(contents.len(), 1, "the stale snapshot is skipped");
    assert!(contents[0].contains("A3"), "the newest snapshot wins");
    assert!(!is_dirty(&app, &path), "the newest snapshot cleared dirty");
    assert!(
        !app.document_state
            .document_saves
            .is_saving(&app.save_ledger_key(&path))
    );
}

/// Blocks inside `write` until released, to hold a save "in flight".
struct GatedSink {
    entered: Mutex<mpsc::Sender<()>>,
    proceed: Mutex<mpsc::Receiver<()>>,
}

impl DocumentSink for GatedSink {
    fn write(&self, _path: &Path, _bytes: &[u8]) -> io::Result<()> {
        self.entered.lock().unwrap().send(()).unwrap();
        self.proceed.lock().unwrap().recv().unwrap();
        Ok(())
    }
}

#[test]
fn quit_waits_for_the_write_in_flight() {
    let (entered_tx, entered_rx) = mpsc::channel();
    let (proceed_tx, proceed_rx) = mpsc::channel();
    let sink = Arc::new(GatedSink {
        entered: Mutex::new(entered_tx),
        proceed: Mutex::new(proceed_rx),
    });
    let (mut app, _path) = app_with_dirty_sheet(sink);

    let save = app.update(Message::File(FileMsg::Save));
    let saving = std::thread::spawn(move || run(save));
    entered_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("the write is inside the sink");

    let main_window = iced::window::Id::unique();
    app.ui_state.main_window_id = Some(main_window);
    let quit = app.update(Message::Window(WindowMsg::SecondaryWindowClosed(
        main_window,
    )));
    let (done_tx, done_rx) = mpsc::channel();
    let quitting = std::thread::spawn(move || {
        let _ = run(quit);
        done_tx.send(()).unwrap();
    });

    assert!(
        done_rx.recv_timeout(Duration::from_millis(200)).is_err(),
        "quit finished while a document write was still running"
    );
    proceed_tx.send(()).unwrap();
    done_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("quit proceeds once the write finished");
    quitting.join().unwrap();
    saving.join().unwrap();
}

#[test]
fn a_replaced_engine_is_not_marked_clean_by_the_old_snapshots_completion() {
    let recorder = Arc::new(Recorder::default());
    let (mut app, path) = app_with_dirty_sheet(recorder.clone());

    let task = app.update(Message::File(FileMsg::Save));
    // A History restore / reload swaps in a brand-new engine.
    let sheet = SchematicSheet {
        uuid: uuid::Uuid::new_v4(),
        paper_size: "A4".to_string(),
        ..Default::default()
    };
    app.document_state
        .engines
        .insert(path.clone(), signex_engine::Engine::new(sheet).unwrap());
    complete(&mut app, task);

    assert!(
        is_dirty(&app, &path),
        "the new engine's content is not what the snapshot wrote"
    );
}

fn save_as(app: &mut Signex, from: &Path, to: &Path) -> Task<Message> {
    app.update(Message::File(FileMsg::SaveAs {
        from_path: from.to_path_buf(),
        to_path: to.to_path_buf(),
    }))
}

#[test]
fn save_as_rekeys_the_tab_only_after_the_write_succeeded() {
    let recorder = Arc::new(Recorder::default());
    let (mut app, path) = app_with_dirty_sheet(recorder.clone());
    let target = PathBuf::from("document-save-async-new.snxsch");

    let task = save_as(&mut app, &path, &target);
    assert_eq!(app.document_state.tabs[0].path, path, "old path until done");
    complete(&mut app, task);

    assert_eq!(app.document_state.tabs[0].path, target);
    assert!(app.document_state.engines.contains_key(&target));
    assert!(!app.document_state.engines.contains_key(&path));
    assert!(!app.document_state.tabs[0].dirty);
    assert_eq!(recorder.contents().len(), 1);
}

#[test]
fn a_failed_save_as_keeps_the_old_path_and_reports() {
    let _ = diagnostics::init_logging();
    let recorder = Arc::new(Recorder::default());
    *recorder.fail_with.lock().unwrap() = Some("save-as disk-full marker 9e2a".to_string());
    let (mut app, path) = app_with_dirty_sheet(recorder.clone());
    let target = PathBuf::from("document-save-async-new.snxsch");

    let task = save_as(&mut app, &path, &target);
    complete(&mut app, task);

    assert_eq!(app.document_state.tabs[0].path, path);
    assert!(app.document_state.engines.contains_key(&path));
    assert!(!app.document_state.engines.contains_key(&target));
    assert!(is_dirty(&app, &path));
    assert!(app.document_state.error_notice.is_some());
}

#[test]
fn save_as_does_not_rekey_when_the_tab_was_switched_mid_write() {
    let recorder = Arc::new(Recorder::default());
    let (mut app, path) = app_with_dirty_sheet(recorder.clone());
    let target = PathBuf::from("document-save-async-new.snxsch");

    let task = save_as(&mut app, &path, &target);
    app.document_state.tabs.push(TabInfo {
        title: "other".to_string(),
        path: PathBuf::from("other.snxsch"),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 1;
    complete(&mut app, task);

    assert_eq!(app.document_state.tabs[0].path, path);
    assert!(app.document_state.engines.contains_key(&path));
    assert!(!app.document_state.engines.contains_key(&target));
    assert!(app.document_state.error_notice.is_some());
}

#[test]
fn a_superseded_save_as_neither_rekeys_nor_marks_clean() {
    let recorder = Arc::new(Recorder::default());
    let (mut app, path) = app_with_dirty_sheet(recorder);
    let target = PathBuf::from("document-save-async-new.snxsch");
    let revision = app.document_state.engines[&path].revision();

    let _ = app.update(Message::File(FileMsg::DocumentSaved(
        signex_app::persist::DocumentSaveDone {
            path: target.clone(),
            ledger_key: app.save_ledger_key(&target),
            revision,
            net_color_revision: 0,
            save_as_from: Some(path.clone()),
            result: Ok(signex_app::persist::SaveOutcome::Superseded),
        },
    )));

    assert_eq!(app.document_state.tabs[0].path, path);
    assert!(!app.document_state.engines.contains_key(&target));
    assert!(is_dirty(&app, &path));
    assert!(app.document_state.error_notice.is_some());
}

#[test]
fn save_as_onto_a_file_with_a_write_in_flight_is_refused() {
    let _ = diagnostics::init_logging();
    let recorder = Arc::new(Recorder::default());
    let (mut app, path) = app_with_dirty_sheet(recorder.clone());
    let target = PathBuf::from("document-save-async-new.snxsch");

    let first = save_as(&mut app, &path, &target);
    assert!(
        app.document_state
            .document_saves
            .is_saving(&app.save_ledger_key(&target))
    );
    let second = save_as(&mut app, &path, &target);
    complete(&mut app, second);
    assert!(recorder.contents().is_empty(), "the refused Save As wrote");
    assert!(app.document_state.error_notice.is_some());
    complete(&mut app, first);
    assert_eq!(recorder.contents().len(), 1);
}

#[test]
fn closing_the_tab_mid_save_drops_the_parked_engine_once_the_write_lands() {
    let recorder = Arc::new(Recorder::default());
    let (mut app, path) = app_with_dirty_sheet(recorder.clone());

    let task = app.update(Message::File(FileMsg::Save));
    let _ = app.update(Message::ContextMenu(ContextMenuMsg::TabAction(
        TabContextAction::Close(0),
    )));
    assert!(
        app.document_state.engines.contains_key(&path),
        "a dirty engine is parked while the write is unconfirmed"
    );
    complete(&mut app, task);

    assert!(!app.document_state.engines.contains_key(&path));
    assert!(!app.document_state.dirty_paths.contains(&path));
}

#[test]
fn a_superseded_plain_save_keeps_the_document_dirty_until_the_newer_job_lands() {
    let recorder = Arc::new(Recorder::default());
    let (mut app, path) = app_with_dirty_sheet(recorder.clone());

    let first = app.update(Message::File(FileMsg::Save));
    let second = app.update(Message::File(FileMsg::Save));

    // The newer job writes first; the older one is then skipped as stale.
    // Deliver the older completion before the newer one: it must not clear
    // the marker, only the newer job's completion does.
    let newer = run(second);
    let older = run(first);
    for message in older {
        let _ = app.update(message);
    }
    assert!(is_dirty(&app, &path), "the newer job has not landed yet");

    for message in newer {
        let _ = app.update(message);
    }
    assert_eq!(recorder.contents().len(), 1);
    assert!(!is_dirty(&app, &path));
}

#[test]
fn a_dirty_and_edited_parked_engine_survives_a_mid_save_close() {
    let recorder = Arc::new(Recorder::default());
    let (mut app, path) = app_with_dirty_sheet(recorder.clone());

    let task = app.update(Message::File(FileMsg::Save));
    edit(&mut app, &path, "A3");
    let _ = app.update(Message::ContextMenu(ContextMenuMsg::TabAction(
        TabContextAction::Close(0),
    )));
    complete(&mut app, task);

    assert!(
        app.document_state.engines.contains_key(&path),
        "the unsaved edit lives only in the parked engine"
    );
    assert!(app.document_state.dirty_paths.contains(&path));
}
