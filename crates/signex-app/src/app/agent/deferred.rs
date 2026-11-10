//! An agent's edit that waits for its disk reads (#2199, decision
//! https://github.com/alplabai/signex/issues/2199#issuecomment-6771692137): the
//! request is answered when the edit has run on their completion, never when
//! it was dispatched. A headless run takes the outcome by its ticket instead.
//! See `command::edit_reads` and `docs/AGENT_EDITS.md`, "Edits that read the
//! disk".

use iced::Task;

use super::AgentReply;
use super::reply::outcome_verdict;
use crate::app::command::{CommandOutcome, ReadsLoaded};
use crate::app::{Message, Signex};

/// What the reply to a waiting request needs: the request's id and caller.
pub(crate) struct WaitingReply {
    pub(super) id: Option<String>,
    pub(super) pid: Option<u32>,
    pub(super) reply: AgentReply,
}

impl Signex {
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
        let (outcome, task) = match self.agent_gate_closed(ticket, &loaded) {
            Some(reason) => (CommandOutcome::Disabled { reason }, Task::none()),
            None => self.run_with_reads(loaded),
        };
        match self.agent.waiting.remove(&ticket) {
            Some(waiting) => {
                let verdict = outcome_verdict(&id, outcome, None);
                self.send_agent_reply(
                    waiting.id.as_deref(),
                    waiting.pid,
                    mark,
                    &before,
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
        task
    }

    /// Why a socket request waiting on `ticket` may no longer run: the gate
    /// a new request meets (agent connections on and listening, the document
    /// one of an open project's), checked again when its reads land. `None`
    /// for a headless run's edit, which has no socket.
    fn agent_gate_closed(&mut self, ticket: u64, loaded: &ReadsLoaded) -> Option<String> {
        if !self.agent.waiting.contains_key(&ticket) {
            return None;
        }
        if !self.ui_state.agent_connections_enabled || self.agent.endpoint.is_none() {
            return Some("agent connections are off".to_string());
        }
        self.check_agent_document(&loaded.command, &loaded.args)
            .err()
    }

    /// The outcome of the edit waiting on `ticket`, once it ran.
    pub(crate) fn take_finished_edit(&mut self, ticket: u64) -> Option<CommandOutcome> {
        self.document_state.agent_edits.finished.remove(&ticket)
    }
}
