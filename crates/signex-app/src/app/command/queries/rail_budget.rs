//! `query_rail_budget document=<project, or one of its sheets or its board>`
//! (#2943): every rail of the project with its voltage, its source, each
//! load and its current, the total and the headroom. The parts are read off
//! the UI thread (`handlers::requirements_check::rails`), so the request is
//! answered once the run lands, as an edit that waits for its reads is. It
//! changes nothing, is not numbered and says nothing in Messages. See
//! `docs/CLI.md`, "Rail budgets".

use iced::Task;

use crate::app::agent::{AgentMsg, QueryAnswered};
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;

/// A `query_rail_budget` refusal, the reason under the catalog's "not read"
/// line.
fn rail_budget_not_read(why: String) -> signex_i18n::Translatable {
    signex_i18n::Translatable::new(signex_i18n::ids::REQUIREMENTS_RAIL_BUDGET_REFUSED)
        .arg("why", why)
}

impl Signex {
    /// Start the run `args` asks for; refused at once when the operands name
    /// no open project.
    pub(super) fn query_rail_budget(
        &mut self,
        command: &AppCommandId,
        args: &CommandArgs,
    ) -> (CommandOutcome, Task<Message>) {
        let refused = |why: String| {
            let reason = rail_budget_not_read(why);
            (CommandOutcome::Disabled { reason }, Task::none())
        };
        let id = match self.rules_project_named(args) {
            Ok(id) => id,
            Err(reason) => return refused(format!("`query_rail_budget` {reason}")),
        };
        let Some(project) = self.document_state.project_by_id(id) else {
            return refused("`query_rail_budget`: the project is no longer open".to_string());
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
                let answered = QueryAnswered {
                    waiting: waiting.clone(),
                    document: path.clone(),
                    answer: answer.map_err(rail_budget_not_read),
                };
                Message::Agent(AgentMsg::QueryAnswered(Box::new(answered)))
            },
        );
        (CommandOutcome::Deferred { ticket }, task)
    }
}
