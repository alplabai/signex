//! #859 — Save All writes off the UI thread and only then closes / quits.

use std::path::{Path, PathBuf};

use crate::app::contracts::SaveAllNext;
use crate::app::{FileMsg, Message, Signex, SymbolEditorState, TabInfo, TabKind};
use crate::persist::run_task::outputs;

/// An app with a dirty symbol draft at `dir/Lib.snxsym` and a dirty
/// schematic at `dir/Sheet.snxsch`.
fn app_with_two_dirty_documents(dir: &Path) -> (Signex, PathBuf, PathBuf) {
    let (mut app, _task) = Signex::new();
    let symbol_path = dir.join("Lib.snxsym");
    let file = signex_library::SymbolFile::from_symbol(signex_library::Symbol::empty("Sym1"));
    let mut editor = SymbolEditorState::new(symbol_path.clone(), file);
    editor.dirty = true;
    app.document_state
        .symbol_editors
        .insert(symbol_path.clone(), editor);
    app.document_state.dirty_paths.insert(symbol_path.clone());

    let sheet_path = dir.join("Sheet.snxsch");
    let mut engine = signex_engine::Engine::new(signex_types::schematic::SchematicSheet {
        uuid: uuid::Uuid::new_v4(),
        paper_size: "A4".to_string(),
        ..Default::default()
    })
    .expect("engine");
    engine.set_path(Some(sheet_path.clone()));
    app.document_state
        .engines
        .insert(sheet_path.clone(), engine);
    app.document_state.dirty_paths.insert(sheet_path.clone());
    app.document_state.tabs.push(TabInfo {
        title: "Sheet".into(),
        path: sheet_path.clone(),
        cached_document: None,
        dirty: true,
        project_id: None,
        kind: TabKind::Schematic,
    });
    (app, symbol_path, sheet_path)
}

fn save_all(app: &mut Signex, paths: &[PathBuf]) -> iced::Task<Message> {
    app.begin_save_all("Save All", paths, &[], SaveAllNext::Quit)
}

fn feed(app: &mut Signex, messages: Vec<Message>) {
    for message in messages {
        let _ = app.update(message);
    }
}

fn ends_with_settle(messages: &[Message]) -> bool {
    matches!(
        messages.last(),
        Some(Message::File(FileMsg::SaveAllSettled(_)))
    )
}

#[test]
fn save_all_writes_when_the_task_runs_and_settles_after_every_write() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, symbol, sheet) = app_with_two_dirty_documents(dir.path());

    let task = save_all(&mut app, &[symbol.clone(), sheet.clone()]);
    assert!(!symbol.exists() && !sheet.exists(), "update wrote nothing");

    let messages = outputs(task);
    assert!(symbol.exists() && sheet.exists());
    assert!(
        ends_with_settle(&messages),
        "the settle message follows the completions"
    );
    feed(&mut app, messages);
    assert!(app.document_state.dirty_paths.is_empty());
    assert!(app.document_state.error_notice.is_none());
}

#[test]
fn a_path_nothing_backs_is_reported_with_a_reason() {
    let (mut app, _task) = Signex::new();
    let err = app
        .begin_dirty_path_save(Path::new("/nonexistent/Ghost.snxsym"))
        .err()
        .expect("no editor backs it");
    assert!(
        format!("{err:#}").contains("can't be saved automatically"),
        "unexpected error: {err:#}"
    );
}

#[test]
fn a_failed_write_keeps_signex_open_and_names_the_file_and_the_reason() {
    let _ = crate::diagnostics::init_logging();
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, symbol, sheet) = app_with_two_dirty_documents(dir.path());
    // A file where the sheet's directory belongs makes only that write fail.
    let blocker = dir.path().join("blocker");
    std::fs::write(&blocker, b"x").expect("blocker");
    let blocked = blocker.join("Sheet.snxsch");
    let engine = app.document_state.engines.remove(&sheet).expect("engine");
    app.document_state.engines.insert(blocked.clone(), engine);
    app.document_state.dirty_paths.remove(&sheet);
    app.document_state.dirty_paths.insert(blocked.clone());

    let task = save_all(&mut app, &[symbol.clone(), blocked.clone()]);
    feed(&mut app, outputs(task));

    assert!(symbol.exists(), "the other file still saved");
    assert!(app.document_state.dirty_paths.contains(&blocked));
    let notice = app.document_state.error_notice.as_ref().expect("notice");
    assert!(format!("{notice:?}").contains("stayed open"), "{notice:?}");
    assert!(format!("{notice:?}").contains("Sheet.snxsch"), "{notice:?}");
    let path_text = blocked.display().to_string();
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
fn a_draft_edited_during_save_all_is_reported_unsaved() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, symbol, _sheet) = app_with_two_dirty_documents(dir.path());
    let task = save_all(&mut app, std::slice::from_ref(&symbol));
    app.document_state
        .symbol_editors
        .get_mut(&symbol)
        .expect("editor")
        .dirty = true;

    feed(&mut app, outputs(task));

    assert!(app.document_state.dirty_paths.contains(&symbol));
    let notice = app.document_state.error_notice.as_ref().expect("notice");
    assert!(format!("{notice:?}").contains("Lib.snxsym"), "{notice:?}");
}

#[test]
fn quit_waits_for_the_writes_of_a_running_save_all() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, symbol, sheet) = app_with_two_dirty_documents(dir.path());
    let task = save_all(&mut app, &[symbol.clone(), sheet.clone()]);
    let barrier = app.document_state.document_saves.barrier();
    let (tx, rx) = std::sync::mpsc::channel();
    let waiter = std::thread::spawn(move || {
        barrier.wait();
        tx.send(()).expect("send");
    });
    assert!(
        rx.recv_timeout(std::time::Duration::from_millis(200))
            .is_err(),
        "the barrier holds while the writes have not run"
    );
    let _ = outputs(task);
    rx.recv_timeout(std::time::Duration::from_secs(5))
        .expect("the barrier opens once both writes finished");
    waiter.join().expect("waiter");
    assert!(symbol.exists() && sheet.exists());
}
