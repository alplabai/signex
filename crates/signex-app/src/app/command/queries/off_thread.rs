//! A query whose answer is worked out off the UI thread (#1313): the run
//! goes to a worker and the request is answered once it lands, as a check or
//! an edit that waits for its reads is (`agent::QueryAnswered`). It changes
//! nothing, is not numbered and says nothing in Messages.

use std::path::PathBuf;

use iced::Task;
use serde_json::Value;
use signex_i18n::Translatable;

use crate::app::agent::{AgentMsg, QueryAnswered};
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;

impl Signex {
    /// Run `job` on a worker for `command` on `target`; the request is
    /// answered when it lands, a failure in the words `not_read` gives it.
    pub(super) fn answer_off_thread(
        &mut self,
        command: &AppCommandId,
        args: &CommandArgs,
        target: PathBuf,
        not_read: fn(String) -> Translatable,
        job: impl FnOnce() -> Result<Value, String> + Send + 'static,
    ) -> (CommandOutcome, Task<Message>) {
        let waiting = self.wait_for_command(command, args);
        let ticket = waiting.ticket;
        let task = Task::perform(
            crate::app::path_op_jobs::run_blocking(job),
            move |ran: Result<Result<Value, String>, String>| {
                let answered = QueryAnswered {
                    waiting: waiting.clone(),
                    document: target.clone(),
                    answer: ran.and_then(|answer| answer).map_err(not_read),
                };
                Message::Agent(AgentMsg::QueryAnswered(Box::new(answered)))
            },
        );
        (CommandOutcome::Deferred { ticket }, task)
    }
}
