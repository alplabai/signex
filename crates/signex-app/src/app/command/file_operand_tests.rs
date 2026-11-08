//! #2155: `open_document` and `new_document` run their `path` operand through
//! their operand handlers, and the result is what the menu entry and its dialog
//! produce; a refusal reaches the Messages log.

use std::path::{Path, PathBuf};

use super::{CommandArgs, CommandOutcome};
use crate::app::{Message, Signex};
use crate::diagnostics::logged_count;
use crate::io_ports::{HeadlessAnswers, IoPorts};
use crate::keymap::AppCommandId;
use crate::menu_bar::MenuMessage;
use crate::task_driver::TaskDriver;

/// The refusals are counted in the shared log, so these tests take turns.
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn headless_app() -> (Signex, HeadlessAnswers) {
    let answers = HeadlessAnswers::default();
    let (mut app, _boot) = Signex::new();
    app.ui_state.io = IoPorts::headless(answers.clone());
    (app, answers)
}

fn command(name: &str) -> AppCommandId {
    AppCommandId::new(name).expect("a valid id")
}

fn settle(app: &mut Signex, task: iced::Task<Message>) {
    TaskDriver::new().settle(app, task).expect("settles");
}

/// Run `name path=<path>` as an in-process request does, to its end.
fn run_with_operand(app: &mut Signex, name: &str, path: &Path) -> CommandOutcome {
    let id = command(name);
    let operand = format!("path={}", path.display());
    let args = CommandArgs::parse_operands(&id, &[operand.as_str()]).expect("operands");
    let snapshot = app.open_snapshot();
    let (outcome, task) = app.run_operands(&id, &args);
    match outcome {
        CommandOutcome::Ran => app.settle_and_answer_new_document(&id, snapshot, task).0,
        other => other,
    }
}

/// The dialog route: the menu entry with `path` answered.
fn run_with_dialog(app: &mut Signex, answers: &HeadlessAnswers, menu: MenuMessage, path: &Path) {
    answers.replace_all([("path".to_string(), path.display().to_string())]);
    let task = app.update(Message::Menu(menu));
    settle(app, task);
}

fn write_schematic(path: &Path) {
    let text = signex_types::format::SnxSchematic::new(signex_types::schematic::SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
        ..Default::default()
    })
    .write_string()
    .expect("encode");
    std::fs::write(path, text).expect("write schematic");
}

/// What a surface leaves open: each document's path and kind, and the tab.
fn open_set(app: &Signex) -> (Vec<(Option<PathBuf>, String)>, Option<PathBuf>) {
    let mut docs: Vec<_> = app
        .document_entries()
        .into_iter()
        .map(|entry| (entry.path, entry.kind.as_str().to_string()))
        .collect();
    docs.sort();
    (docs, app.document_state.active_path.clone())
}

fn dir() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("snx-2155-file")
        .tempdir()
        .expect("temp dir")
}

fn serial() -> std::sync::MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(|e| e.into_inner())
}

#[test]
fn open_document_with_a_path_opens_what_the_dialog_route_opens() {
    let _serial = serial();
    let dir = dir();
    let sheet = dir.path().join("a.snxsch");
    write_schematic(&sheet);

    let (mut by_dialog, answers) = headless_app();
    run_with_dialog(&mut by_dialog, &answers, MenuMessage::OpenProject, &sheet);
    let (mut by_operand, _) = headless_app();
    let outcome = run_with_operand(&mut by_operand, "open_document", &sheet);

    let CommandOutcome::Answered(answer) = outcome else {
        panic!("open_document did not answer: {outcome:?}");
    };
    assert_eq!(answer["kind"], "schematic", "{answer}");
    assert_eq!(answer["path"], sheet.display().to_string());
    let (docs, active) = open_set(&by_operand);
    assert!(docs.iter().any(|(path, _)| path.as_ref() == Some(&sheet)));
    assert_eq!(open_set(&by_dialog), (docs, active));
}

#[test]
fn new_document_with_a_path_creates_what_the_dialog_route_creates() {
    let _serial = serial();
    let dir = dir();
    let by_dialog_dir = dir.path().join("dialog");
    let by_operand_dir = dir.path().join("operand");
    for sub in [&by_dialog_dir, &by_operand_dir] {
        std::fs::create_dir(sub).expect("dir");
    }

    let (mut by_dialog, answers) = headless_app();
    let dialog_project = by_dialog_dir.join("p.snxprj");
    run_with_dialog(
        &mut by_dialog,
        &answers,
        MenuMessage::NewProject,
        &dialog_project,
    );
    let (mut by_operand, _) = headless_app();
    let project = by_operand_dir.join("p.snxprj");
    let outcome = run_with_operand(&mut by_operand, "new_document", &project);

    let CommandOutcome::Answered(answer) = outcome else {
        panic!("new_document did not answer: {outcome:?}");
    };
    assert_eq!(answer["kind"], "project", "{answer}");
    assert!(by_operand_dir.join("p/p.snxprj").is_file(), "the .snxprj");
    assert!(
        by_operand_dir.join("p/p.snxsch").is_file(),
        "the companion sheet"
    );
    let names = |app: &Signex, root: &Path| -> Vec<_> {
        open_set(app)
            .0
            .into_iter()
            .map(|(path, kind)| (path.map(|p| p.strip_prefix(root).unwrap().to_owned()), kind))
            .collect()
    };
    assert_eq!(
        names(&by_operand, &by_operand_dir),
        names(&by_dialog, &by_dialog_dir)
    );
}

#[test]
fn opening_a_missing_file_is_refused_and_logged() {
    let _serial = serial();
    let dir = dir();
    let (mut app, _) = headless_app();
    let before = logged_count("Failed to open document path");

    let outcome = run_with_operand(&mut app, "open_document", &dir.path().join("gone.snxsch"));

    assert!(
        matches!(outcome, CommandOutcome::Disabled { .. }),
        "{outcome:?}"
    );
    assert!(
        logged_count("Failed to open document path") > before,
        "logged"
    );
    assert!(app.document_state.error_notice.is_some(), "notice raised");
}

#[test]
fn opening_an_unsupported_file_is_refused_with_its_reason() {
    let _serial = serial();
    let dir = dir();
    let (mut app, _) = headless_app();
    let id = command("open_document");
    let operand = format!("path={}", dir.path().join("x.txt").display());
    let args = CommandArgs::parse_operands(&id, &[operand.as_str()]).expect("operands");
    let before = logged_count("Failed to open document path");

    let (outcome, _) = app.run_operands(&id, &args);

    let CommandOutcome::Disabled { reason } = outcome else {
        panic!("not refused: {outcome:?}");
    };
    assert!(reason.contains("unsupported file type"), "{reason}");
    assert!(
        logged_count("Failed to open document path") > before,
        "logged"
    );
}

#[test]
fn a_second_new_project_at_one_path_is_refused_and_logged() {
    let _serial = serial();
    let dir = dir();
    let project = dir.path().join("p.snxprj");
    let (mut app, _) = headless_app();
    assert!(matches!(
        run_with_operand(&mut app, "new_document", &project),
        CommandOutcome::Answered(_)
    ));
    let before = logged_count("Failed to create new project");

    let outcome = run_with_operand(&mut app, "new_document", &project);

    assert!(
        matches!(outcome, CommandOutcome::Disabled { .. }),
        "{outcome:?}"
    );
    assert!(
        logged_count("Failed to create new project") > before,
        "logged"
    );
}

#[test]
fn a_new_project_path_that_is_not_a_project_file_is_refused_with_its_reason() {
    let _serial = serial();
    let dir = dir();
    let (mut app, _) = headless_app();
    let id = command("new_document");
    let operand = format!("path={}", dir.path().join("p.txt").display());
    let args = CommandArgs::parse_operands(&id, &[operand.as_str()]).expect("operands");
    let before = logged_count("Failed to create new project");

    let (outcome, _) = app.run_operands(&id, &args);

    let CommandOutcome::Disabled { reason } = outcome else {
        panic!("not refused: {outcome:?}");
    };
    assert!(reason.contains("must end in .snxprj"), "{reason}");
    assert!(
        logged_count("Failed to create new project") > before,
        "logged"
    );
}

#[test]
fn new_document_takes_no_overwrite_operand() {
    let id = command("new_document");
    let err = CommandArgs::parse_operands(&id, &["path=/tmp/p.snxprj", "overwrite=true"]);
    assert!(err.is_err(), "overwrite means nothing to a new project");
}
