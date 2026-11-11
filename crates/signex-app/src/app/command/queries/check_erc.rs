//! `check_erc document=<sheet> [baseline=<run>]` (#2199, decision
//! https://github.com/alplabai/signex/issues/2199#issuecomment-6798772157):
//! ERC on demand. It checks the project of the named sheet (a loose sheet on
//! its own) as Run ERC does, off the UI thread, and answers with a
//! `signex_types::check` report once the run lands: it is deferred like an
//! edit that waits for its reads, and answered once. The run is numbered and
//! kept (`state::check_runs`); with `baseline` the answer also lists the keys
//! that are `new` and `gone` since that run. Nothing reaches the ERC panel,
//! the ERC state or Messages; `run_erc` is unchanged. See `docs/CLI.md`,
//! "Checks on demand".

use std::path::PathBuf;
use std::sync::Arc;

use iced::Task;
use signex_types::check::CheckReport;

use super::text_operand;
use crate::app::agent::AgentMsg;
use crate::app::command::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::document_ids::DocumentKind;
use crate::app::handlers::erc::ERC_CHECK;
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;

/// A `check_erc` request whose run is under way.
#[derive(Debug, Clone)]
pub struct CheckRequested {
    pub(crate) ticket: u64,
    pub(crate) command: AppCommandId,
    pub(crate) args: CommandArgs,
    /// The sheet the request named.
    pub(crate) sheet: PathBuf,
    /// What the run checks, for its baseline: the project file, or the
    /// loose sheet.
    pub(crate) scope: PathBuf,
    pub(crate) baseline: Option<u64>,
}

/// A finished `check_erc` run: the request and what the run found.
#[derive(Debug, Clone)]
pub struct CheckFinished {
    pub(crate) request: CheckRequested,
    pub(crate) report: Arc<Result<CheckReport, String>>,
}

impl Signex {
    /// Start the check `args` asks for; refused at once when the operands
    /// name nothing it can check.
    pub(super) fn check_erc(
        &mut self,
        command: &AppCommandId,
        args: &CommandArgs,
    ) -> (CommandOutcome, Task<Message>) {
        let (sheet, scope) = match self.erc_check_target(args) {
            Ok(target) => target,
            Err(reason) => return (CommandOutcome::disabled(reason), Task::none()),
        };
        let baseline = match baseline_operand(args) {
            Ok(baseline) => baseline,
            Err(reason) => return (CommandOutcome::disabled(reason), Task::none()),
        };
        let project = self
            .document_state
            .projects
            .iter()
            .find(|project| project.path == scope);
        let job = self.erc_check_job(&sheet, project);
        let edits = &mut self.document_state.agent_edits;
        edits.tickets += 1;
        let ticket = edits.tickets;
        let request = CheckRequested {
            ticket,
            command: command.clone(),
            args: args.clone(),
            sheet,
            scope,
            baseline,
        };
        let task = Task::perform(
            async move {
                tokio::task::spawn_blocking(job)
                    .await
                    .map_err(|error| format!("the check did not finish: {error}"))
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

    /// The sheet `args` names and what checking it covers: its project's
    /// file, or the sheet itself when it is in no open project.
    fn erc_check_target(&self, args: &CommandArgs) -> Result<(PathBuf, PathBuf), String> {
        let Some(OperandValue::Document(target)) = args.operand("document") else {
            return Err("`check_erc` needs the operand `document`".to_string());
        };
        let entry = self.resolve_document_target(target)?;
        if entry.kind != DocumentKind::Schematic {
            return Err(format!(
                "`check_erc` checks a sheet, and {} is a {}",
                entry.name,
                entry.kind.as_str()
            ));
        }
        let Some(sheet) = entry.path else {
            return Err(format!("{} has no file to check", entry.name));
        };
        let project = entry.project.as_ref().and_then(|named| {
            self.document_state
                .projects
                .iter()
                .find(|project| project.path == named.path)
        });
        let scope = project.map_or_else(|| sheet.clone(), |project| project.path.clone());
        Ok((sheet, scope))
    }

    /// The answer to a finished run: numbered, kept and compared with its
    /// baseline; refused when the run failed or its baseline is not kept.
    pub(crate) fn erc_check_answer(&mut self, finished: &CheckFinished) -> CommandOutcome {
        let request = &finished.request;
        let report = match finished.report.as_ref() {
            Ok(report) => report.clone(),
            Err(why) => return CommandOutcome::disabled(why.clone()),
        };
        let runs = &mut self.document_state.check_runs;
        let earlier = match request.baseline {
            Some(run) => match runs.keys_of(run, ERC_CHECK, &request.scope) {
                Ok(keys) => Some((run, keys.to_vec())),
                Err(why) => return CommandOutcome::disabled(why),
            },
            None => None,
        };
        let run = runs.record(ERC_CHECK, &request.scope, report.keys());
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
