//! #692 — the dock layout is persisted only when it changed, debounced,
//! off the UI thread, flushed on quit, and a failed write is reported.
//!
//! `DockLayoutStore` is the seam: each test swaps a recording double into
//! `document_state.dock_layout_sync`, so no `prefs.json` is touched and the
//! writes can be counted. `iced::Task`s are driven to completion with
//! `iced_runtime::task::into_stream` on a tokio runtime, which is what
//! the real runtime does with them.

use std::sync::{Arc, Mutex};
use std::thread::ThreadId;
use std::time::Instant;

use futures::StreamExt;
use iced::Task;
use signex_app::app::{Message, Signex, WindowMsg};
use signex_app::diagnostics;
use signex_app::dock::{
    DOCK_LAYOUT_SAVE_DEBOUNCE, DockLayoutMsg, DockLayoutSnapshot, DockLayoutStore, DockLayoutSync,
    DockMessage, PanelPosition,
};

#[derive(Default)]
struct Recorder {
    writes: Mutex<Vec<(ThreadId, DockLayoutSnapshot)>>,
    fail_with: Mutex<Option<String>>,
}

impl DockLayoutStore for Recorder {
    fn write(&self, layout: &DockLayoutSnapshot) -> Result<(), String> {
        self.writes
            .lock()
            .unwrap()
            .push((std::thread::current().id(), layout.clone()));
        match self.fail_with.lock().unwrap().clone() {
            Some(reason) => Err(reason),
            None => Ok(()),
        }
    }
}

impl Recorder {
    fn count(&self) -> usize {
        self.writes.lock().unwrap().len()
    }
}

fn app_with_recorder() -> (Signex, Arc<Recorder>) {
    let (mut app, _boot) = Signex::new();
    let recorder = Arc::new(Recorder::default());
    app.document_state.dock_layout_sync = DockLayoutSync::new(
        recorder.clone(),
        DockLayoutSnapshot::capture(&app.document_state.dock),
    );
    (app, recorder)
}

/// Run a task to completion the way the iced runtime would, returning the
/// messages it produced.
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

fn dock_layout_messages(messages: Vec<Message>) -> Vec<DockLayoutMsg> {
    messages
        .into_iter()
        .filter_map(|m| match m {
            Message::DockLayout(msg) => Some(msg),
            _ => None,
        })
        .collect()
}

#[test]
fn hover_and_other_transient_messages_write_nothing() {
    let (mut app, recorder) = app_with_recorder();

    let hover_in = app.update(Message::Dock(DockMessage::TabHoverEnter(
        PanelPosition::Left,
        0,
    )));
    let hover_out = app.update(Message::Dock(DockMessage::TabHoverExit(
        PanelPosition::Left,
        0,
    )));
    let scroll = app.update(Message::Dock(DockMessage::TabScroll(
        PanelPosition::Left,
        1,
    )));

    let produced: Vec<_> = [hover_in, hover_out, scroll]
        .into_iter()
        .flat_map(run)
        .collect();
    assert!(
        dock_layout_messages(produced).is_empty(),
        "transient messages must not even arm a save timer"
    );
    assert_eq!(recorder.count(), 0);
}

#[test]
fn a_real_change_writes_exactly_once_after_the_debounce_and_off_the_ui_thread() {
    let (mut app, recorder) = app_with_recorder();

    // Two layout changes in a burst: only the last timer is current.
    let first = app.update(Message::Dock(DockMessage::ToggleCollapse(
        PanelPosition::Left,
    )));
    let second = app.update(Message::Dock(DockMessage::ToggleCollapse(
        PanelPosition::Right,
    )));
    assert_eq!(recorder.count(), 0, "update itself must not write");

    let started = Instant::now();
    let elapsed = dock_layout_messages(run(Task::batch([first, second])));
    assert!(
        started.elapsed() >= DOCK_LAYOUT_SAVE_DEBOUNCE,
        "the save timer must wait out the debounce"
    );
    assert_eq!(elapsed.len(), 2, "each change armed one timer");

    // Feed the timer messages back; only the newest generation may write.
    let mut write_tasks = Vec::new();
    for msg in elapsed {
        write_tasks.push(app.update(Message::DockLayout(msg)));
    }
    assert_eq!(
        recorder.count(),
        0,
        "handling the debounce message returns a Task; the write is not in update"
    );
    let saved = dock_layout_messages(run(Task::batch(write_tasks)));

    assert_eq!(recorder.count(), 1, "exactly one write for the burst");
    let (writer_thread, written) = recorder.writes.lock().unwrap()[0].clone();
    assert_ne!(
        writer_thread,
        std::thread::current().id(),
        "the write must run off the calling (UI) thread"
    );
    assert!(written.left.collapsed && written.right.collapsed);
    assert!(matches!(saved.as_slice(), [DockLayoutMsg::Saved(Ok(()))]));
}

#[test]
fn changing_back_to_the_saved_layout_writes_nothing() {
    let (mut app, recorder) = app_with_recorder();

    let collapse = app.update(Message::Dock(DockMessage::ToggleCollapse(
        PanelPosition::Left,
    )));
    let restore = app.update(Message::Dock(DockMessage::ToggleCollapse(
        PanelPosition::Left,
    )));
    let elapsed = dock_layout_messages(run(Task::batch([collapse, restore])));
    for msg in elapsed {
        let _ = run(app.update(Message::DockLayout(msg)));
    }

    assert_eq!(recorder.count(), 0, "the layout on disk is already right");
}

#[test]
fn quit_flushes_a_pending_layout_before_exiting() {
    let (mut app, recorder) = app_with_recorder();
    let _timer_not_run = app.update(Message::Dock(DockMessage::ToggleCollapse(
        PanelPosition::Bottom,
    )));
    assert_eq!(recorder.count(), 0);

    let quit = app.update(Message::Window(WindowMsg::CloseMainWindow));
    let _ = run(quit);

    assert_eq!(recorder.count(), 1, "the pending layout must reach disk");
    assert!(recorder.writes.lock().unwrap()[0].1.bottom.collapsed);
}

#[test]
fn a_failed_write_is_reported_to_the_messages_panel_and_retried_on_the_next_change() {
    let _ = diagnostics::init_logging();
    let (mut app, recorder) = app_with_recorder();
    let reason = "dock_layout_persist disk-full marker 7e3b";
    *recorder.fail_with.lock().unwrap() = Some(reason.to_string());

    let change = app.update(Message::Dock(DockMessage::ToggleCollapse(
        PanelPosition::Left,
    )));
    let elapsed = dock_layout_messages(run(change));
    let write = app.update(Message::DockLayout(elapsed[0].clone()));
    let saved = dock_layout_messages(run(write));
    let _ = app.update(Message::DockLayout(saved[0].clone()));

    assert!(
        app.document_state
            .panel_ctx
            .diagnostics
            .iter()
            .any(|entry| entry.message.contains(reason)),
        "the failure must be in the Messages panel snapshot"
    );

    // The disk still holds the old layout, so a further change retries.
    *recorder.fail_with.lock().unwrap() = None;
    let again = app.update(Message::Dock(DockMessage::ToggleCollapse(
        PanelPosition::Right,
    )));
    let elapsed = dock_layout_messages(run(again));
    let write = app.update(Message::DockLayout(elapsed[0].clone()));
    let _ = run(write);
    assert_eq!(recorder.count(), 2, "the retry wrote after the failure");
}

/// Holds a write inside `write` until told to proceed, so a write can be
/// "already running" when quit starts.
struct GatedStore {
    entered: Mutex<std::sync::mpsc::Sender<()>>,
    proceed: Mutex<std::sync::mpsc::Receiver<()>>,
    finished: std::sync::atomic::AtomicBool,
}

impl DockLayoutStore for GatedStore {
    fn write(&self, _layout: &DockLayoutSnapshot) -> Result<(), String> {
        self.entered.lock().unwrap().send(()).unwrap();
        self.proceed.lock().unwrap().recv().unwrap();
        self.finished
            .store(true, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }
}

#[test]
fn quit_waits_for_a_write_that_is_already_running() {
    // Arrange: one write is running inside the store, nothing is pending.
    let (mut app, _boot) = Signex::new();
    let (entered_tx, entered_rx) = std::sync::mpsc::channel();
    let (proceed_tx, proceed_rx) = std::sync::mpsc::channel();
    let store = Arc::new(GatedStore {
        entered: Mutex::new(entered_tx),
        proceed: Mutex::new(proceed_rx),
        finished: std::sync::atomic::AtomicBool::new(false),
    });
    app.document_state.dock_layout_sync = DockLayoutSync::new(
        store.clone(),
        DockLayoutSnapshot::capture(&app.document_state.dock),
    );
    let _timer_not_run = app.update(Message::Dock(DockMessage::ToggleCollapse(
        PanelPosition::Left,
    )));
    let running = app
        .document_state
        .dock_layout_sync
        .flush()
        .expect("the changed layout is pending");
    let in_flight = std::thread::spawn(move || running.run());
    entered_rx.recv().unwrap();
    let release = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(300));
        proceed_tx.send(()).unwrap();
    });

    // Act
    let quit = app.update(Message::Window(WindowMsg::CloseMainWindow));
    let _ = run(quit);

    // Assert: the quit task only completed after the running write did.
    assert!(
        store.finished.load(std::sync::atomic::Ordering::SeqCst),
        "quit must not exit while a dock write is still running"
    );
    in_flight.join().unwrap().unwrap();
    release.join().unwrap();
}

/// The same loop against the real `PrefsDockLayoutStore` and
/// `try_update_prefs_json`: the layout lands in the file and the user's
/// other settings are still there.
#[test]
fn the_real_store_writes_the_dock_and_keeps_other_settings() {
    // Arrange
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("prefs.json");
    std::fs::write(
        &path,
        br#"{"theme": "signex", "erc": {"unused_pin": "off"}}"#,
    )
    .unwrap();
    let (mut app, _boot) = Signex::new();
    app.document_state.dock_layout_sync = DockLayoutSync::new(
        Arc::new(signex_app::fonts::PrefsDockLayoutStore::at(path.clone())),
        DockLayoutSnapshot::capture(&app.document_state.dock),
    );

    // Act
    let change = app.update(Message::Dock(DockMessage::ToggleCollapse(
        PanelPosition::Left,
    )));
    let elapsed = dock_layout_messages(run(change));
    let saved = dock_layout_messages(run(app.update(Message::DockLayout(elapsed[0].clone()))));

    // Assert
    assert!(matches!(saved.as_slice(), [DockLayoutMsg::Saved(Ok(()))]));
    let prefs: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(prefs["theme"], "signex");
    assert_eq!(prefs["erc"]["unused_pin"], "off");
    assert_eq!(prefs["dock"]["left"]["collapsed"], true);
    assert_eq!(prefs["dock"]["right"]["collapsed"], false);
}
