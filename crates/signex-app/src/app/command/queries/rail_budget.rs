//! `query_rail_budget document=<project, or one of its sheets or its board>`
//! (#2943): every rail of the project with its voltage, its source, each
//! load and its current, the total and the headroom. The parts are read off
//! the UI thread (`handlers::requirements_check::rails`), so the request is
//! answered once the run lands, as an edit that waits for its reads is. It
//! changes nothing, is not numbered and says nothing in Messages. See
//! `docs/CLI.md`, "Rail budgets".

use std::path::PathBuf;

use iced::Task;
use serde_json::Value;

use crate::app::agent::{AgentMsg, WaitingCommand};
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;

/// A finished rail budget run: the request it answers and what it found.
#[derive(Debug, Clone)]
pub struct RailBudgetAnswered {
    pub(crate) waiting: WaitingCommand,
    pub(crate) project: PathBuf,
    pub(crate) answer: Result<Value, String>,
}

impl Signex {
    /// Start the run `args` asks for; refused at once when the operands name
    /// no open project.
    pub(super) fn query_rail_budget(
        &mut self,
        command: &AppCommandId,
        args: &CommandArgs,
    ) -> (CommandOutcome, Task<Message>) {
        let id = match self.rules_project_named(args) {
            Ok(id) => id,
            Err(reason) => {
                let reason = format!("`query_rail_budget` {reason}");
                return (CommandOutcome::disabled(reason), Task::none());
            }
        };
        let Some(project) = self.document_state.project_by_id(id) else {
            let reason = "`query_rail_budget`: the project is no longer open";
            return (CommandOutcome::disabled(reason), Task::none());
        };
        let job = self.rail_budget_job(project);
        let path = project.path.clone();
        let waiting = self.wait_for_command(command, args);
        let ticket = waiting.ticket;
        let task = Task::perform(
            async move {
                tokio::task::spawn_blocking(job)
                    .await
                    .map_err(|error| format!("the rail budget did not finish: {error}"))
                    .and_then(|answer| answer)
            },
            move |answer| {
                let answered = RailBudgetAnswered {
                    waiting: waiting.clone(),
                    project: path.clone(),
                    answer,
                };
                Message::Agent(AgentMsg::RailBudgetAnswered(Box::new(answered)))
            },
        );
        (CommandOutcome::Deferred { ticket }, task)
    }

    /// The run of a rail budget query landed: answer its caller (the socket,
    /// or a headless run by its ticket). A query writes no edit journal line.
    pub(crate) fn handle_rail_budget_answered(
        &mut self,
        answered: RailBudgetAnswered,
    ) -> Task<Message> {
        let RailBudgetAnswered {
            waiting,
            project,
            answer,
        } = answered;
        self.finish_waiting_command(waiting, &project, move |_| match answer {
            Ok(value) => CommandOutcome::Answered(value),
            Err(reason) => CommandOutcome::disabled(reason),
        })
    }
}
