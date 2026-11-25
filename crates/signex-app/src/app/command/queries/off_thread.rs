//! A query whose answer is worked out off the UI thread (#1313): the run
//! goes to a worker and the request is answered once it lands, as a check or
//! an edit that waits for its reads is. It changes nothing, is not numbered
//! and says nothing in Messages.

use std::path::PathBuf;

use iced::Task;
use serde_json::Value;

use crate::app::agent::{AgentMsg, WaitingCommand};
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;

/// A finished off-thread query: the request it answers, the document it
/// read and what it found.
#[derive(Debug, Clone)]
pub struct QueryAnswered {
    pub(crate) waiting: WaitingCommand,
    pub(crate) target: PathBuf,
    pub(crate) answer: Result<Value, String>,
}

impl Signex {
    /// Run `job` on a worker for `command` on `target`; the request is
    /// answered when it lands.
    pub(super) fn answer_off_thread(
        &mut self,
        command: &AppCommandId,
        args: &CommandArgs,
        target: PathBuf,
        job: impl FnOnce() -> Result<Value, String> + Send + 'static,
    ) -> (CommandOutcome, Task<Message>) {
        let waiting = self.wait_for_command(command, args);
        let ticket = waiting.ticket;
        let task = Task::perform(
            crate::app::path_op_jobs::run_blocking(job),
            move |ran: Result<Result<Value, String>, String>| {
                let answered = QueryAnswered {
                    waiting: waiting.clone(),
                    target: target.clone(),
                    answer: ran.and_then(|answer| answer),
                };
                Message::Agent(AgentMsg::QueryAnswered(Box::new(answered)))
            },
        );
        (CommandOutcome::Deferred { ticket }, task)
    }

    /// The run of an off-thread query landed: answer its caller (the
    /// socket, or a headless run by its ticket). A query writes no edit
    /// journal line.
    pub(crate) fn handle_query_answered(&mut self, answered: QueryAnswered) -> Task<Message> {
        let QueryAnswered {
            waiting,
            target,
            answer,
        } = answered;
        self.finish_waiting_command(waiting, &target, move |_| match answer {
            Ok(value) => CommandOutcome::Answered(value),
            Err(reason) => super::refused(reason).0,
        })
    }
}
