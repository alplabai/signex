//! Mockup row 2, decided 2026-11-24 (docs/DECISIONS.md): opening a project
//! opens no document by itself; the canvas shows the project hint.

use std::path::{Path, PathBuf};

use signex_i18n::ids;

use crate::app::path_ops::settle;
use crate::app::{FileMsg, Message, Signex, TabInfo, TabKind};
use crate::layout_probe::{LaidOutText, lay_out_texts};

const WINDOW: iced::Size = iced::Size::new(1400.0, 900.0);

/// Create project `stem` under `dir` through File ▸ New Project and return
/// its `.snxprj` and the schematic it was written with.
fn written_project(dir: &Path, stem: &str) -> (PathBuf, PathBuf) {
    let (mut app, _boot) = Signex::new();
    let picked = dir.join(format!("{stem}.snxprj"));
    settle(&mut app, Message::File(FileMsg::NewProject(Some(picked))));
    let project = app.document_state.projects[0].path.clone();
    let sheet = app
        .document_state
        .tabs
        .iter()
        .find(|t| matches!(t.kind, TabKind::Schematic))
        .expect("New Project opens its schematic")
        .path
        .clone();
    (project, sheet)
}

fn shows(texts: &[LaidOutText], label: &str) -> bool {
    texts.iter().any(|t| t.text == label)
}

#[test]
fn opening_a_saved_project_shows_the_hint_and_opens_no_document() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let (project, sheet) = written_project(dir.path(), "Hint");
    assert!(sheet.exists(), "the project has a schematic to open");

    let (mut app, _boot) = Signex::new();
    settle(&mut app, Message::File(FileMsg::Opened(Some(project))));

    let state = &app.document_state;
    assert!(state.active_project.is_some(), "the project is open");
    let sheet_or_board = |t: &&TabInfo| matches!(t.kind, TabKind::Schematic | TabKind::Pcb);
    let opened: Vec<&TabInfo> = state.tabs.iter().filter(sheet_or_board).collect();
    assert!(
        opened.is_empty(),
        "no document opens on its own: {opened:?}"
    );
    assert!(state.engines.is_empty() && state.active_path.is_none());

    let main = iced::window::Id::unique();
    app.ui_state.main_window_id = Some(main);
    app.ui_state.first_run_tour_open = false;
    let new_sheet = app
        .ui_state
        .i18n
        .message_plain(ids::START_HINT_NEW_SHEET, &[]);
    let texts = lay_out_texts(app.view(main), WINDOW);
    assert!(
        shows(&texts, &new_sheet),
        "the canvas shows the project hint: {texts:#?}"
    );
}
