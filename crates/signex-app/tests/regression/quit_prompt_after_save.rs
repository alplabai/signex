//! #1833 — a Save made while the quit "Unsaved Edits" prompt is open must
//! not leave the prompt listing files that are no longer unsaved: it drops
//! the saved files, and closes once nothing is left.

use std::path::{Path, PathBuf};

use signex_app::app::{FileMsg, Message, Signex, TabInfo, TabKind, WindowMsg};
use signex_types::schematic::SchematicSheet;
use tempfile::TempDir;

/// Open `name` in `dir` as a dirty schematic tab and make it active.
fn open_dirty_sheet(app: &mut Signex, dir: &Path, name: &str) -> PathBuf {
    let path = dir.join(name);
    let sheet = SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
        paper_size: "A4".to_string(),
        ..Default::default()
    };
    let mut engine = signex_engine::Engine::new(sheet).expect("engine");
    engine.set_path(Some(path.clone()));
    app.document_state.engines.insert(path.clone(), engine);
    app.document_state.tabs.push(TabInfo {
        title: name.to_string(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app.document_state.active_path = Some(path.clone());
    app.document_state.mark_document_dirty(&path);
    path
}

fn request_quit(app: &mut Signex) {
    let _ = app.update(Message::Window(WindowMsg::CloseMainWindow));
    assert!(app.ui_state.app_quit_confirm.is_some(), "quit must ask");
}

fn save_active(app: &mut Signex) {
    let task = app.update(Message::File(FileMsg::Save));
    crate::drive::drive(app, task);
}

#[test]
fn saving_the_only_unsaved_file_closes_the_quit_prompt() {
    let tmp = TempDir::new().unwrap();
    let (mut app, _boot) = Signex::new();
    let sheet = open_dirty_sheet(&mut app, tmp.path(), "Only.snxsch");
    request_quit(&mut app);

    save_active(&mut app);

    assert!(sheet.exists(), "the save wrote the file");
    assert!(!app.document_state.has_dirty_documents());
    assert!(
        app.ui_state.app_quit_confirm.is_none(),
        "nothing is unsaved any more, so the prompt must not stay up"
    );
}

#[test]
fn saving_one_of_two_unsaved_files_drops_it_from_the_quit_prompt() {
    let tmp = TempDir::new().unwrap();
    let (mut app, _boot) = Signex::new();
    let other = open_dirty_sheet(&mut app, tmp.path(), "Other.snxsch");
    let saved = open_dirty_sheet(&mut app, tmp.path(), "Saved.snxsch");
    request_quit(&mut app);

    save_active(&mut app);

    assert!(saved.exists(), "the save wrote the active file");
    let modal = app
        .ui_state
        .app_quit_confirm
        .as_ref()
        .expect("one file is still unsaved, so the prompt stays");
    assert_eq!(modal.dirty_paths, vec![other], "only the unsaved file");
    assert!(modal.dirty_rows.is_empty());
}
