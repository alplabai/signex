//! #2896 and #2612: `open_gerber_viewer` from the palette (the GUI path) and
//! from a headless run, and the Messages Source of what it reads.

use std::path::{Path, PathBuf};

use serde_json::Value;
use signex_i18n::{RenderTranslatable, ids};

use super::GerberViewerMsg;
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::{Message, Signex};
use crate::diagnostics::DiagnosticLevel;
use crate::keymap::AppCommandId;
use crate::panels::MessageSource;

fn command() -> AppCommandId {
    AppCommandId::new("open_gerber_viewer").expect("a catalog id")
}

/// A Gerber copper image of `function` with one flash.
fn copper(function: &str) -> String {
    format!(
        "%FSLAX46Y46*%\n%MOMM*%\n%TF.FileFunction,{function}*%\n%ADD10C,1.000000*%\nD10*\nX0Y0D03*\nM02*\n"
    )
}

/// A package folder: two copper layers, and a file that is not
/// fabrication data whose name carries `marker`.
fn package_folder(marker: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a temp dir");
    std::fs::write(dir.path().join("pkg-L1.gbr"), copper("Copper,L1,Top")).expect("write");
    std::fs::write(dir.path().join("pkg-L2.gbr"), copper("Copper,L2,Bot")).expect("write");
    std::fs::write(dir.path().join(format!("notes-{marker}.txt")), "x").expect("write");
    dir
}

fn with_path(path: &Path) -> CommandArgs {
    let operand = format!("path={}", path.display());
    CommandArgs::parse_operands(&command(), &[operand.as_str()]).expect("operands")
}

fn answer(outcome: CommandOutcome) -> Value {
    match outcome {
        CommandOutcome::Answered(value) => value,
        other => panic!("expected an answer, got {other:?}"),
    }
}

fn texts(value: &Value) -> Vec<String> {
    value
        .as_array()
        .expect("a list")
        .iter()
        .map(|item| item.as_str().expect("text").to_string())
        .collect()
}

fn viewer_tabs(app: &Signex) -> usize {
    app.document_state
        .tabs
        .iter()
        .filter(|tab| tab.kind.is_gerber_viewer())
        .count()
}

/// The GUI path: the palette (a key, any surface without operands) runs the
/// bridge arm, which opens the empty viewer once and focuses it after.
#[test]
fn the_palette_opens_the_empty_viewer_and_focuses_it_after() {
    let command = command();
    assert!(crate::app::command::bridge::is_dispatchable(&command));
    assert!(matches!(
        crate::app::command::bridge::core_to_message(&command, &CommandArgs::none()),
        Some(Message::GerberViewer(GerberViewerMsg::ViewerRequested))
    ));
    let (mut app, _task) = Signex::new();
    let (outcome, _task) = app.dispatch_command_reporting(&command, CommandArgs::none(), None);
    assert_eq!(outcome, CommandOutcome::Ran);
    let idx = app.gerber_viewer_tab().expect("the tab opened");
    assert_eq!(app.document_state.active_tab, idx);
    assert!(
        app.document_view.gerber_viewer.open.is_none(),
        "the empty state"
    );
    let (outcome, _task) = app.dispatch_command_reporting(&command, CommandArgs::none(), None);
    assert_eq!(outcome, CommandOutcome::Ran);
    assert_eq!(viewer_tabs(&app), 1, "one Gerber Viewer tab");
}

/// The headless path without `path`: the tab opens and the answer says it
/// is empty.
#[test]
fn a_headless_call_without_a_path_answers_with_the_empty_viewer() {
    let (mut app, _task) = Signex::new();
    let (outcome, _task) = app.run_headless_command_in_test(&command(), CommandArgs::none());
    let answer = answer(outcome);
    assert_eq!(answer["tab"], "gerber_viewer");
    assert_eq!(answer["reading"], false);
    for list in ["paths", "layers", "not_read", "problems"] {
        assert_eq!(answer[list], Value::Array(Vec::new()), "{list}");
    }
    assert_eq!(viewer_tabs(&app), 1);
}

/// The headless path with `path`: the folder opens as one package and the
/// call is answered once the read landed, with the layers top first, the
/// file not read and its problem worded as its Messages row; that row has
/// the Gerber Viewer Source (#2612).
#[test]
fn a_headless_call_with_a_folder_answers_once_the_package_is_read() {
    // Installs the logger first.
    let _ = crate::diagnostics::said_on_this_thread();
    let marker = signex_types::object_id::short_id(&uuid::Uuid::now_v7());
    let dir = package_folder(&marker);
    let not_read = format!("notes-{marker}.txt");
    let mark = crate::diagnostics::newest_entry_id();
    let (mut app, _task) = Signex::new();
    let (outcome, _task) = app.run_headless_command_in_test(&command(), with_path(dir.path()));
    let answer = answer(outcome);

    assert_eq!(answer["reading"], false, "the read landed first");
    assert_eq!(texts(&answer["paths"]), [dir.path().display().to_string()]);
    assert_eq!(texts(&answer["layers"]), ["pkg-L1.gbr", "pkg-L2.gbr"]);
    assert_eq!(texts(&answer["not_read"]), std::slice::from_ref(&not_read));
    let problems = answer["problems"].as_array().expect("problems");
    assert_eq!(problems.len(), 1, "{problems:#?}");
    assert_eq!(problems[0]["severity"], "info");
    assert_eq!(problems[0]["file"], not_read.as_str());
    assert!(problems[0]["line"].is_u64(), "{problems:#?}");
    let message = problems[0]["message"].as_str().expect("text");
    assert!(message.contains(&not_read), "{message}");
    let viewer = &app.document_view.gerber_viewer;
    assert!(viewer.open.is_some() && viewer.reading.is_none());
    assert_eq!(viewer.recent.entries()[0].paths, [dir.path().to_path_buf()]);

    if crate::diagnostics::configured_level() < log::LevelFilter::Info {
        eprintln!("skipped the Messages half: the log filter drops information");
        return;
    }
    let rows: Vec<_> = crate::diagnostics::entries_since(mark)
        .entries
        .into_iter()
        .filter(|entry| entry.message.contains(&not_read))
        .collect();
    assert_eq!(rows.len(), 1, "one Messages row: {rows:#?}");
    assert_eq!(rows[0].level, DiagnosticLevel::Info);
    assert_eq!(
        MessageSource::of_code(&rows[0].code),
        MessageSource::GerberViewer
    );
}

/// A package that cannot be read refuses the call with the Messages
/// sentence, which names the package; the row has the Gerber Viewer Source.
#[test]
fn a_package_that_cannot_be_read_refuses_the_call_and_names_it() {
    let _ = crate::diagnostics::said_on_this_thread();
    let marker = signex_types::object_id::short_id(&uuid::Uuid::now_v7());
    let dir = tempfile::tempdir().expect("a temp dir");
    let gone = format!("gone-{marker}.gbr");
    let mark = crate::diagnostics::newest_entry_id();
    let (mut app, _task) = Signex::new();
    let args = with_path(&dir.path().join(&gone));
    let (outcome, _task) = app.run_headless_command_in_test(&command(), args);
    let CommandOutcome::Disabled { reason } = outcome else {
        panic!("expected a refusal, got {outcome:?}");
    };
    assert_eq!(reason.id(), Some(ids::GERBER_VIEWER_READ_FAILED));
    let text = reason.render_en_us();
    assert!(
        text.starts_with(&format!("Gerber Viewer: {gone}: ")),
        "{text}"
    );
    assert!(app.document_view.gerber_viewer.open.is_none());

    if crate::diagnostics::configured_level() < log::LevelFilter::Error {
        eprintln!("skipped the Messages half: the log filter drops errors");
        return;
    }
    let rows: Vec<_> = crate::diagnostics::entries_since(mark)
        .entries
        .into_iter()
        .filter(|entry| {
            entry.message.starts_with("Gerber Viewer:") && entry.message.contains(&gone)
        })
        .collect();
    assert_eq!(rows.len(), 1, "one Messages row: {rows:#?}");
    assert_eq!(rows[0].level, DiagnosticLevel::Error);
    assert_eq!(
        MessageSource::of_code(&rows[0].code),
        MessageSource::GerberViewer
    );
}

/// A read that a later open replaced still answers its call: refused,
/// naming the package it was for, and nothing of it is shown.
#[test]
fn a_replaced_read_refuses_the_call_that_waited_on_it() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let first = dir.path().join("first.zip");
    let (mut app, _task) = Signex::new();
    let waiting = app.wait_for_command(&command(), &with_path(&first));
    let ticket = waiting.ticket;
    let later = vec![dir.path().join("later.zip")];
    let _ = app.update(Message::from(GerberViewerMsg::PathsOpened(later)));
    let job = app.document_view.gerber_viewer.reading.expect("a read");
    let stale = GerberViewerMsg::PackageRead {
        job: job - 1,
        paths: vec![first],
        result: Err("replaced".into()),
        waiting: Some(waiting),
    };
    let _ = app.update(Message::from(stale));
    match app.take_finished_edit(ticket) {
        Some(CommandOutcome::Disabled { reason }) => {
            assert_eq!(reason.id(), Some(ids::GERBER_VIEWER_READ_SUPERSEDED));
            assert!(reason.render_en_us().contains("first.zip"));
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
    let viewer = &app.document_view.gerber_viewer;
    assert!(viewer.open.is_none());
    assert_eq!(viewer.reading, Some(job), "the later read is still awaited");
}

/// The read a person started answers no command.
#[test]
fn a_persons_open_answers_no_command() {
    let (mut app, _task) = Signex::new();
    let paths = vec![PathBuf::from("/fab/p.zip")];
    let _ = app.update(Message::from(GerberViewerMsg::PathsOpened(paths.clone())));
    let job = app.document_view.gerber_viewer.reading.expect("a read");
    let finished = app.document_state.agent_edits.finished.len();
    let _ = app.update(Message::from(GerberViewerMsg::PackageRead {
        job,
        paths,
        result: Err("gone".into()),
        waiting: None,
    }));
    assert_eq!(app.document_state.agent_edits.finished.len(), finished);
    assert!(app.document_view.gerber_viewer.open.is_none());
}
