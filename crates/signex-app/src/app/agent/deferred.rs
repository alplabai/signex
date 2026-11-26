//! An agent's edit that waits for its disk reads (#2199, decision
//! https://github.com/alplabai/signex/issues/2199#issuecomment-6771692137): the
//! request is answered when the edit has run on their completion, never when
//! it was dispatched. A headless run takes the outcome by its ticket instead.
//! A check (`check_erc`, `check_drc`) waits for its run the same way. See
//! `command::edit_reads` and `docs/AGENT_EDITS.md`, "Edits that read the
//! disk".

use std::path::Path;

use iced::Task;

use super::reply::outcome_verdict;
use super::{AgentReply, ChangeSnapshot};
use crate::app::command::queries::CheckFinished;
use crate::app::command::{CommandArgs, CommandOutcome, ReadsLoaded};
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;

/// What the reply to a waiting request needs: the request's id and caller,
/// and the session token it was let in with.
pub(crate) struct WaitingReply {
    pub(super) id: Option<String>,
    pub(super) pid: Option<u32>,
    pub(super) reply: AgentReply,
    pub(super) token: Option<String>,
}

/// A command run with operands that waits on its own off-thread work (a file
/// read): what answering it once the work lands needs.
#[derive(Debug, Clone, PartialEq)]
pub struct WaitingCommand {
    pub(crate) ticket: u64,
    pub(crate) command: AppCommandId,
    pub(crate) args: CommandArgs,
    pub(crate) origin: super::Origin,
}

/// A query whose work ran off the UI thread (`query_bom`, #2938;
/// `query_rail_budget`, #2943): the request it answers, the document it
/// named and its answer, or why there is none, in the query's own words.
#[derive(Debug, Clone)]
pub struct QueryAnswered {
    pub(crate) waiting: WaitingCommand,
    pub(crate) document: std::path::PathBuf,
    pub(crate) answer: Result<serde_json::Value, signex_i18n::Translatable>,
}

impl Signex {
    /// The work of a query landed: answer its caller with it. A query writes
    /// no edit journal line.
    pub(crate) fn handle_query_answered(&mut self, answered: QueryAnswered) -> Task<Message> {
        let QueryAnswered {
            waiting,
            document,
            answer,
        } = answered;
        self.finish_waiting_command(waiting, &document, move |_| match answer {
            Ok(value) => CommandOutcome::Answered(value),
            Err(reason) => CommandOutcome::Disabled { reason },
        })
    }

    /// Start waiting for `command`: a new ticket, answered by
    /// [`Signex::finish_waiting_command`].
    pub(crate) fn wait_for_command(
        &mut self,
        command: &AppCommandId,
        args: &CommandArgs,
    ) -> WaitingCommand {
        let edits = &mut self.document_state.agent_edits;
        edits.tickets += 1;
        WaitingCommand {
            ticket: edits.tickets,
            command: command.clone(),
            args: args.clone(),
            origin: self.agent.origin.clone(),
        }
    }

    /// The work `waiting` waited for landed: `run` it on `target` and answer
    /// the socket request, or keep the outcome for the headless run.
    pub(crate) fn finish_waiting_command(
        &mut self,
        waiting: WaitingCommand,
        target: &Path,
        run: impl FnOnce(&mut Self) -> CommandOutcome,
    ) -> Task<Message> {
        let WaitingCommand {
            ticket,
            command,
            args,
            origin,
        } = waiting;
        let mark = crate::diagnostics::newest_entry_id();
        let before = self.change_snapshot();
        let (outcome, task) = match self.agent_gate_closed(ticket, &command, &args) {
            Some(reason) => self.turned_away_waiting(ticket, target, reason),
            None => self.with_origin(origin, |app| {
                app.journaled(&command, &args, |app| (run(app), Task::none()))
            }),
        };
        self.answer_waiting(ticket, command.as_str(), mark, &before, outcome);
        task
    }

    /// The reads of a waiting edit landed: run it (`command::edit_reads`) and
    /// answer the socket request that started it, or keep the outcome for the
    /// headless run that waits on its ticket.
    pub(crate) fn handle_edit_reads_loaded(&mut self, loaded: ReadsLoaded) -> Task<Message> {
        let ticket = loaded.ticket;
        let id = loaded.command.as_str().to_string();
        // The reply reports what this run does: what other requests or the
        // person changed and logged while the reads ran is not the agent's
        // (its first pass was refused with nothing applied).
        let mark = crate::diagnostics::newest_entry_id();
        let before = self.change_snapshot();
        let (outcome, task) = match self.agent_gate_closed(ticket, &loaded.command, &loaded.args) {
            Some(reason) => self.turned_away_waiting(ticket, &loaded.target, reason),
            // A socket or headless edit is journaled as one that did not wait.
            None => {
                let (command, args) = (loaded.command.clone(), loaded.args.clone());
                self.with_origin(loaded.origin.clone(), |app| {
                    app.journaled(&command, &args, |app| app.run_with_reads(loaded))
                })
            }
        };
        self.answer_waiting(ticket, &id, mark, &before, outcome);
        task
    }

    /// The run of a check finished: answer its caller with the report. A
    /// query writes no edit journal line.
    pub(crate) fn handle_check_finished(&mut self, finished: &CheckFinished) -> Task<Message> {
        let request = &finished.request;
        let mark = crate::diagnostics::newest_entry_id();
        let before = self.change_snapshot();
        let (outcome, task) =
            match self.agent_gate_closed(request.ticket, &request.command, &request.args) {
                Some(reason) => self.turned_away_waiting(request.ticket, &request.document, reason),
                None => (self.check_answer(finished), Task::none()),
            };
        let id = request.command.as_str().to_string();
        self.answer_waiting(request.ticket, &id, mark, &before, outcome);
        task
    }

    /// A command that waited for a read of its own (the project's reuse
    /// blocks, #2410) runs now that it landed, as the caller and channel that
    /// sent it, journaled like an edit that did not wait, and its caller is
    /// answered (the socket, or a headless run by `ticket`).
    pub(crate) fn rerun_after_read(
        &mut self,
        ticket: u64,
        command: &AppCommandId,
        args: &CommandArgs,
        document: &Path,
        origin: super::Origin,
        serving: bool,
    ) -> Task<Message> {
        let mark = crate::diagnostics::newest_entry_id();
        let before = self.change_snapshot();
        let (outcome, task) = match self.agent_gate_closed(ticket, command, args) {
            Some(reason) => self.turned_away_waiting(ticket, document, reason),
            None => self.with_origin(origin, |app| {
                let serving = std::mem::replace(&mut app.agent.serving, serving);
                let ran = app.journaled(command, args, |app| app.run_operands(command, args));
                app.agent.serving = serving;
                ran
            }),
        };
        self.answer_waiting(ticket, command.as_str(), mark, &before, outcome);
        task
    }

    /// Turned away at the gate, as at request time: an access event, no edit
    /// line.
    fn turned_away_waiting(
        &mut self,
        ticket: u64,
        target: &Path,
        reason: String,
    ) -> (CommandOutcome, Task<Message>) {
        let pid = self.agent.waiting.get(&ticket).and_then(|w| w.pid);
        let named = self
            .document_state
            .document_project(target)
            .map(|project| project.path.clone());
        let refused = self.journal_turned_away(pid, Some(&reason), named);
        (CommandOutcome::disabled(reason), refused)
    }

    /// Answer the socket request waiting on `ticket` with `outcome`, or keep
    /// it for the headless run that waits on the ticket.
    fn answer_waiting(
        &mut self,
        ticket: u64,
        id: &str,
        mark: Option<u64>,
        before: &ChangeSnapshot,
        outcome: CommandOutcome,
    ) {
        match self.agent.waiting.remove(&ticket) {
            Some(waiting) => {
                let verdict = outcome_verdict(id, outcome, None);
                self.send_agent_reply(
                    waiting.id.as_deref(),
                    waiting.pid,
                    mark,
                    before,
                    verdict,
                    &waiting.reply,
                );
            }
            None => {
                self.document_state
                    .agent_edits
                    .finished
                    .insert(ticket, outcome);
            }
        }
    }

    /// Why a socket request waiting on `ticket` may no longer run: the gate
    /// a new request meets (agent connections on and listening, the session
    /// token unchanged, the document one of an open project's), checked again
    /// when its reads land. `None` for a headless run's edit, which has no
    /// socket.
    fn agent_gate_closed(
        &mut self,
        ticket: u64,
        command: &AppCommandId,
        args: &CommandArgs,
    ) -> Option<String> {
        let waiting = self.agent.waiting.get(&ticket)?;
        if !self.ui_state.agent_connections_enabled || self.agent.endpoint.is_none() {
            return Some("agent connections are off".to_string());
        }
        // Connections turned off and on again meanwhile: a new token.
        let current = self.agent.token().map(|t| t.as_str().to_string());
        if waiting.token.is_none() || waiting.token != current {
            return Some("the session token is missing or wrong".to_string());
        }
        self.check_agent_document(command, args).err()
    }

    /// The outcome of the edit waiting on `ticket`, once it ran.
    pub(crate) fn take_finished_edit(&mut self, ticket: u64) -> Option<CommandOutcome> {
        self.document_state.agent_edits.finished.remove(&ticket)
    }
}
