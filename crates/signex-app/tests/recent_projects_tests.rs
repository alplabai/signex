// This integration-test binary discards fallible test-setup calls with
// `let _ = ...` routinely; see `passive_calculator_tests.rs`.
#![expect(
    clippy::let_underscore_must_use,
    reason = "integration tests discard Results with `let _ = ...` routinely; this is test scaffolding, not a production `iced::Task` being dropped, and `[lints]` in Cargo.toml is package-scoped so each integration-test crate root needs its own"
)]

//! File ▸ Open Recent end to end: the real open path, the real prefs
//! file (redirected to a per-process temp dir by `test-prefs-redirect`).
//!
//! Every test shares that one prefs file, so they serialise on a lock and
//! each starts from a cleared list.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use signex_app::app::{FileMsg, Message, Signex};
use signex_app::menu_bar::MenuMessage;

static PREFS_LOCK: Mutex<()> = Mutex::new(());

/// A fresh app with an empty recent list, holding the prefs lock.
fn fresh_app() -> (Signex, MutexGuard<'static, ()>) {
    let guard = PREFS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (mut app, _startup) = Signex::new();
    let _ = app.update(Message::Menu(MenuMessage::ClearRecentProjects));
    (app, guard)
}

fn make_project(dir: &Path, name: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, b"").expect("create project");
    path
}

fn open(app: &mut Signex, path: &Path) {
    let _ = app.update(Message::File(FileMsg::Opened(Some(path.to_path_buf()))));
}

#[test]
fn opening_a_project_records_it_and_survives_a_restart() {
    let (mut app, _guard) = fresh_app();
    let dir = tempfile::tempdir().expect("tempdir");
    let project = make_project(dir.path(), "demo.snxprj");

    open(&mut app, &project);

    assert_eq!(app.ui_state.recent_projects.entries().len(), 1);
    // A "restart" is a new app reading the same prefs file.
    let (restarted, _startup) = Signex::new();
    assert_eq!(
        restarted.ui_state.recent_projects.entries(),
        app.ui_state.recent_projects.entries()
    );
    assert_eq!(
        signex_app::fonts::read_recent_projects(),
        app.ui_state.recent_projects.entries()
    );
}

#[test]
fn a_failed_open_is_not_recorded() {
    let (mut app, _guard) = fresh_app();
    let dir = tempfile::tempdir().expect("tempdir");

    open(&mut app, &dir.path().join("missing.snxprj"));

    assert!(app.ui_state.recent_projects.entries().is_empty());
    assert!(signex_app::fonts::read_recent_projects().is_empty());
}

#[test]
fn reopening_moves_to_front_without_a_duplicate() {
    let (mut app, _guard) = fresh_app();
    let dir = tempfile::tempdir().expect("tempdir");
    let a = make_project(dir.path(), "a.snxprj");
    let b = make_project(dir.path(), "b.snxprj");

    open(&mut app, &a);
    open(&mut app, &b);
    open(&mut app, &a);

    let entries = app.ui_state.recent_projects.entries();
    assert_eq!(entries.len(), 2);
    assert!(entries[0].ends_with("a.snxprj"));
    assert!(entries[1].ends_with("b.snxprj"));
}

#[test]
fn choosing_a_vanished_entry_drops_it_and_says_so() {
    let (mut app, _guard) = fresh_app();
    let dir = tempfile::tempdir().expect("tempdir");
    let project = make_project(dir.path(), "gone.snxprj");
    open(&mut app, &project);
    let listed = app.ui_state.recent_projects.entries()[0].clone();
    std::fs::remove_file(&project).expect("delete project");
    let projects_before = app.document_state.projects.len();

    let _ = app.update(Message::Menu(MenuMessage::OpenRecentProject(listed)));

    assert!(app.ui_state.recent_projects.entries().is_empty());
    assert!(signex_app::fonts::read_recent_projects().is_empty());
    let notice = app
        .document_state
        .error_notice
        .as_ref()
        .expect("a visible notice");
    assert!(notice.detail.contains("gone.snxprj"));
    assert_eq!(
        app.document_state.projects.len(),
        projects_before,
        "nothing may be half-opened"
    );
}

#[test]
fn choosing_a_live_entry_reopens_it_and_clear_empties_the_list() {
    let (mut app, _guard) = fresh_app();
    let dir = tempfile::tempdir().expect("tempdir");
    let project = make_project(dir.path(), "live.snxprj");
    open(&mut app, &project);
    let listed = app.ui_state.recent_projects.entries()[0].clone();
    app.document_state.projects.clear();

    let _ = app.update(Message::Menu(MenuMessage::OpenRecentProject(listed)));
    assert_eq!(app.document_state.projects.len(), 1, "the project reopened");
    assert!(app.document_state.error_notice.is_none());

    let _ = app.update(Message::Menu(MenuMessage::ClearRecentProjects));
    assert!(app.ui_state.recent_projects.entries().is_empty());
    assert!(signex_app::fonts::read_recent_projects().is_empty());
}
