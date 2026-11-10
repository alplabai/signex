//! An agent's edit that waits for its disk reads (#2199, decision
//! https://github.com/alplabai/signex/issues/2199#issuecomment-6771692137): the
//! request is answered when the edit has run on their completion, never when
//! it was dispatched. A headless run takes the outcome by its ticket instead.
//! See `command::edit_reads` and `docs/AGENT_EDITS.md`, "Edits that read the
//! disk".

use iced::Task;

use super::reply::outcome_verdict;
use super::{AgentReply, ChangeSnapshot};
use crate::app::command::{CommandOutcome, ReadsLoaded};
use crate::app::{Message, Signex};

/// What the reply to a waiting request needs: the request's id and caller,
/// and what the app looked like when it arrived.
pub(crate) struct WaitingReply {
    pub(super) id: Option<String>,
    pub(super) pid: Option<u32>,
    pub(super) mark: Option<u64>,
    pub(super) before: ChangeSnapshot,
    pub(super) reply: AgentReply,
}

impl Signex {
    /// The reads of a waiting edit landed: run it (`command::edit_reads`) and
    /// answer the socket request that started it, or keep the outcome for the
    /// headless run that waits on its ticket.
    pub(crate) fn handle_edit_reads_loaded(&mut self, loaded: ReadsLoaded) -> Task<Message> {
        let ticket = loaded.ticket;
        let id = loaded.command.as_str().to_string();
        let (outcome, task) = self.run_with_reads(loaded);
        match self.agent.waiting.remove(&ticket) {
            Some(waiting) => {
                let verdict = outcome_verdict(&id, outcome, None);
                self.send_agent_reply(
                    waiting.id.as_deref(),
                    waiting.pid,
                    waiting.mark,
                    &waiting.before,
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

    /// The outcome of the edit waiting on `ticket`, once it ran.
    pub(crate) fn take_finished_edit(&mut self, ticket: u64) -> Option<CommandOutcome> {
        self.document_state.agent_edits.finished.remove(&ticket)
    }
}
