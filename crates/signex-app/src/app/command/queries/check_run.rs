//! What every on-demand check shares (#2199, decisions
//! https://github.com/alplabai/signex/issues/2199#issuecomment-6798772157 and
//! https://github.com/alplabai/signex/issues/2199#issuecomment-6817821301):
//! the run goes off the UI thread and is answered once it lands, as an edit
//! that waits for its reads is; the answer is numbered, kept
//! (`state::check_runs`) and compared with its `baseline`. `check_erc` and
//! `check_drc` differ only in what they check. See `docs/CLI.md`, "Checks on
//! demand".

use std::path::PathBuf;
use std::sync::Arc;

use iced::Task;
use signex_types::check::CheckReport;

use super::text_operand;
use crate::app::agent::AgentMsg;
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;

/// A check request whose run is under way.
#[derive(Debug, Clone)]
pub struct CheckRequested {
    pub(crate) ticket: u64,
    pub(crate) command: AppCommandId,
    pub(crate) args: CommandArgs,
    /// Which check runs (`erc`, `drc`).
    pub(crate) check: &'static str,
    /// The document the request named.
    pub(crate) document: PathBuf,
    /// What the run checks, for its baseline: for ERC the project file or a
    /// loose sheet, for DRC the board file.
    pub(crate) scope: PathBuf,
    pub(crate) baseline: Option<u64>,
}

/// A finished check run: the request and what the run found.
#[derive(Debug, Clone)]
pub struct CheckFinished {
    pub(crate) request: CheckRequested,
    pub(crate) report: Arc<Result<CheckReport, String>>,
}

/// What a check run checks: the document named and the baseline scope.
pub(super) struct CheckTarget {
    pub(super) check: &'static str,
    pub(super) document: PathBuf,
    pub(super) scope: PathBuf,
}

impl Signex {
    /// Run `job` off the UI thread for `target`; the request is answered when
    /// it lands. Refused at once when `baseline` is not a run number.
    pub(super) fn start_check(
        &mut self,
        command: &AppCommandId,
        args: &CommandArgs,
        target: CheckTarget,
        job: impl FnOnce() -> Result<CheckReport, String> + Send + 'static,
    ) -> (CommandOutcome, Task<Message>) {
        let baseline = match baseline_operand(args) {
            Ok(baseline) => baseline,
            Err(reason) => return (CommandOutcome::disabled(reason), Task::none()),
        };
        let edits = &mut self.document_state.agent_edits;
        edits.tickets += 1;
        let ticket = edits.tickets;
        let request = CheckRequested {
            ticket,
            command: command.clone(),
            args: args.clone(),
            check: target.check,
            document: target.document,
            scope: target.scope,
            baseline,
        };
        let task = Task::perform(
            async move {
                tokio::task::spawn_blocking(job)
                    .await
                    .map_err(|error| format!("the check did not finish: {error}"))
                    .and_then(|report| report)
            },
            move |report| {
                let finished = CheckFinished {
                    request: request.clone(),
                    report: Arc::new(report),
                };
                Message::Agent(AgentMsg::CheckFinished(Box::new(finished)))
            },
        );
        (CommandOutcome::Deferred { ticket }, task)
    }

    /// The answer to a finished run: numbered, kept and compared with its
    /// baseline; refused when the run failed or its baseline is not kept.
    pub(crate) fn check_answer(&mut self, finished: &CheckFinished) -> CommandOutcome {
        let request = &finished.request;
        let report = match finished.report.as_ref() {
            Ok(report) => report.clone(),
            Err(why) => return CommandOutcome::disabled(why.clone()),
        };
        let runs = &mut self.document_state.check_runs;
        let earlier = match request.baseline {
            Some(run) => match runs.keys_of(run, request.check, &request.scope) {
                Ok(keys) => Some((run, keys.to_vec())),
                Err(why) => return CommandOutcome::disabled(why),
            },
            None => None,
        };
        let run = runs.record(request.check, &request.scope, report.keys());
        let report = CheckReport { run, ..report };
        let report = match earlier {
            Some((baseline, keys)) => report.compared_with(baseline, &keys),
            None => report,
        };
        match serde_json::to_value(&report) {
            Ok(value) => CommandOutcome::Answered(value),
            Err(error) => {
                CommandOutcome::disabled(format!("the report could not be written: {error}"))
            }
        }
    }
}

/// The run `baseline` names, if it was given.
fn baseline_operand(args: &CommandArgs) -> Result<Option<u64>, String> {
    let Some(text) = text_operand(args, "baseline") else {
        return Ok(None);
    };
    text.trim()
        .parse::<u64>()
        .map(Some)
        .map_err(|_| format!("`baseline` is the run number an earlier answer gave, not `{text}`"))
}
