//! `open_gerber_viewer [path=..]` (#2896): the Gerber Viewer tab from the
//! catalog, so the palette, the CLI and the screenshot run reach it. With no
//! `path` the tab opens, or is focused, and the command answers with what it
//! shows. With a `path` (a folder, a zip or one file) that package is read
//! as a drop or the Open menu reads it, and the command answers once the
//! read lands (`CommandOutcome::Deferred`): the layers, the files not read
//! and every read problem, worded as its Messages row. Rules:
//! `docs/GERBER_VIEWER.md`, "Opening from the catalog".

use std::path::PathBuf;

use iced::Task;
use serde_json::{Value, json};
use signex_gerber::{ReadProblem, Severity};
use signex_i18n::{I18n, Translatable};

use crate::app::agent::WaitingCommand;
use crate::app::command::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::gerber_read::problem_text;
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;

type Ran = (CommandOutcome, Task<Message>);

/// The catalog id this module answers for.
const COMMAND_ID: &str = "open_gerber_viewer";

/// The operand handler of `open_gerber_viewer`.
pub(crate) fn open_gerber_viewer(app: &mut Signex, args: &CommandArgs) -> Ran {
    let Some(OperandValue::Path(path)) = args.operand("path") else {
        let task = app.show_gerber_viewer();
        return (CommandOutcome::Answered(app.gerber_viewer_answer()), task);
    };
    let Ok(command) = AppCommandId::new(COMMAND_ID) else {
        return (CommandOutcome::UnknownId, Task::none());
    };
    let path = PathBuf::from(path);
    let waiting = app.wait_for_command(&command, args);
    let ticket = waiting.ticket;
    let task = app.open_gerber_paths_for(vec![path], Some(waiting));
    (CommandOutcome::Deferred { ticket }, task)
}

impl Signex {
    /// Answer the call `waiting` with what the tab shows now; nothing
    /// without one (a person's open).
    pub(super) fn answer_gerber_command(
        &mut self,
        waiting: Option<WaitingCommand>,
        paths: &[PathBuf],
    ) -> Task<Message> {
        let Some(waiting) = waiting else {
            return Task::none();
        };
        let target = target_of(paths);
        self.finish_waiting_command(waiting, &target, |app| {
            CommandOutcome::Answered(app.gerber_viewer_answer())
        })
    }

    /// Refuse the call `waiting` with `reason`; nothing without one.
    pub(super) fn refuse_gerber_command(
        &mut self,
        waiting: Option<WaitingCommand>,
        paths: &[PathBuf],
        reason: Translatable,
    ) -> Task<Message> {
        let Some(waiting) = waiting else {
            return Task::none();
        };
        let target = target_of(paths);
        self.finish_waiting_command(waiting, &target, |_| CommandOutcome::Disabled { reason })
    }

    /// What the Gerber Viewer shows, as `open_gerber_viewer` answers it:
    /// the paths opened, the layers in the list's order (top first), the
    /// files not read and each read problem with its Messages sentence.
    pub(crate) fn gerber_viewer_answer(&self) -> Value {
        let viewer = &self.document_state.gerber_viewer;
        let reading = viewer.reading.is_some();
        let Some(open) = viewer.open.as_ref() else {
            return json!({
                "tab": "gerber_viewer",
                "reading": reading,
                "paths": [],
                "layers": [],
                "not_read": [],
                "problems": [],
            });
        };
        let layers: Vec<&str> = open
            .order
            .iter()
            .filter_map(|layer| open.package.layers.get(*layer))
            .map(|layer| layer.image.file.as_str())
            .collect();
        let not_read: Vec<&str> = open
            .package
            .not_read
            .iter()
            .map(|problem| problem.at.file.as_str())
            .collect();
        let problems: Vec<Value> = open
            .package
            .problems()
            .into_iter()
            .map(|problem| problem_answer(problem, &self.ui_state.i18n))
            .collect();
        let paths: Vec<String> = open
            .paths
            .iter()
            .map(|path| path.display().to_string())
            .collect();
        json!({
            "tab": "gerber_viewer",
            "reading": reading,
            "paths": paths,
            "layers": layers,
            "not_read": not_read,
            "problems": problems,
        })
    }
}

/// One read problem as the answer lists it.
fn problem_answer(problem: &ReadProblem, i18n: &I18n) -> Value {
    json!({
        "severity": severity_name(problem.severity()),
        "file": problem.at.file,
        "line": problem.at.line,
        "message": problem_text(problem, i18n).into_string(),
    })
}

fn severity_name(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Info => "info",
    }
}

/// The path an answer turned away at the agent gate is about: the first
/// path opened.
fn target_of(paths: &[PathBuf]) -> PathBuf {
    paths.first().cloned().unwrap_or_default()
}
