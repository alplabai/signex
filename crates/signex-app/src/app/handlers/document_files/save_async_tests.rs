//! #859 — what a save completion does to git and to a parked engine.

use std::path::{Path, PathBuf};

use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::state::WindowKind;
use crate::app::{FileMsg, Message, Signex};
use crate::persist::{DocumentSaveDone, SaveOutcome};

fn app_with_engine(dir: &Path) -> (Signex, PathBuf, u64) {
    let mut app = app_workspace(&dir.to_string_lossy(), &[]);
    app.document_state.projects[0].data.enable_git = true;
    let path = dir.join("a.snxsch");
    let sheet = signex_types::schematic::SchematicSheet {
        uuid: uuid::Uuid::new_v4(),
        ..Default::default()
    };
    let engine = signex_engine::Engine::new(sheet).expect("engine");
    let revision = engine.revision();
    app.document_state.engines.insert(path.clone(), engine);
    (app, path, revision)
}

fn land(app: &mut Signex, path: &Path, revision: u64) {
    let _ = app.update(Message::File(FileMsg::DocumentSaved(DocumentSaveDone {
        path: path.to_path_buf(),
        revision,
        save_as_from: None,
        result: Ok(SaveOutcome::Written),
    })));
}

#[test]
fn a_completion_for_the_current_revision_commits_to_git() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, path, revision) = app_with_engine(dir.path());
    land(&mut app, &path, revision);
    assert_eq!(app.document_state.inflight_git_commits.len(), 1);
}

#[test]
fn a_completion_for_another_revision_makes_no_save_commit() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, path, revision) = app_with_engine(dir.path());
    land(&mut app, &path, revision.wrapping_add(1));
    assert!(app.document_state.inflight_git_commits.is_empty());
    assert!(app.document_state.pending_git_commits.is_empty());
}

#[test]
fn a_clean_engine_is_dropped_unless_an_undocked_window_holds_the_path() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, path, revision) = app_with_engine(dir.path());
    app.ui_state.windows.insert(
        iced::window::Id::unique(),
        WindowKind::UndockedTab {
            path: path.clone(),
            title: "a".into(),
        },
    );
    land(&mut app, &path, revision);
    assert!(app.document_state.engines.contains_key(&path));

    app.ui_state.windows.clear();
    land(&mut app, &path, revision);
    assert!(!app.document_state.engines.contains_key(&path));
}
