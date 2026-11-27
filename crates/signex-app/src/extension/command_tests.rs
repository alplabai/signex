//! Extension catalog commands (#2495): registration checks, and a command run
//! through the shared dispatch path from a window and headless.

use std::sync::{Arc, Mutex};

use iced::Task;
use signex_commands::catalog::{CommandFlags, CommandMetadata, Operand, OperandKind};
use signex_i18n_ids::{Translatable, UserText};

use super::{Extension, ExtensionHost, ExtensionMsg, ExtensionPoints, InstallError, RegisterError};
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;

/// The payload the sample handlers send: which command ran.
#[derive(Debug, Clone, PartialEq)]
struct CommandRan(&'static str);

/// A row with the given id and flags and nothing else.
const fn row(id: &'static str, flags: CommandFlags) -> CommandMetadata {
    CommandMetadata {
        id,
        category: "sample",
        flags,
        ..CommandMetadata::DEFAULT
    }
}

const SAY_HELLO: CommandMetadata = row("ext.sample.say_hello", CommandFlags::RUNS_HEADLESS);
const SHOW_NOTES: CommandMetadata = row("ext.sample.show_notes", CommandFlags::GUI_ONLY);
const REFUSE: CommandMetadata = row("ext.sample.refuse", CommandFlags::RUNS_HEADLESS);

/// A sample extension that adds [`SAY_HELLO`], [`SHOW_NOTES`] and [`REFUSE`]
/// and records every `CommandRan` its `update` receives.
struct Sample {
    ran: Arc<Mutex<Vec<&'static str>>>,
}

impl Extension for Sample {
    fn name(&self) -> &str {
        "sample"
    }

    fn register(&mut self, points: &mut ExtensionPoints) {
        let commands = &mut points.commands;
        let added = [
            commands.add(&SAY_HELLO, |_| Ok(CommandRan("say_hello"))),
            commands.add(&SHOW_NOTES, |_| Ok(CommandRan("show_notes"))),
            commands.add(&REFUSE, |_| {
                Err::<CommandRan, _>(Translatable::verbatim(UserText::new("nothing to refuse")))
            }),
        ];
        assert!(added.iter().all(Result::is_ok), "{added:?}");
    }

    fn update(&mut self, message: ExtensionMsg) -> Task<Message> {
        if let Some(CommandRan(name)) = message.payload::<CommandRan>() {
            self.ran.lock().expect("ran").push(*name);
        }
        Task::none()
    }
}

fn app_with_sample() -> (Signex, Arc<Mutex<Vec<&'static str>>>) {
    let ran = Arc::new(Mutex::new(Vec::new()));
    let sample = Sample {
        ran: Arc::clone(&ran),
    };
    let host = ExtensionHost::install(vec![Box::new(sample)]).expect("install");
    let (app, _boot) = Signex::new();
    (app.with_extensions(host), ran)
}

fn id(text: &str) -> AppCommandId {
    AppCommandId::new(text).expect("command id")
}

fn ran(log: &Arc<Mutex<Vec<&'static str>>>) -> Vec<&'static str> {
    log.lock().expect("ran").clone()
}

#[test]
fn an_extension_command_runs_from_a_window_through_dispatch_command() {
    let (mut app, log) = app_with_sample();
    for command in ["ext.sample.say_hello", "ext.sample.show_notes"] {
        let (outcome, _task) =
            app.dispatch_command_reporting(&id(command), CommandArgs::none(), None);
        assert_eq!(outcome, CommandOutcome::Ran, "{command}");
    }
    assert_eq!(ran(&log), ["say_hello", "show_notes"]);

    // The keymap, palette and menu entry point runs it the same way.
    let _task = app.dispatch_command(&id("ext.sample.say_hello"), CommandArgs::none(), None);
    assert_eq!(ran(&log), ["say_hello", "show_notes", "say_hello"]);
}

#[test]
fn a_headless_run_runs_a_core_command_and_refuses_a_gui_only_one() {
    let (mut app, log) = app_with_sample();
    let (outcome, _task) =
        app.run_headless_command_in_test(&id("ext.sample.say_hello"), CommandArgs::none());
    assert_eq!(outcome, CommandOutcome::Ran);
    assert_eq!(ran(&log), ["say_hello"]);

    let (outcome, _task) =
        app.run_headless_command_in_test(&id("ext.sample.show_notes"), CommandArgs::none());
    assert_eq!(
        outcome,
        CommandOutcome::disabled("`ext.sample.show_notes` only works in a window")
    );
    assert_eq!(ran(&log), ["say_hello"], "a refused command reached update");
}

#[test]
fn a_handler_refusal_is_reported_and_update_is_not_called() {
    let (mut app, log) = app_with_sample();
    let reason = Translatable::verbatim(UserText::new("nothing to refuse"));
    let (outcome, _task) =
        app.dispatch_command_reporting(&id("ext.sample.refuse"), CommandArgs::none(), None);
    assert_eq!(
        outcome,
        CommandOutcome::Disabled {
            reason: reason.clone()
        }
    );
    let (outcome, _task) =
        app.run_headless_command_in_test(&id("ext.sample.refuse"), CommandArgs::none());
    assert_eq!(outcome, CommandOutcome::Disabled { reason });
    assert!(ran(&log).is_empty());
}

/// The Community model has no extension command: an `ext.` id is unknown
/// there, as any id the catalog does not have.
#[test]
fn an_extension_id_nobody_installed_is_unknown() {
    let (mut app, _boot) = Signex::new();
    let command = id("ext.sample.say_hello");
    assert!(app.extensions().commands().next().is_none());
    assert_eq!(app.command_metadata(&command), None);
    let (outcome, _task) = app.dispatch_command_reporting(&command, CommandArgs::none(), None);
    assert_eq!(outcome, CommandOutcome::UnknownId);

    let (mut app, log) = app_with_sample();
    let (outcome, _task) =
        app.dispatch_command_reporting(&id("ext.sample.missing"), CommandArgs::none(), None);
    assert_eq!(outcome, CommandOutcome::UnknownId);
    assert!(ran(&log).is_empty());
}

#[test]
fn command_metadata_finds_built_in_and_installed_rows() {
    let (app, _log) = app_with_sample();
    assert_eq!(
        app.command_metadata(&id("undo")).map(|m| m.id),
        Some("undo")
    );
    assert_eq!(
        app.command_metadata(&id("ext.sample.say_hello")),
        Some(&SAY_HELLO)
    );
    let ids: Vec<&str> = app
        .extensions()
        .commands()
        .map(|command| command.metadata().id)
        .collect();
    assert_eq!(
        ids,
        [
            "ext.sample.say_hello",
            "ext.sample.show_notes",
            "ext.sample.refuse"
        ]
    );
}

/// An extension that offers one row and keeps what `add` answered.
struct Offers {
    rows: &'static [&'static CommandMetadata],
    answers: Arc<Mutex<Vec<Result<(), RegisterError>>>>,
}

impl Extension for Offers {
    fn name(&self) -> &str {
        "sample"
    }

    fn register(&mut self, points: &mut ExtensionPoints) {
        for &row in self.rows {
            let answer = points.commands.add(row, |_| Ok(CommandRan("offered")));
            self.answers.lock().expect("answers").push(answer);
        }
    }
}

/// What `add` answered for `rows`, and what install made of it.
fn offer(
    rows: &'static [&'static CommandMetadata],
) -> (
    Vec<Result<(), RegisterError>>,
    Result<ExtensionHost, InstallError>,
) {
    let answers = Arc::new(Mutex::new(Vec::new()));
    let offers = Offers {
        rows,
        answers: Arc::clone(&answers),
    };
    let installed = ExtensionHost::install(vec![Box::new(offers)]);
    let answers = answers.lock().expect("answers").clone();
    (answers, installed)
}

/// The first refusal `add` gave for `rows`; install refuses with it too.
fn refusal(rows: &'static [&'static CommandMetadata]) -> RegisterError {
    let (answers, installed) = offer(rows);
    let error = answers
        .into_iter()
        .find_map(Result::err)
        .expect("a refusal");
    assert_eq!(
        installed.err(),
        Some(InstallError::Refused {
            extension: "sample".to_owned(),
            error: error.clone(),
        })
    );
    error
}

const BUILT_IN_UNDO: CommandMetadata = row("undo", CommandFlags::RUNS_HEADLESS);
const LOCAL_UNDO: CommandMetadata = row("ext.sample.undo", CommandFlags::RUNS_HEADLESS);
const NO_PREFIX: CommandMetadata = row("say_hello", CommandFlags::RUNS_HEADLESS);
const OTHER_PREFIX: CommandMetadata = row("ext.other.say_hello", CommandFlags::RUNS_HEADLESS);
const BAD_LOCAL: CommandMetadata = row("ext.sample.Say", CommandFlags::RUNS_HEADLESS);
const TWO_SEGMENTS: CommandMetadata = row("ext.sample.say.hello", CommandFlags::RUNS_HEADLESS);
const UNDECIDED: CommandMetadata = row("ext.sample.undecided", CommandFlags::NONE);
const BOTH: CommandMetadata = row(
    "ext.sample.both",
    CommandFlags {
        runs_headless: true,
        ..CommandFlags::GUI_ONLY
    },
);
const MUTATES: CommandMetadata = row("ext.sample.mutates", CommandFlags::MUTATES);
const WITH_OPERAND: CommandMetadata = CommandMetadata {
    operands: &[Operand {
        name: "text",
        kind: OperandKind::Text,
        required: true,
    }],
    ..row("ext.sample.with_operand", CommandFlags::RUNS_HEADLESS)
};

#[test]
fn an_id_that_is_or_reads_as_a_built_in_is_refused() {
    use RegisterError::ShadowsBuiltIn;
    assert_eq!(refusal(&[&BUILT_IN_UNDO]), ShadowsBuiltIn("undo".into()));
    assert_eq!(
        refusal(&[&LOCAL_UNDO]),
        ShadowsBuiltIn("ext.sample.undo".into())
    );
}

#[test]
fn an_id_outside_the_owners_prefix_is_refused() {
    let outside = |id: &str| RegisterError::NotOwned(id.into());
    assert_eq!(refusal(&[&NO_PREFIX]), outside("say_hello"));
    assert_eq!(refusal(&[&OTHER_PREFIX]), outside("ext.other.say_hello"));
    assert_eq!(
        refusal(&[&BAD_LOCAL]),
        RegisterError::InvalidId("ext.sample.Say".into())
    );
    // Orchestrator decision 2026-11-25: one segment after the name.
    assert_eq!(
        refusal(&[&TWO_SEGMENTS]),
        RegisterError::InvalidId("ext.sample.say.hello".into())
    );
}

#[test]
fn a_duplicate_id_is_refused_and_the_first_row_kept() {
    let (answers, installed) = offer(&[&SAY_HELLO, &SAY_HELLO]);
    let duplicate = RegisterError::Duplicate("ext.sample.say_hello".into());
    assert_eq!(answers, [Ok(()), Err(duplicate.clone())]);
    assert_eq!(
        installed.err(),
        Some(InstallError::Refused {
            extension: "sample".into(),
            error: duplicate,
        })
    );
}

#[test]
fn a_row_must_decide_headless_and_take_no_operands_or_document() {
    use RegisterError as Error;
    assert_eq!(
        refusal(&[&UNDECIDED]),
        Error::HeadlessUndecided("ext.sample.undecided".into())
    );
    assert_eq!(
        refusal(&[&BOTH]),
        Error::HeadlessUndecided("ext.sample.both".into())
    );
    assert_eq!(
        refusal(&[&MUTATES]),
        Error::MutatesDocument("ext.sample.mutates".into())
    );
    assert_eq!(
        refusal(&[&WITH_OPERAND]),
        Error::TakesOperands("ext.sample.with_operand".into())
    );
}

#[test]
fn accepted_rows_install() {
    let (answers, installed) = offer(&[&SAY_HELLO, &SHOW_NOTES]);
    assert_eq!(answers, [Ok(()), Ok(())]);
    let host = installed.expect("install");
    assert!(host.command("ext.sample.say_hello").is_some());
    assert!(host.command("ext.sample.show_notes").is_some());
    assert!(host.command("ext.other.say_hello").is_none());
    assert!(host.command("say_hello").is_none());
    assert!(!host.points().all(ExtensionPoints::is_empty));
}
