//! #3013 — `recover_library`: the Library Missing and Git Repository Missing
//! choices (mockup row 138) on the GUI path (`Signex::update` with the
//! dialog's buttons and its file pick) and the headless path (operands),
//! leaving the same state.

use std::path::{Path, PathBuf};

use signex_i18n::RenderTranslatable;
use signex_types::project::{LibraryEntry, LibraryEntryKind};

use super::{CommandArgs, CommandOutcome};
use crate::app::{Message, Signex};
use crate::io_ports::{HeadlessAnswers, IoPorts};
use crate::keymap::AppCommandId;
use crate::library::messages::LibraryMessage;
use crate::library::recovery::{GitMissingChoice, LibraryMissingChoice, RecoveryDialog};
use crate::task_driver::TaskDriver;

fn settle(app: &mut Signex, task: iced::Task<Message>) {
    TaskDriver::new().settle(app, task).expect("settles");
}

fn send(app: &mut Signex, msg: LibraryMessage) {
    let task = app.update(Message::Library(msg));
    settle(app, task);
}

fn headless(app: &mut Signex, operands: &[&str]) -> CommandOutcome {
    let command = AppCommandId::new("recover_library").expect("a catalog id");
    let args = CommandArgs::parse_operands(&command, operands).expect("operands parse");
    let (outcome, task) = app.run_headless_command_in_test(&command, args);
    settle(app, task);
    outcome
}

fn refusal(outcome: CommandOutcome) -> String {
    match outcome {
        CommandOutcome::Disabled { reason } => reason.render_en_us(),
        other => panic!("not refused: {other:?}"),
    }
}

fn arg(name: &str, path: &Path) -> String {
    format!("{name}={}", path.display())
}

/// An app with the project of `dir` open, listing the library `gone`, which
/// is not on disk, and its Library Missing recovery open, as a failed mount
/// leaves it.
fn app_missing(dir: &Path) -> (Signex, PathBuf) {
    let mut app =
        crate::app::handlers::menu::export::tests::app_workspace(&dir.to_string_lossy(), &[]);
    let gone = dir.join("gone").join("gone.snxlib");
    let project = &mut app.document_state.projects[0];
    project.data.libraries.push(LibraryEntry {
        path: gone.clone(),
        kind: LibraryEntryKind::Shared,
        library_id: None,
        unknown: Default::default(),
    });
    let owner = project.id;
    app.library.raise_missing_library(&gone, owner);
    (app, gone)
}

/// The libraries the project lists, and whether a recovery is still open.
fn left(app: &Signex) -> (Vec<PathBuf>, bool, bool) {
    let project = &app.document_state.projects[0];
    let listed = project
        .data
        .libraries
        .iter()
        .map(|entry| project.data.resolve_library_path(entry))
        .collect();
    (
        listed,
        app.library.recovery.is_some(),
        app.document_state.is_document_dirty(&project.path),
    )
}

#[test]
fn remove_from_project_by_operands_is_the_dialogs_button() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut by_dialog, _) = app_missing(dir.path());
    send(
        &mut by_dialog,
        LibraryMessage::RecoveryLibraryMissing(LibraryMissingChoice::RemoveFromProject),
    );
    let (mut by_command, gone) = app_missing(dir.path());
    let outcome = headless(&mut by_command, &[&arg("path", &gone), "action=remove"]);
    assert!(
        matches!(outcome, CommandOutcome::Answered(_)),
        "{outcome:?}"
    );
    assert_eq!(left(&by_command), left(&by_dialog));
    assert_eq!(left(&by_command), (Vec::new(), false, true));
}

/// Skip for now leaves the library listed and skipped; the command raises a
/// skipped library's recovery again, as the status bar chip does.
#[test]
fn skip_by_operands_is_the_dialogs_close_and_a_skipped_library_is_raised_again() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut by_dialog, _) = app_missing(dir.path());
    send(
        &mut by_dialog,
        LibraryMessage::RecoveryLibraryMissing(LibraryMissingChoice::Cancel),
    );
    let (mut by_command, gone) = app_missing(dir.path());
    let outcome = headless(&mut by_command, &[&arg("path", &gone), "action=skip"]);
    assert!(
        matches!(outcome, CommandOutcome::Answered(_)),
        "{outcome:?}"
    );
    assert_eq!(left(&by_command), left(&by_dialog));
    assert_eq!(by_command.library.missing.skipped(), [gone.clone()]);

    let outcome = headless(&mut by_command, &[&arg("path", &gone), "action=remove"]);
    assert!(
        matches!(outcome, CommandOutcome::Answered(_)),
        "{outcome:?}"
    );
    assert_eq!(left(&by_command), (Vec::new(), false, true));
}

#[test]
fn locate_by_operands_is_the_dialogs_file_pick() {
    let dir = tempfile::tempdir().expect("tempdir");
    let found = dir.path().join("found").join("found.snxlib");
    crate::library::commands::create_library_file(&found, &Default::default()).expect("library");

    let (mut by_dialog, _) = app_missing(dir.path());
    let answers = HeadlessAnswers::default();
    by_dialog.ui_state.io = IoPorts::headless(answers.clone());
    answers.replace_all([("path".to_string(), found.display().to_string())]);
    send(
        &mut by_dialog,
        LibraryMessage::RecoveryLibraryMissing(LibraryMissingChoice::Locate),
    );

    let (mut by_command, gone) = app_missing(dir.path());
    let outcome = headless(
        &mut by_command,
        &[
            &arg("path", &gone),
            "action=locate",
            &arg("located", &found),
        ],
    );
    let CommandOutcome::Answered(answer) = outcome else {
        panic!("not answered: {outcome:?}");
    };
    assert_eq!(answer["located"], found.display().to_string());
    assert_eq!(left(&by_command), left(&by_dialog));
    assert_eq!(left(&by_command), (vec![found.clone()], false, true));
    assert!(by_command.library.library_at(&found).is_some());
}

/// Skip for now on Git Repository Missing opens the library read only; the
/// command does what the button does.
#[test]
fn read_only_by_operands_is_git_missings_skip() {
    let dir = tempfile::tempdir().expect("tempdir");
    let lib = dir.path().join("lib").join("lib.snxlib");
    crate::library::commands::create_library_file(&lib, &Default::default()).expect("library");
    let git_missing = || RecoveryDialog::GitMissing {
        path: lib.clone(),
        remote: None,
        error: None,
    };
    let (mut by_dialog, _boot) = Signex::new();
    by_dialog.library.recovery = Some(git_missing());
    send(
        &mut by_dialog,
        LibraryMessage::RecoveryGitMissing(GitMissingChoice::Skip),
    );
    let (mut by_command, _boot) = Signex::new();
    by_command.library.recovery = Some(git_missing());
    let outcome = headless(&mut by_command, &[&arg("path", &lib), "action=read_only"]);
    assert!(
        matches!(outcome, CommandOutcome::Answered(_)),
        "{outcome:?}"
    );
    let state = |app: &Signex| {
        (
            app.library.recovery.is_some(),
            app.library.library_at(&lib).is_some(),
            app.library.is_read_only(&lib),
        )
    };
    assert_eq!(state(&by_command), state(&by_dialog));
    assert!(!state(&by_command).0, "the recovery is answered");
}

#[test]
fn what_no_recovery_asks_is_refused_with_nothing_changed() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, gone) = app_missing(dir.path());
    let other = dir.path().join("other.snxlib");
    let path = arg("path", &gone);
    let elsewhere = arg("path", &other);
    for (operands, needle) in [
        (vec![path.as_str(), "action=fix"], "`action` is one of"),
        (
            vec![path.as_str(), "action=locate"],
            "`locate` needs the operand `located`",
        ),
        (vec![elsewhere.as_str(), "action=remove"], "The recovery of"),
        (
            vec![path.as_str(), "action=reinit"],
            "No Git Repository Missing recovery is open",
        ),
    ] {
        let reason = refusal(headless(&mut app, &operands));
        assert!(reason.contains(needle), "{operands:?}: {reason}");
    }
    assert_eq!(left(&app), (vec![gone.clone()], true, false));

    // With no recovery open, a library nobody skipped is not a missing one.
    app.library.finish_recovery();
    let reason = refusal(headless(&mut app, &[&elsewhere, "action=remove"]));
    assert!(reason.contains("is not a missing library"), "{reason}");
}
