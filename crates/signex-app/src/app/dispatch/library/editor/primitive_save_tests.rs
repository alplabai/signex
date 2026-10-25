//! #859 — symbol / footprint container saves run off the UI thread.

use std::path::{Path, PathBuf};

use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{Message, Signex, SymbolEditorState, TabInfo, TabKind};
use crate::persist::run_task::outputs;

/// An app with a dirty symbol editor (and its tab) keyed at `dir/Lib.snxsym`
/// whose project has git on.
fn app_with_dirty_symbol(dir: &Path) -> (Signex, PathBuf) {
    let mut app = app_workspace(&dir.to_string_lossy(), &[]);
    app.document_state.projects[0].data.enable_git = true;
    let path = dir.join("Lib.snxsym");
    let file = signex_library::SymbolFile::from_symbol(signex_library::Symbol::empty("Sym1"));
    let mut editor = SymbolEditorState::new(path.clone(), file);
    editor.dirty = true;
    app.document_state
        .symbol_editors
        .insert(path.clone(), editor);
    app.document_state.dirty_paths.insert(path.clone());
    app.document_state.tabs.push(TabInfo {
        title: "Lib".into(),
        path: path.clone(),
        cached_document: None,
        dirty: true,
        project_id: None,
        kind: TabKind::SymbolEditor(path.clone()),
    });
    (app, path)
}

fn land(app: &mut Signex, task: iced::Task<Message>) {
    for message in outputs(task) {
        let _ = app.update(message);
    }
}

#[test]
fn the_write_happens_when_the_task_runs_not_in_update() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, path) = app_with_dirty_symbol(dir.path());

    let task = app.save_primitive_tab_at(&path).expect("save starts");
    assert!(!path.exists(), "update itself wrote nothing");
    assert!(
        app.document_state.dirty_paths.contains(&path),
        "nothing is clean before the write completes"
    );

    land(&mut app, task);
    assert!(path.exists(), "the task wrote the container");
    assert!(!app.document_state.dirty_paths.contains(&path));
    assert!(!app.document_state.tabs[0].dirty);
    assert!(!app.document_state.symbol_editors[&path].dirty);
}

#[test]
fn a_saved_primitive_still_commits_to_the_project_git_repo() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, path) = app_with_dirty_symbol(dir.path());
    let task = app.save_primitive_tab_at(&path).expect("save starts");
    assert!(app.document_state.inflight_git_commits.is_empty());
    land(&mut app, task);
    assert_eq!(app.document_state.inflight_git_commits.len(), 1);
}

#[test]
fn a_failed_write_reaches_the_messages_panel_and_keeps_the_draft_dirty() {
    let _ = crate::diagnostics::init_logging();
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, _) = app_with_dirty_symbol(dir.path());
    // A file where the container's directory belongs makes the write fail.
    let blocker = dir.path().join("blocker");
    std::fs::write(&blocker, b"x").expect("blocker");
    let path = blocker.join("Lib.snxsym");
    let editor = app
        .document_state
        .symbol_editors
        .remove(&dir.path().join("Lib.snxsym"))
        .expect("editor");
    app.document_state
        .symbol_editors
        .insert(path.clone(), editor);
    app.document_state.dirty_paths.insert(path.clone());

    let task = app.save_primitive_tab_at(&path).expect("save starts");
    land(&mut app, task);

    assert!(app.document_state.symbol_editors[&path].dirty);
    assert!(app.document_state.dirty_paths.contains(&path));
    assert!(app.document_state.error_notice.is_some());
    assert!(app.document_state.inflight_git_commits.is_empty());
    let path_text = path.display().to_string();
    assert!(
        app.document_state
            .panel_ctx
            .diagnostics
            .iter()
            .any(|entry| entry.message.contains(&path_text)),
        "the failure must reach the Messages panel"
    );
}

#[test]
fn an_edit_during_the_write_keeps_the_draft_dirty() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, path) = app_with_dirty_symbol(dir.path());
    let task = app.save_primitive_tab_at(&path).expect("save starts");
    // Every canvas edit sets this flag.
    app.document_state
        .symbol_editors
        .get_mut(&path)
        .expect("editor")
        .dirty = true;

    land(&mut app, task);

    assert!(path.exists(), "the snapshot was written");
    assert!(app.document_state.symbol_editors[&path].dirty);
    assert!(app.document_state.dirty_paths.contains(&path));
    assert!(app.document_state.tabs[0].dirty);
}

#[test]
fn quit_waits_for_a_running_container_write() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, path) = app_with_dirty_symbol(dir.path());
    let task = app.save_primitive_tab_at(&path).expect("save starts");
    let barrier = app.document_state.document_saves.barrier();
    let (tx, rx) = std::sync::mpsc::channel();
    let waiter = std::thread::spawn(move || {
        barrier.wait();
        tx.send(()).expect("send");
    });
    assert!(
        rx.recv_timeout(std::time::Duration::from_millis(200))
            .is_err(),
        "the barrier holds while the write has not run"
    );
    let _ = outputs(task);
    rx.recv_timeout(std::time::Duration::from_secs(5))
        .expect("the barrier opens once the write finished");
    waiter.join().expect("waiter");
    assert!(path.exists());
}

#[test]
fn a_second_save_of_a_path_still_writing_is_refused_without_losing_the_dirty_flag() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, path) = app_with_dirty_symbol(dir.path());
    let _running = app.save_primitive_tab_at(&path).expect("save starts");
    app.document_state
        .symbol_editors
        .get_mut(&path)
        .expect("editor")
        .dirty = true;
    assert!(app.save_primitive_tab_at(&path).is_err());
    assert!(app.document_state.symbol_editors[&path].dirty);
}

#[test]
fn a_failed_save_as_puts_the_tab_back_where_it_was() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, from) = app_with_dirty_symbol(dir.path());
    let blocker = dir.path().join("blocker");
    std::fs::write(&blocker, b"x").expect("blocker");
    let to = blocker.join("Other.snxsym");

    let task = app.handle_save_primitive_as(&from, &to).expect("starts");
    assert_eq!(
        app.document_state.tabs[0].path, to,
        "re-keyed while writing"
    );
    land(&mut app, task);

    assert_eq!(app.document_state.tabs[0].path, from);
    assert!(app.document_state.symbol_editors.contains_key(&from));
    assert!(app.document_state.symbol_editors[&from].dirty);
    assert!(app.document_state.dirty_paths.contains(&from));
    assert!(app.document_state.error_notice.is_some());
}
