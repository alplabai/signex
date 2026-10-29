//! #893 part 1: library mutations run on a worker, not inside `update`.
//!
//! A real git-backed library is mounted; the other process's write lock is
//! played by a held `flock` on the library's lock file.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use iced::Task;
use signex_library::adapters::local_git::{LibraryInitOptions, LocalGitAdapter};
use signex_library::library_file::{FORMAT_TOKEN, LibrarySection, SnxlibManifest};
use signex_library::manifest::{LibraryMode, UsersConfig, WorkflowConfig};
use signex_library::{LibraryAdapter, PrimitiveKind, PrimitiveRef, RowId};
use uuid::Uuid;

use crate::app::{Message, Signex};
use crate::library::component_preview::test_row;
use crate::library::messages::{BrowserEditMsg, LibraryMessage};
use crate::library::state::{
    ComponentPreviewState, EditRowModalState, EditorAddress, LibraryBrowserState,
    LibraryDisplaySettings, NewTableDraft, OpenLibrary,
};
use crate::library::writes::test_support::{run_task, run_write};
use crate::persist::run_task::outputs;

const TABLE: &str = "R";

struct Fixture {
    app: Signex,
    file: PathBuf,
    library_id: Uuid,
    rows: Vec<RowId>,
    /// Keeps the library's directory alive.
    _dir: tempfile::TempDir,
}

fn fixture() -> Fixture {
    signex_library::git_identity::test_identity::install();
    let dir = tempfile::tempdir().unwrap();
    // A long path, like a macOS or Windows temp dir: the Messages panel cuts
    // a line at 160 characters, and the reason must survive that.
    let file = dir
        .path()
        .join("a-long-directory-name-like-a-windows-temp-folder")
        .join("Lib")
        .join("Lib.snxlib");
    let library_id = Uuid::now_v7();
    let manifest = SnxlibManifest {
        format: FORMAT_TOKEN.into(),
        library_id,
        library: LibrarySection {
            name: "Lib".into(),
            description: None,
        },
        mode: LibraryMode::default(),
        workflow: WorkflowConfig::default(),
        users: UsersConfig::default(),
        classes: Vec::new(),
    };
    let adapter = LocalGitAdapter::init(
        &file,
        manifest,
        LibraryInitOptions {
            enable_git: true,
            use_lfs: false,
        },
    )
    .expect("init");
    let mut rows = Vec::new();
    for pn in ["R-1", "R-2"] {
        let mut seed = test_row();
        seed.row_id = Uuid::now_v7();
        seed.internal_pn = signex_library::InternalPn::new(pn);
        seed.symbol_ref = PrimitiveRef::new(library_id, Uuid::now_v7());
        adapter.insert_row(TABLE, seed.clone(), "seed").unwrap();
        rows.push(RowId::from_uuid(seed.row_id));
    }
    adapter.create_empty_table("Empty", "seed").unwrap();
    let cached = adapter.read_table(TABLE).unwrap();
    let (mut app, _task) = Signex::new();
    app.library.open_libraries.clear();
    app.library.set.mount(Box::new(adapter)).expect("mount");
    app.library.open_libraries.push(OpenLibrary {
        root: file.clone(),
        display_name: "Lib".into(),
        library_id,
        tables: std::collections::HashMap::from([(TABLE.to_string(), cached)]),
        cached_components: Vec::new(),
        cached_symbols: Vec::new(),
        cached_footprints: Vec::new(),
        cached_sims: Vec::new(),
        cached_classes: Vec::new(),
        observed_stamp: None,
        dangling: Default::default(),
        own_edit_stamp: None,
        display: LibraryDisplaySettings::default(),
        pricing: Default::default(),
    });
    let mut browser = LibraryBrowserState::new(file.clone());
    browser.active_table = Some(TABLE.to_string());
    app.library.library_browsers.insert(file.clone(), browser);
    Fixture {
        app,
        file,
        library_id,
        rows,
        _dir: dir,
    }
}

impl Fixture {
    fn browser(&mut self) -> &mut LibraryBrowserState {
        self.app
            .library
            .library_browsers
            .get_mut(&self.file)
            .unwrap()
    }

    fn address(&self, row: usize) -> EditorAddress {
        EditorAddress::new(self.file.clone(), TABLE.into(), self.rows[row])
    }

    fn cached_row(&self, row: usize) -> signex_library::ComponentRow {
        self.app
            .library
            .library_at(&self.file)
            .and_then(|lib| lib.tables.get(TABLE))
            .and_then(|rows| {
                rows.iter()
                    .find(|r| RowId::from_uuid(r.row_id) == self.rows[row])
            })
            .cloned()
            .expect("row cached")
    }

    /// What the library holds on disk: its tables and every row.
    fn on_disk(&self) -> String {
        let adapter = LocalGitAdapter::open(&self.file).expect("open");
        let mut tables = adapter.list_tables().unwrap();
        tables.sort();
        let mut out = format!("{tables:?}\n");
        for table in tables {
            for row in adapter.read_table(&table).unwrap() {
                out.push_str(&format!("{row:?}\n"));
            }
        }
        out
    }

    fn stored_pn(&self, row: usize) -> String {
        LocalGitAdapter::open(&self.file)
            .unwrap()
            .read_row(TABLE, self.rows[row])
            .unwrap()
            .internal_pn
            .as_str()
            .to_string()
    }

    fn open_modal(&mut self, row: usize) {
        let address = self.address(row);
        let draft = self.cached_row(row);
        self.browser().edit_modal = Some(EditRowModalState::new(address, draft));
    }

    fn open_preview(&mut self, row: usize) -> EditorAddress {
        let address = self.address(row);
        let preview =
            ComponentPreviewState::from_row(self.file.clone(), TABLE.into(), self.cached_row(row));
        self.app.library.editors.insert(address.clone(), preview);
        address
    }

    fn edit_modal(&mut self, msg: BrowserEditMsg) -> Task<Message> {
        self.app.handle_browser_edit_msg(self.file.clone(), msg)
    }

    fn modal_pn(&mut self) -> String {
        self.browser()
            .edit_modal
            .as_ref()
            .expect("modal open")
            .draft
            .internal_pn
            .as_str()
            .to_string()
    }
}

/// Another process's write lock on the library, held until dropped.
fn hold_write_lock(file: &Path) -> std::fs::File {
    let path = signex_library::write_lock_path(file);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)
        .unwrap();
    lock.try_lock().expect("the lock is free");
    lock
}

/// Run a started write to completion and apply its result, the way the
/// runtime does.
fn finish(app: &mut Signex, task: Task<Message>) {
    run_task(app, task);
}

// ── Each mutation returns a Task and leaves the disk alone until it runs ──

type Start = fn(&mut Fixture) -> Task<Message>;

fn start_delete_row(f: &mut Fixture) -> Task<Message> {
    f.app
        .handle_browser_delete_row_confirm(f.file.clone(), TABLE.into(), f.rows[0])
}

fn start_cell_edit(f: &mut Fixture) -> Task<Message> {
    let key = (f.rows[0], "internal_pn".to_string());
    f.browser().cell_edit.insert(key, "CELL-EDIT".into());
    f.app.handle_browser_cell_commit(
        f.file.clone(),
        TABLE.into(),
        f.rows[0],
        "internal_pn".into(),
    )
}

fn start_modal_save(f: &mut Fixture) -> Task<Message> {
    f.open_modal(0);
    let _ = f.edit_modal(BrowserEditMsg::SetInternalPn("MODAL-EDIT".into()));
    f.edit_modal(BrowserEditMsg::Save)
}

fn start_add_table(f: &mut Fixture) -> Task<Message> {
    f.browser().adding_table = Some(NewTableDraft {
        name: "Caps".into(),
        ..Default::default()
    });
    f.app.handle_browser_confirm_add_table(f.file.clone())
}

fn start_rename_table(f: &mut Fixture) -> Task<Message> {
    f.browser().renaming_table = Some((TABLE.into(), "Resistors".into()));
    f.app.handle_browser_confirm_rename_table(f.file.clone())
}

fn start_delete_table(f: &mut Fixture) -> Task<Message> {
    f.app
        .handle_browser_delete_table(f.file.clone(), "Empty".into())
}

fn start_add_row(f: &mut Fixture) -> Task<Message> {
    f.app
        .handle_browser_add_component(f.file.clone(), Some(TABLE.into()))
}

fn start_new_component(f: &mut Fixture) -> Task<Message> {
    f.app.handle_new_component()
}

fn start_browser_bind(f: &mut Fixture) -> Task<Message> {
    let primitive = PrimitiveRef::new(f.library_id, Uuid::now_v7());
    let address = f.address(0);
    f.app
        .apply_primitive_pick_to_browser_row(address, PrimitiveKind::Footprint, primitive)
}

fn start_preview_bind(f: &mut Fixture) -> Task<Message> {
    let address = f.open_preview(0);
    let primitive = PrimitiveRef::new(f.library_id, Uuid::now_v7());
    f.app
        .apply_primitive_pick_to_preview(address, PrimitiveKind::Footprint, primitive)
}

fn start_preview_save(f: &mut Fixture) -> Task<Message> {
    let address = f.open_preview(0);
    let state = f.app.library.editors.get_mut(&address).unwrap();
    state.row.description = "edited in the preview".into();
    state.dirty = true;
    f.app.handle_save_row(&address)
}

#[test]
fn every_library_mutation_returns_a_task_and_leaves_the_disk_alone_until_it_runs() {
    let mutations: [(&str, Start); 11] = [
        ("delete row", start_delete_row),
        ("cell edit", start_cell_edit),
        ("modal save", start_modal_save),
        ("add table", start_add_table),
        ("rename table", start_rename_table),
        ("delete table", start_delete_table),
        ("add row", start_add_row),
        ("new component", start_new_component),
        ("bind in the browser", start_browser_bind),
        ("bind in the preview", start_preview_bind),
        ("preview save", start_preview_save),
    ];
    for (name, start) in mutations {
        let mut f = fixture();
        let before = f.on_disk();
        let task = start(&mut f);
        assert_eq!(
            f.on_disk(),
            before,
            "{name}: `update` must not touch the disk"
        );
        assert!(
            f.app.library.writes.pending(&f.file) > 0,
            "{name}: the write is owed, in the Model, while it is queued"
        );
        assert!(
            iced_runtime::task::into_stream(task.map(|m| m)).is_some(),
            "{name}: update must return a Task"
        );
        let mut ran = fixture();
        let before = ran.on_disk();
        let task = start(&mut ran);
        finish(&mut ran.app, task);
        assert_ne!(ran.on_disk(), before, "{name}: running the task writes");
        assert_eq!(ran.app.library.writes.pending(&ran.file), 0, "{name}");
    }
}

// ── The UI thread does not wait for the write lock ──────────────────────

#[test]
fn update_returns_at_once_while_another_process_holds_the_write_lock() {
    let mut f = fixture();
    let _held = hold_write_lock(&f.file);
    f.open_modal(0);
    let _ = f.edit_modal(BrowserEditMsg::SetInternalPn("LOCKED".into()));
    let key = (f.rows[1], "internal_pn".to_string());
    f.browser().cell_edit.insert(key, "LOCKED-CELL".into());

    let started = Instant::now();
    let tasks = vec![
        f.edit_modal(BrowserEditMsg::Save),
        f.app.handle_browser_cell_commit(
            f.file.clone(),
            TABLE.into(),
            f.rows[1],
            "internal_pn".into(),
        ),
        f.app
            .handle_browser_delete_row_confirm(f.file.clone(), TABLE.into(), f.rows[1]),
    ];
    let waited = started.elapsed();
    assert!(
        waited < Duration::from_millis(1500),
        "update waited {waited:?} for the write lock"
    );
    assert!(
        tasks
            .into_iter()
            .all(|t| iced_runtime::task::into_stream(t).is_some())
    );
}

#[test]
fn a_write_that_times_out_on_the_lock_reports_in_messages_and_the_ui_shows_the_disk() {
    let _ = crate::diagnostics::init_logging();
    let mut f = fixture();
    let _held = hold_write_lock(&f.file);
    let id = f.rows[1].to_string();
    f.browser().selected_row = Some(f.rows[1]);
    let task = f
        .app
        .handle_browser_delete_row_confirm(f.file.clone(), TABLE.into(), f.rows[1]);
    finish(&mut f.app, task);

    assert!(
        crate::diagnostics::recent_entries().iter().any(|entry| {
            entry.message.contains(&id)
                && entry.message.contains("was not deleted")
                && entry.message.contains("another Signex process")
        }),
        "the refused delete must reach the Messages panel"
    );
    // The row is still there, on disk and in what the grid shows.
    drop(_held);
    assert_eq!(f.stored_pn(1), "R-2");
    assert_eq!(f.cached_row(1).internal_pn.as_str(), "R-2");
    let kept = Some(f.rows[1]);
    assert_eq!(
        f.browser().selected_row,
        kept,
        "a refused delete keeps the selection"
    );
    assert!(f.browser().delete_confirm.is_none());
}

// ── No lost edits ───────────────────────────────────────────────────────

#[test]
fn a_modal_edit_made_while_its_save_runs_is_not_lost() {
    let mut f = fixture();
    f.open_modal(0);
    let _ = f.edit_modal(BrowserEditMsg::SetInternalPn("FIRST".into()));
    let task = f.edit_modal(BrowserEditMsg::Save);
    // The user goes on typing while the worker writes.
    let _ = f.edit_modal(BrowserEditMsg::SetInternalPn("SECOND".into()));
    finish(&mut f.app, task);

    assert_eq!(f.stored_pn(0), "FIRST", "the copy that was saved is stored");
    assert!(
        f.browser().edit_modal.is_some(),
        "the modal stays open: it holds an edit that is not stored"
    );
    assert_eq!(f.modal_pn(), "SECOND", "the typed edit survives the result");
    // And it saves on its own Save, on the revision the first save made.
    let task = f.edit_modal(BrowserEditMsg::Save);
    finish(&mut f.app, task);
    assert_eq!(f.stored_pn(0), "SECOND");
    assert!(f.browser().edit_modal.is_none());
}

#[test]
fn a_modal_save_that_is_current_when_it_lands_closes_the_modal() {
    let mut f = fixture();
    f.open_modal(0);
    let _ = f.edit_modal(BrowserEditMsg::SetInternalPn("ONLY".into()));
    let task = f.edit_modal(BrowserEditMsg::Save);
    finish(&mut f.app, task);
    assert_eq!(f.stored_pn(0), "ONLY");
    assert!(f.browser().edit_modal.is_none());
}

#[test]
fn a_cell_typed_on_while_its_save_runs_keeps_the_newer_text() {
    let mut f = fixture();
    let key = (f.rows[0], "internal_pn".to_string());
    f.browser().cell_edit.insert(key.clone(), "CELL-1".into());
    let task = f.app.handle_browser_cell_commit(
        f.file.clone(),
        TABLE.into(),
        f.rows[0],
        "internal_pn".into(),
    );
    f.browser().cell_edit.insert(key.clone(), "CELL-12".into());
    finish(&mut f.app, task);

    assert_eq!(f.stored_pn(0), "CELL-1");
    assert_eq!(
        f.browser().cell_edit.get(&key).map(String::as_str),
        Some("CELL-12"),
        "what was typed after the commit is still in the cell"
    );
    assert_eq!(f.cached_row(0).internal_pn.as_str(), "CELL-1");
}

#[test]
fn a_refused_cell_commit_keeps_what_was_typed() {
    let mut f = fixture();
    let _held = hold_write_lock(&f.file);
    let key = (f.rows[0], "internal_pn".to_string());
    f.browser()
        .cell_edit
        .insert(key.clone(), "NOT-SAVED".into());
    let task = f.app.handle_browser_cell_commit(
        f.file.clone(),
        TABLE.into(),
        f.rows[0],
        "internal_pn".into(),
    );
    finish(&mut f.app, task);
    assert_eq!(
        f.browser().cell_edit.get(&key).map(String::as_str),
        Some("NOT-SAVED")
    );
    drop(_held);
    assert_eq!(f.stored_pn(0), "R-1");
}

// ── Order ───────────────────────────────────────────────────────────────

#[test]
fn two_quick_edits_to_one_library_land_in_the_order_they_were_made() {
    let mut f = fixture();
    // Create a table, then rename it: the rename only works if the create
    // ran first.
    f.browser().adding_table = Some(NewTableDraft {
        name: "Caps".into(),
        ..Default::default()
    });
    let create = f.app.handle_browser_confirm_add_table(f.file.clone());
    f.browser().renaming_table = Some(("Caps".into(), "Capacitors".into()));
    let rename = f.app.handle_browser_confirm_rename_table(f.file.clone());

    // The runtime may wake the second worker first.
    let later = std::thread::spawn(move || outputs(rename));
    std::thread::sleep(Duration::from_millis(150));
    let first = outputs(create);
    let second = later.join().unwrap();
    for message in first.into_iter().chain(second) {
        let next = f.app.update(message);
        run_task(&mut f.app, next);
    }

    let tables = LocalGitAdapter::open(&f.file)
        .unwrap()
        .list_tables()
        .unwrap();
    assert!(tables.contains(&"Capacitors".to_string()), "{tables:?}");
    assert!(!tables.contains(&"Caps".to_string()), "{tables:?}");
    assert_eq!(f.browser().active_table.as_deref(), Some("Capacitors"));
}

fn commit_cell(f: &mut Fixture, row: usize, column: &str, text: &str) -> Task<Message> {
    let key = (f.rows[row], column.to_string());
    f.browser().cell_edit.insert(key, text.into());
    f.app
        .handle_browser_cell_commit(f.file.clone(), TABLE.into(), f.rows[row], column.into())
}

fn stored_row(f: &Fixture, row: usize) -> signex_library::ComponentRow {
    LocalGitAdapter::open(&f.file)
        .unwrap()
        .read_row(TABLE, f.rows[row])
        .unwrap()
}

#[test]
fn two_quick_edits_to_different_fields_of_one_row_both_land() {
    let mut f = fixture();
    let first = commit_cell(&mut f, 0, "internal_pn", "A-FIRST");
    let second = commit_cell(&mut f, 0, "mpn", "A-MPN");
    // Both run at once, as the runtime would run them.
    finish(&mut f.app, Task::batch([first, second]));
    let stored = stored_row(&f, 0);
    assert_eq!(stored.internal_pn.as_str(), "A-FIRST");
    assert_eq!(stored.primary_mpn.mpn, "A-MPN");
    assert_eq!(f.cached_row(0).internal_pn.as_str(), "A-FIRST");
    assert_eq!(f.cached_row(0).primary_mpn.mpn, "A-MPN");
    assert!(
        f.browser().cell_edit.is_empty(),
        "nothing is left to retype"
    );
}

#[test]
fn two_quick_edits_to_one_field_leave_the_last_value() {
    let mut f = fixture();
    let first = commit_cell(&mut f, 0, "internal_pn", "A-1");
    let second = commit_cell(&mut f, 0, "internal_pn", "A-2");
    finish(&mut f.app, Task::batch([first, second]));
    assert_eq!(stored_row(&f, 0).internal_pn.as_str(), "A-2");
    assert_eq!(f.cached_row(0).internal_pn.as_str(), "A-2");
}

#[test]
fn an_edit_queued_behind_a_change_from_outside_is_still_refused() {
    let _ = crate::diagnostics::init_logging();
    let mut f = fixture();
    let first = commit_cell(&mut f, 0, "internal_pn", "A-FIRST");
    let second = commit_cell(&mut f, 0, "mpn", "A-MPN");
    run_write(&mut f.app, first);
    // Another process changes the row before the second edit runs.
    let adapter = LocalGitAdapter::open(&f.file).unwrap();
    let mut theirs = adapter.read_row(TABLE, f.rows[0]).unwrap();
    theirs.description = "theirs".into();
    adapter.update_row(TABLE, theirs, "other window").unwrap();
    run_write(&mut f.app, second);

    let stored = stored_row(&f, 0);
    assert_eq!(stored.description, "theirs", "their change is kept");
    assert_ne!(stored.primary_mpn.mpn, "A-MPN", "the edit was refused");
    let key = (f.rows[0], "mpn".to_string());
    assert_eq!(
        f.browser().cell_edit.get(&key).map(String::as_str),
        Some("A-MPN"),
        "the refused text stays to retry"
    );
}

// ── Quit and close wait ─────────────────────────────────────────────────

#[test]
fn quit_waits_for_a_library_write_in_flight() {
    let mut f = fixture();
    let in_flight = f.app.library.writes.turn(&f.file);
    let marker = Message::File(crate::app::FileMsg::DocumentSaveWaitFailed(
        "after the writes".into(),
    ));
    let task = f.app.flush_quit_writes_then(Task::done(marker));
    let mut stream = iced_runtime::task::into_stream(task).expect("a real task");
    let (sender, receiver) = std::sync::mpsc::channel();
    let runner = std::thread::spawn(move || {
        use futures::StreamExt;
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(async move {
                while let Some(action) = stream.next().await {
                    if let iced_runtime::Action::Output(Message::File(
                        crate::app::FileMsg::DocumentSaveWaitFailed(text),
                    )) = action
                        && text == "after the writes"
                    {
                        let _ = sender.send(());
                    }
                }
            });
    });
    assert!(
        receiver.recv_timeout(Duration::from_millis(600)).is_err(),
        "quit ran while a library write was still in flight"
    );
    drop(in_flight);
    receiver
        .recv_timeout(Duration::from_secs(10))
        .expect("quit continues once the write is done");
    runner.join().unwrap();
}

#[test]
fn closing_a_library_never_unmounts_under_a_running_write() {
    let mut f = fixture();
    let in_flight = f.app.library.writes.turn(&f.file);
    f.app.library.close_library(&f.file);
    assert!(
        f.app.library.library_at(&f.file).is_some(),
        "the library stays mounted while its write runs"
    );
    assert!(f.app.library.closes_ready().is_empty());
    drop(in_flight);
    f.app.close_libraries_whose_writes_are_done();
    assert!(
        f.app.library.library_at(&f.file).is_none(),
        "and goes once the write is done"
    );
}

#[test]
fn a_close_does_not_block_update() {
    let mut f = fixture();
    let _held = f.app.library.writes.turn(&f.file);
    let started = Instant::now();
    f.app.library.close_library(&f.file);
    assert!(started.elapsed() < Duration::from_millis(500));
}

// ── A failed write leaves the UI on the disk's truth ────────────────────

#[test]
fn a_failed_table_create_keeps_the_form_with_the_reason_and_no_phantom_table() {
    let mut f = fixture();
    f.browser().adding_table = Some(NewTableDraft {
        name: TABLE.into(),
        ..Default::default()
    });
    let task = f.app.handle_browser_confirm_add_table(f.file.clone());
    finish(&mut f.app, task);
    let draft = f.browser().adding_table.clone().expect("form stays open");
    assert!(draft.error.is_some(), "the reason is shown");
    assert!(!draft.saving, "and it can be confirmed again");
    assert_ne!(f.browser().active_table.as_deref(), Some("nope"));
}

#[test]
fn a_failed_modal_save_keeps_the_draft_and_shows_the_stored_revision_is_stale() {
    let mut f = fixture();
    f.open_modal(0);
    let _ = f.edit_modal(BrowserEditMsg::SetInternalPn("MINE".into()));
    // Someone else moves the row on first.
    let adapter = LocalGitAdapter::open(&f.file).unwrap();
    let mut theirs = adapter.read_row(TABLE, f.rows[0]).unwrap();
    theirs.description = "theirs".into();
    adapter.update_row(TABLE, theirs, "other window").unwrap();
    let task = f.edit_modal(BrowserEditMsg::Save);
    finish(&mut f.app, task);
    let modal = f.browser().edit_modal.clone().expect("modal stays open");
    assert_eq!(
        modal.draft.internal_pn.as_str(),
        "MINE",
        "the draft is kept"
    );
    assert!(modal.stale_base, "the stale refusal offers the reload");
    assert!(modal.saving.is_none());
    assert_eq!(f.stored_pn(0), "R-1");
}

#[test]
fn the_write_finished_message_is_a_library_message() {
    // The completion is routed through the Library namespace, not a new
    // flat variant of the root `Message`.
    let mut f = fixture();
    let task = start_delete_row(&mut f);
    let messages = outputs(task);
    assert!(
        messages
            .iter()
            .any(|m| matches!(m, Message::Library(LibraryMessage::WriteFinished(_))))
    );
}

#[test]
fn the_git_reinit_of_a_library_runs_off_the_ui_thread() {
    let _ = crate::diagnostics::init_logging();
    use crate::library::recovery::{GitMissingChoice, RecoveryDialog};
    signex_library::git_identity::test_identity::install();
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("Lib").join("Lib.snxlib");
    let manifest = SnxlibManifest {
        format: FORMAT_TOKEN.into(),
        library_id: Uuid::now_v7(),
        library: LibrarySection {
            name: "Lib".into(),
            description: None,
        },
        mode: LibraryMode::default(),
        workflow: WorkflowConfig::default(),
        users: UsersConfig::default(),
        classes: Vec::new(),
    };
    LocalGitAdapter::init(
        &file,
        manifest,
        LibraryInitOptions {
            enable_git: false,
            use_lfs: false,
        },
    )
    .expect("init");
    let git_dir = file.parent().unwrap().join(".git");
    assert!(!git_dir.exists());
    let (mut app, _task) = Signex::new();
    app.library.recovery = Some(RecoveryDialog::GitMissing {
        path: file.clone(),
        remote: None,
        error: None,
    });
    let task = app.update(Message::Library(LibraryMessage::RecoveryGitMissing(
        GitMissingChoice::ReInit,
    )));
    assert!(!git_dir.exists(), "`update` must not run git");
    assert!(app.library.writes.pending(&file) > 0);
    // A second click while it runs starts nothing.
    drop(
        app.update(Message::Library(LibraryMessage::RecoveryGitMissing(
            GitMissingChoice::ReInit,
        ))),
    );
    assert_eq!(app.library.writes.pending(&file), 1, "one re-init owed");
    assert!(iced_runtime::task::into_stream(task.map(|m| m)).is_some());
    let mut ran = Signex::new().0;
    ran.library.recovery = Some(RecoveryDialog::GitMissing {
        path: file.clone(),
        remote: None,
        error: None,
    });
    let task = ran.update(Message::Library(LibraryMessage::RecoveryGitMissing(
        GitMissingChoice::ReInit,
    )));
    let finished: Vec<_> = outputs(task);
    for message in finished {
        drop(ran.update(message));
    }
    assert!(git_dir.exists(), "the worker ran `git init`");
    assert!(ran.library.recovery.is_none());
}

fn bind_footprint(f: &mut Fixture, row: usize, primitive: PrimitiveRef) -> Task<Message> {
    let address = f.address(row);
    f.app
        .apply_primitive_pick_to_browser_row(address, PrimitiveKind::Footprint, primitive)
}

#[test]
fn a_rebased_cell_write_caches_the_row_it_wrote_so_a_later_write_keeps_the_bind() {
    let mut f = fixture();
    let primitive = PrimitiveRef::new(f.library_id, Uuid::now_v7());
    // A bind, then a cell edit of the same row built before the bind landed:
    // the cell edit is rebased onto the bind.
    let bind = bind_footprint(&mut f, 0, primitive);
    let x = commit_cell(&mut f, 0, "internal_pn", "X-EDIT");
    run_write(&mut f.app, Task::batch([bind, x]));
    // The re-read has not landed. A third edit is built from the cache.
    let y = commit_cell(&mut f, 0, "mpn", "Y-EDIT");
    run_write(&mut f.app, y);

    let stored = stored_row(&f, 0);
    assert_eq!(stored.footprint_ref, Some(primitive), "the bind survives");
    assert_eq!(stored.internal_pn.as_str(), "X-EDIT");
    assert_eq!(stored.primary_mpn.mpn, "Y-EDIT");
}

fn description_dirty_preview(f: &mut Fixture, row: usize, text: &str) -> EditorAddress {
    let address = f.open_preview(row);
    let state = f.app.library.editors.get_mut(&address).unwrap();
    state.row.description = text.into();
    state.dirty = true;
    address
}

#[test]
fn a_save_all_ahead_of_a_cell_edit_loses_neither() {
    let mut f = fixture();
    let address = description_dirty_preview(&mut f, 0, "from the preview");
    let save = f
        .app
        .begin_save_row_in_batch(&address, 1)
        .expect("starts")
        .expect("a task");
    let cell = commit_cell(&mut f, 0, "mpn", "CELL-MPN");
    run_write(&mut f.app, Task::batch([save, cell]));
    let stored = stored_row(&f, 0);
    assert_eq!(stored.description, "from the preview");
    assert_eq!(
        stored.primary_mpn.mpn, "CELL-MPN",
        "the cell edit was not refused"
    );
    assert!(f.app.library.batch_row_failures.is_empty());
}

#[test]
fn a_save_all_behind_a_cell_edit_is_refused_visibly_and_keeps_the_draft() {
    let mut f = fixture();
    let address = description_dirty_preview(&mut f, 0, "from the preview");
    let cell = commit_cell(&mut f, 0, "mpn", "CELL-MPN");
    let save = f
        .app
        .begin_save_row_in_batch(&address, 1)
        .expect("starts")
        .expect("a task");
    run_write(&mut f.app, Task::batch([cell, save]));
    let stored = stored_row(&f, 0);
    assert_eq!(stored.primary_mpn.mpn, "CELL-MPN", "the cell edit landed");
    assert_ne!(stored.description, "from the preview");
    let editor = f.app.library.editors.get(&address).unwrap();
    assert!(editor.dirty, "the draft stays for the user");
    assert_eq!(
        f.app.library.batch_row_failures.len(),
        1,
        "and Save All names it"
    );
}

#[test]
fn a_bind_and_a_save_all_of_one_preview_end_up_consistent() {
    let mut f = fixture();
    let address = description_dirty_preview(&mut f, 0, "from the preview");
    let primitive = PrimitiveRef::new(f.library_id, Uuid::now_v7());
    let bind =
        f.app
            .apply_primitive_pick_to_preview(address.clone(), PrimitiveKind::Footprint, primitive);
    // A Save All while the bind is in flight is refused, not raced.
    assert!(f.app.begin_save_row_in_batch(&address, 1).is_err());
    run_write(&mut f.app, bind);
    let save = f
        .app
        .begin_save_row_in_batch(&address, 2)
        .expect("starts")
        .expect("a task");
    run_write(&mut f.app, save);
    let stored = stored_row(&f, 0);
    assert_eq!(stored.footprint_ref, Some(primitive));
    assert_eq!(stored.description, "from the preview");
    assert!(!f.app.library.editors.get(&address).unwrap().dirty);
    assert!(!f.app.library.editors.get(&address).unwrap().stale_base);
}

#[test]
fn a_failed_new_component_insert_reaches_messages() {
    let _ = crate::diagnostics::init_logging();
    let mut f = fixture();
    let _held = hold_write_lock(&f.file);
    let task = f
        .app
        .handle_browser_add_component(f.file.clone(), Some(TABLE.into()));
    finish(&mut f.app, task);
    assert!(
        crate::diagnostics::recent_entries()
            .iter()
            .any(|entry| entry.message.contains("New component was not added")),
        "the failed insert must be in Messages"
    );
}

#[path = "deferred_close_tests.rs"]
mod deferred_close_tests;

// Unix only: the test makes its unreadable file with `chmod 000`.
#[cfg(unix)]
#[path = "held_back_tests.rs"]
mod held_back_tests;
