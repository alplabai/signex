//! #2155: `open_document` and `new_document` run their `path` operand through
//! their operand handlers, and the result is what the menu entry and its dialog
//! produce; a refusal reaches the Messages log and the error notice.

use std::path::{Path, PathBuf};

use super::{CommandArgs, CommandOutcome};
use crate::app::{Message, Signex};
use crate::diagnostics::{entries_since, init_logging, newest_entry_id};
use crate::io_ports::{HeadlessAnswers, IoPorts};
use crate::keymap::AppCommandId;
use crate::menu_bar::MenuMessage;
use crate::task_driver::TaskDriver;

/// The tests share one log and one headless picker, so they take turns.
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

/// Where the log stands now; what is logged after it is `messages_since`.
fn log_mark() -> Option<u64> {
    let _ = init_logging();
    newest_entry_id()
}

/// The log lines after `mark` that name `needle` (a path or an extension
/// unique to the test, so a test in another module cannot satisfy it).
fn messages_since(mark: Option<u64>, needle: &str) -> Vec<String> {
    entries_since(mark)
        .entries
        .into_iter()
        .map(|entry| entry.message)
        .filter(|message| message.contains(needle))
        .collect()
}

fn notice(app: &Signex) -> Option<(String, String)> {
    app.document_state
        .error_notice
        .as_ref()
        .map(|n| (n.title.clone(), n.detail.clone()))
}

/// Run `name path=<path> [extra..]` as an in-process request does, and answer
/// as a headless run does.
fn run_with_operand(app: &mut Signex, name: &str, path: &Path, extra: &[&str]) -> CommandOutcome {
    let id = command(name);
    let mut operands = vec![format!("path={}", path.display())];
    operands.extend(extra.iter().map(|e| e.to_string()));
    let refs: Vec<&str> = operands.iter().map(String::as_str).collect();
    let args = CommandArgs::parse_operands(&id, &refs).expect("operands");
    let snapshot = app.open_snapshot(&args);
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
    let outcome = run_with_operand(&mut by_operand, "open_document", &sheet, &[]);

    let CommandOutcome::Answered(answer) = outcome else {
        panic!("open_document did not answer: {outcome:?}");
    };
    assert_eq!(answer["kind"], "schematic", "{answer}");
    assert_eq!(answer["path"], sheet.display().to_string());
    let (docs, active) = open_set(&by_operand);
    assert!(docs.iter().any(|(path, _)| path.as_ref() == Some(&sheet)));
    assert_eq!(open_set(&by_dialog), (docs, active));
}

/// A dialog answer left over from an earlier command never names the document
/// an operand run opened.
#[test]
fn the_path_operand_wins_over_a_leftover_dialog_answer() {
    let _serial = serial();
    let dir = dir();
    let (a, b) = (dir.path().join("a.snxsch"), dir.path().join("b.snxsch"));
    write_schematic(&a);
    write_schematic(&b);
    let (mut app, answers) = headless_app();
    answers.replace_all([("path".to_string(), a.display().to_string())]);

    let outcome = run_with_operand(&mut app, "open_document", &b, &[]);

    let CommandOutcome::Answered(answer) = outcome else {
        panic!("open_document did not answer: {outcome:?}");
    };
    assert_eq!(answer["path"], b.display().to_string(), "{answer}");
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
    let outcome = run_with_operand(&mut by_operand, "new_document", &project, &[]);

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

/// A file that is not there fails on the read worker, not in the handler: the
/// handler says `Ran`, and the run is refused because no document appeared.
#[test]
fn a_missing_file_ran_the_open_but_made_no_document_and_the_read_failure_is_logged() {
    let _serial = serial();
    let dir = dir();
    let gone = dir.path().join("gone.snxsch");
    let (mut app, _) = headless_app();
    let mark = log_mark();

    let outcome = run_with_operand(&mut app, "open_document", &gone, &[]);

    assert!(
        matches!(&outcome, CommandOutcome::Disabled { reason } if reason.contains("made no document")),
        "{outcome:?}"
    );
    let named = gone.display().to_string();
    assert!(
        !messages_since(mark, &named).is_empty(),
        "the read failure is logged"
    );
}

/// The handler's own refusal: the reason, the log line and the Open Failed
/// notice all name the file, and they are what the dialog route leaves.
#[test]
fn an_unsupported_file_is_refused_with_its_reason_in_the_log_and_the_notice() {
    let _serial = serial();
    let dir = dir();
    let file = dir.path().join("x.zq2155");
    let needle = "zq2155";

    let (mut by_dialog, answers) = headless_app();
    let dialog_mark = log_mark();
    run_with_dialog(&mut by_dialog, &answers, MenuMessage::OpenProject, &file);
    let dialog_log = messages_since(dialog_mark, needle);

    let (mut app, _) = headless_app();
    let id = command("open_document");
    let operand = format!("path={}", file.display());
    let args = CommandArgs::parse_operands(&id, &[operand.as_str()]).expect("operands");
    let mark = log_mark();
    let (outcome, _) = app.run_operands(&id, &args);

    let CommandOutcome::Disabled { reason } = outcome else {
        panic!("not refused: {outcome:?}");
    };
    assert!(
        reason.contains("unsupported file type: .zq2155"),
        "{reason}"
    );
    let log = messages_since(mark, needle);
    assert!(
        log.iter()
            .any(|m| m.contains("Failed to open document path")),
        "logged: {log:?}"
    );
    let (title, detail) = notice(&app).expect("the Open Failed notice");
    assert!(detail.contains(needle), "{title}: {detail}");
    assert_eq!(log, dialog_log, "the Messages entries are the menu's");
    assert_eq!(notice(&app), notice(&by_dialog), "the notice is the menu's");
}

#[test]
fn a_second_new_project_at_one_path_is_refused_and_logged_whatever_overwrite_says() {
    let _serial = serial();
    let dir = dir();
    let project = dir.path().join("p.snxprj");
    let (mut app, _) = headless_app();
    assert!(matches!(
        run_with_operand(&mut app, "new_document", &project, &[]),
        CommandOutcome::Answered(_)
    ));
    let mark = log_mark();

    let outcome = run_with_operand(&mut app, "new_document", &project, &["overwrite=true"]);

    assert!(
        matches!(outcome, CommandOutcome::Disabled { .. }),
        "{outcome:?}"
    );
    let named = dir.path().display().to_string();
    let log = messages_since(mark, &named);
    assert!(
        log.iter()
            .any(|m| m.contains("Failed to create new project")),
        "logged: {log:?}"
    );
}

#[test]
fn a_new_project_path_that_is_not_a_project_file_is_refused_with_its_reason() {
    let _serial = serial();
    let dir = dir();
    let file = dir.path().join("p.txt");
    let (mut app, _) = headless_app();
    let id = command("new_document");
    let operand = format!("path={}", file.display());
    let args = CommandArgs::parse_operands(&id, &[operand.as_str()]).expect("operands");
    let mark = log_mark();

    let (outcome, _) = app.run_operands(&id, &args);

    let CommandOutcome::Disabled { reason } = outcome else {
        panic!("not refused: {outcome:?}");
    };
    assert!(reason.contains("must end in .snxprj"), "{reason}");
    let log = messages_since(mark, &file.display().to_string());
    assert!(
        log.iter()
            .any(|m| m.contains("Failed to create new project")),
        "logged: {log:?}"
    );
    let (_, detail) = notice(&app).expect("the notice");
    assert!(detail.contains(&file.display().to_string()), "{detail}");
}
