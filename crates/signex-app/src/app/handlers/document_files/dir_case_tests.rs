//! #1447 — open and Ctrl+S decide "one file or two" from the directory's
//! probed case rule, folding until it answers.

use std::path::{Path, PathBuf};

use crate::app::state::project_case::CaseProbeOutcome;
use crate::app::{FileMsg, Message, Signex};

const DIR: &str = "/w/loose-1447";

fn sheet() -> signex_types::schematic::SchematicSheet {
    signex_types::schematic::SchematicSheet {
        uuid: uuid::Uuid::new_v4(),
        ..Default::default()
    }
}

fn held() -> PathBuf {
    PathBuf::from(DIR).join("Lib.snxsch")
}

fn variant() -> PathBuf {
    PathBuf::from(DIR).join("lib.snxsch")
}

/// A loose `Lib.snxsch` open in a tab with another tab focused, no project.
fn app_holding_loose_lib() -> Signex {
    let (mut app, _task) = Signex::new();
    app.open_schematic_tab(held(), "Lib".to_string(), sheet());
    app.document_state.tabs.push(crate::app::TabInfo {
        title: "Other".to_string(),
        path: PathBuf::from(DIR).join("Other.snxsch"),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: crate::app::TabKind::Schematic,
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app
}

pub(super) fn probed(app: &mut Signex, answer: CaseProbeOutcome) {
    let _task = app.update(Message::File(FileMsg::DirCaseProbed {
        dir: PathBuf::from(DIR),
        answer,
    }));
}

fn request_open(app: &mut Signex, path: PathBuf) {
    let _task: iced::Task<Message> = app.handle_document_file_opened(Some(path));
}

fn finish_open(app: &mut Signex, path: PathBuf) {
    let _task = app.update(Message::File(FileMsg::SchematicOpenFinished {
        title: "Lib".to_string(),
        path,
        result: Ok(Box::new(sheet())),
    }));
}

fn active_path(app: &Signex) -> &Path {
    &app.document_state.tabs[app.document_state.active_tab].path
}

#[test]
fn opening_a_case_variant_focuses_the_held_tab_when_the_directory_folds() {
    let mut app = app_holding_loose_lib();
    probed(&mut app, CaseProbeOutcome::Decided(true));

    request_open(&mut app, variant());

    assert_eq!(
        app.document_state.tabs.len(),
        2,
        "no second tab for one file"
    );
    assert_eq!(active_path(&app), held());
    assert!(app.document_state.pending_opens.is_empty());
}

#[test]
fn opening_a_case_variant_waits_for_an_unprobed_directory() {
    let mut app = app_holding_loose_lib();

    request_open(&mut app, variant());
    request_open(&mut app, variant());

    assert_eq!(app.document_state.tabs.len(), 2, "nothing opened yet");
    assert!(
        app.document_state.pending_opens.contains(&variant()),
        "the request is marked so a repeat is blocked"
    );
    assert!(!app.document_state.engines.contains_key(&variant()));

    // The probe lands: the directory folds, so the waiting open focuses the
    // held tab instead of opening a copy.
    probed(&mut app, CaseProbeOutcome::Decided(true));

    assert_eq!(app.document_state.tabs.len(), 2);
    assert_eq!(active_path(&app), held());
    assert!(app.document_state.pending_opens.is_empty());
}

#[test]
fn opening_a_case_variant_opens_two_tabs_when_the_directory_preserves_case() {
    let mut app = app_holding_loose_lib();
    request_open(&mut app, variant());

    probed(&mut app, CaseProbeOutcome::Decided(false));
    assert!(
        app.document_state.pending_opens.contains(&variant()),
        "the open went ahead: its read is in flight"
    );
    finish_open(&mut app, variant());

    assert_eq!(app.document_state.tabs.len(), 3, "a second tab for lib");
    assert_eq!(active_path(&app), variant());
}
