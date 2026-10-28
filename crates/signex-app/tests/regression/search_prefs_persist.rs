//! #859 — the Library Browser search box is persisted debounced, off the UI thread, coalesced into one write,
//! and flushed on quit; `update` itself never touches `prefs.json`.
//!
//! The store is the seam: each test swaps a recording double into
//! `document_state.search_prefs`, so the real prefs file is untouched.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use futures::StreamExt;
use iced::Task;
use signex_app::app::{Message, Signex, WindowMsg};
use signex_app::diagnostics;
use signex_app::library::LibraryMessage;
use signex_app::persist::{
    DebouncedStore, SEARCH_PREFS_SAVE_DEBOUNCE, SearchPrefsChange, SearchPrefsMsg, SearchPrefsSync,
};

#[derive(Default)]
struct Recorder {
    writes: Mutex<Vec<SearchPrefsChange>>,
}

impl DebouncedStore<SearchPrefsChange> for Recorder {
    fn write(&self, snapshot: &SearchPrefsChange) -> Result<(), String> {
        self.writes.lock().unwrap().push(snapshot.clone());
        Ok(())
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
    app.document_state.search_prefs = SearchPrefsSync::new(recorder.clone());
    (app, recorder)
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

fn search_prefs_messages(messages: Vec<Message>) -> Vec<SearchPrefsMsg> {
    messages
        .into_iter()
        .filter_map(|m| match m {
            Message::SearchPrefs(msg) => Some(msg),
            _ => None,
        })
        .collect()
}

fn library_search(app: &mut Signex, library: &std::path::Path, value: &str) -> Task<Message> {
    app.update(Message::Library(LibraryMessage::BrowserSearchChanged {
        library_path: library.to_path_buf(),
        value: value.to_string(),
    }))
}

fn type_search(app: &mut Signex, text: &str) -> Task<Message> {
    library_search(app, &PathBuf::from("/tmp/signex-859/parts.snxlib"), text)
}

#[test]
fn keystrokes_in_a_library_browser_search_write_once_after_the_debounce() {
    let (mut app, recorder) = app_with_recorder();
    let library = PathBuf::from("/tmp/signex-859/parts.snxlib");

    let tasks: Vec<_> = ["c", "cap"]
        .into_iter()
        .map(|value| {
            app.update(Message::Library(LibraryMessage::BrowserSearchChanged {
                library_path: library.clone(),
                value: value.to_string(),
            }))
        })
        .collect();
    assert_eq!(recorder.count(), 0, "update itself must not write");

    let started = Instant::now();
    let elapsed = search_prefs_messages(run(Task::batch(tasks)));
    assert!(started.elapsed() >= SEARCH_PREFS_SAVE_DEBOUNCE);
    let saves: Vec<_> = elapsed
        .into_iter()
        .map(|msg| app.update(Message::SearchPrefs(msg)))
        .collect();
    drop(run(Task::batch(saves)));

    assert_eq!(recorder.count(), 1);
    let written = recorder.writes.lock().unwrap()[0].clone();
    assert_eq!(written.library_searches.get(&library).unwrap(), "cap");
}

#[test]
fn quit_flushes_a_pending_search_write() {
    let (mut app, recorder) = app_with_recorder();
    drop(type_search(&mut app, "ferrite"));
    assert_eq!(recorder.count(), 0);

    let quit = app.update(Message::Window(WindowMsg::CloseMainWindow));
    drop(run(quit));

    assert_eq!(recorder.count(), 1, "the pending query must reach disk");
    assert_eq!(
        recorder.writes.lock().unwrap()[0]
            .library_searches
            .values()
            .next()
            .map(String::as_str),
        Some("ferrite")
    );
}

#[test]
fn a_stale_timer_after_a_flush_writes_nothing_more() {
    let (mut app, recorder) = app_with_recorder();
    let timer = type_search(&mut app, "x");
    let elapsed = search_prefs_messages(run(timer));
    drop(run(app.update(Message::Window(WindowMsg::CloseMainWindow))));
    assert_eq!(recorder.count(), 1);

    drop(run(app.update(Message::SearchPrefs(elapsed[0].clone()))));

    assert_eq!(recorder.count(), 1, "nothing is pending any more");
}

fn app_with_real_store(path: &std::path::Path) -> Signex {
    let (mut app, _boot) = Signex::new();
    app.document_state.search_prefs = SearchPrefsSync::new(Arc::new(
        signex_app::fonts::PrefsSearchStore::at(path.to_path_buf()),
    ));
    app
}

/// Run the debounce timers to completion and feed every message back, so
/// each resulting write reaches the (real) store.
fn settle(app: &mut Signex, timers: Vec<Task<Message>>) -> Vec<SearchPrefsMsg> {
    let elapsed = search_prefs_messages(run(Task::batch(timers)));
    let saves: Vec<_> = elapsed
        .into_iter()
        .map(|msg| app.update(Message::SearchPrefs(msg)))
        .collect();
    search_prefs_messages(run(Task::batch(saves)))
}

fn read_prefs(path: &std::path::Path) -> serde_json::Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

/// The real store goes through `try_update_prefs_json`: the search
/// value lands and the user's other settings survive.
#[test]
fn the_real_store_writes_the_searches_and_keeps_other_settings() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("prefs.json");
    std::fs::write(&path, br#"{"theme": "signex"}"#).unwrap();
    let mut app = app_with_real_store(&path);
    let library = PathBuf::from("/tmp/signex-859/parts.snxlib");

    let timers = vec![library_search(&mut app, &library, "bav")];
    let saved = settle(&mut app, timers);

    assert!(matches!(saved.as_slice(), [SearchPrefsMsg::Saved(Ok(()))]));
    let prefs = read_prefs(&path);
    assert_eq!(prefs["theme"], "signex");
    assert_eq!(
        prefs["library_browser_searches"][library.display().to_string()],
        "bav"
    );
}

/// Another library's saved search (and one this session never saw, as
/// after an unreadable file at boot or a second instance) must survive a
/// keystroke in a different library.
#[test]
fn a_keystroke_in_one_library_keeps_every_other_librarys_saved_search() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("prefs.json");
    std::fs::write(
        &path,
        br#"{"library_browser_searches": {"/other/b.snxlib": "keep me"}}"#,
    )
    .unwrap();
    let mut app = app_with_real_store(&path);
    let library = PathBuf::from("/tmp/signex-859/a.snxlib");

    let timers = vec![library_search(&mut app, &library, "cap")];
    settle(&mut app, timers);

    let searches = &read_prefs(&path)["library_browser_searches"];
    assert_eq!(searches["/other/b.snxlib"], "keep me");
    assert_eq!(searches[library.display().to_string()], "cap");
}

#[test]
fn clearing_a_query_removes_only_its_own_on_disk_entry() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("prefs.json");
    std::fs::write(
        &path,
        br#"{"library_browser_searches": {"/other/b.snxlib": "keep me", "/tmp/signex-859/a.snxlib": "old"}}"#,
    )
    .unwrap();
    let mut app = app_with_real_store(&path);

    let timers = vec![library_search(
        &mut app,
        &PathBuf::from("/tmp/signex-859/a.snxlib"),
        "",
    )];
    settle(&mut app, timers);

    let searches = read_prefs(&path)["library_browser_searches"].clone();
    assert_eq!(searches["/other/b.snxlib"], "keep me");
    assert!(searches.get("/tmp/signex-859/a.snxlib").is_none());
}

/// Holds a write inside `write` until told to proceed.
struct GatedStore {
    entered: Mutex<std::sync::mpsc::Sender<()>>,
    proceed: Mutex<std::sync::mpsc::Receiver<()>>,
    finished: std::sync::atomic::AtomicBool,
}

impl DebouncedStore<SearchPrefsChange> for GatedStore {
    fn write(&self, _change: &SearchPrefsChange) -> Result<(), String> {
        self.entered.lock().unwrap().send(()).unwrap();
        self.proceed.lock().unwrap().recv().unwrap();
        self.finished
            .store(true, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }
}

#[test]
fn quit_waits_for_a_search_write_that_is_already_running() {
    let (mut app, _boot) = Signex::new();
    let (entered_tx, entered_rx) = std::sync::mpsc::channel();
    let (proceed_tx, proceed_rx) = std::sync::mpsc::channel();
    let store = Arc::new(GatedStore {
        entered: Mutex::new(entered_tx),
        proceed: Mutex::new(proceed_rx),
        finished: std::sync::atomic::AtomicBool::new(false),
    });
    app.document_state.search_prefs = SearchPrefsSync::new(store.clone());
    drop(type_search(&mut app, "tantalum"));
    let running = app
        .document_state
        .search_prefs
        .flush()
        .expect("the query is pending");
    let in_flight = std::thread::spawn(move || running.run());
    entered_rx.recv().unwrap();
    let release = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(300));
        proceed_tx.send(()).unwrap();
    });

    let quit = app.update(Message::Window(WindowMsg::CloseMainWindow));
    drop(run(quit));

    assert!(
        store.finished.load(std::sync::atomic::Ordering::SeqCst),
        "quit must not exit while a search write is still running"
    );
    in_flight.join().unwrap().unwrap();
    release.join().unwrap();
}

/// The OS-level close of the main window is the second quit path.
#[test]
fn the_main_window_closing_flushes_a_pending_search_write() {
    let (mut app, recorder) = app_with_recorder();
    let main = iced::window::Id::unique();
    app.ui_state.main_window_id = Some(main);
    drop(type_search(&mut app, "mlcc"));

    let closed = app.update(Message::Window(WindowMsg::SecondaryWindowClosed(main)));
    drop(run(closed));

    assert_eq!(recorder.count(), 1);
}

/// A failed write must not drop the typed text: quit retries it.
struct FailOnce {
    failed: std::sync::atomic::AtomicBool,
    writes: Mutex<Vec<SearchPrefsChange>>,
}

impl DebouncedStore<SearchPrefsChange> for FailOnce {
    fn write(&self, change: &SearchPrefsChange) -> Result<(), String> {
        if !self.failed.swap(true, std::sync::atomic::Ordering::SeqCst) {
            return Err("search_prefs disk-full marker 4c1d".to_string());
        }
        self.writes.lock().unwrap().push(change.clone());
        Ok(())
    }
}

#[test]
fn a_failed_search_write_is_retried_at_quit() {
    drop(diagnostics::init_logging());
    let (mut app, _boot) = Signex::new();
    let store = Arc::new(FailOnce {
        failed: std::sync::atomic::AtomicBool::new(false),
        writes: Mutex::new(Vec::new()),
    });
    app.document_state.search_prefs = SearchPrefsSync::new(store.clone());
    let timer = type_search(&mut app, "varistor");
    let saved = settle(&mut app, vec![timer]);
    assert!(matches!(saved.as_slice(), [SearchPrefsMsg::Saved(Err(_))]));
    for msg in saved {
        drop(app.update(Message::SearchPrefs(msg)));
    }
    assert!(store.writes.lock().unwrap().is_empty());

    drop(run(app.update(Message::Window(WindowMsg::CloseMainWindow))));

    let writes = store.writes.lock().unwrap();
    assert_eq!(writes.len(), 1, "quit retried the failed write");
    assert_eq!(
        writes[0]
            .library_searches
            .values()
            .next()
            .map(String::as_str),
        Some("varistor")
    );
}
