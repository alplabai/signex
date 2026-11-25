//! One request path (#2495): a catalog command asked for from any surface (a
//! menu row, a key or the palette, the agent socket, a headless run, an
//! extension) runs through [`Signex::run_command_request`] and comes back as
//! one typed [`RequestOutcome`]. Each surface keeps its own gate in front of
//! it and its own runner behind it; what they share is the `if_revision`
//! check and the reading of the outcome. See `docs/EXTENSION_API.md`, "One
//! request path".

mod outcome;
#[cfg(test)]
mod tests;

use iced::Task;

pub use outcome::{RefusalCause, RequestOutcome};

use super::queries::is_query;
use super::{CommandArgs, OperandValue};
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;

/// A command to run by id, with its arguments.
#[derive(Debug, Clone, PartialEq)]
pub struct CommandRequest {
    pub command: AppCommandId,
    pub args: CommandArgs,
    /// The revision (as `read_document` reports it) that the document named
    /// by the `document` operand must still be at. When it moved, the request
    /// is refused and nothing runs.
    pub if_revision: Option<u64>,
}

impl CommandRequest {
    /// A request for `command` with `args` and no revision check.
    pub fn new(command: AppCommandId, args: CommandArgs) -> Self {
        Self {
            command,
            args,
            if_revision: None,
        }
    }

    /// This request, refused unless its document is still at `revision`.
    #[must_use]
    pub fn with_if_revision(mut self, revision: Option<u64>) -> Self {
        self.if_revision = revision;
        self
    }
}

/// Where a request comes from, which picks the runner behind the shared step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RequestSurface {
    /// A menu row, a key, the palette or a context-menu row, from the window
    /// `window` (`None`: the main one), run as `dispatch_command_reporting`
    /// runs it.
    Gui { window: Option<iced::window::Id> },
    /// A headless run: [`Signex::dispatch_command_headless`].
    Headless,
    /// A request on the agent socket that passed the channel's gate (token,
    /// allowlist, document). Only the agent channel can make one.
    Agent(AgentGatePassed),
}

/// Proof that a request passed the agent channel's gate: it cannot be made
/// outside this crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AgentGatePassed(());

impl AgentGatePassed {
    pub(crate) const fn after_gate() -> Self {
        Self(())
    }
}

impl Signex {
    /// Run `request` as `surface` runs a command and say what it came to.
    /// The `if_revision` check comes first: a request whose document moved
    /// is refused before any runner sees it.
    pub fn run_command_request(
        &mut self,
        surface: RequestSurface,
        request: CommandRequest,
    ) -> (RequestOutcome, Task<Message>) {
        let CommandRequest {
            command,
            args,
            if_revision,
        } = request;
        if let Some(expected) = if_revision
            && let Some(refusal) = self.revision_refusal(&args, expected)
        {
            return (refusal, Task::none());
        }
        match surface {
            RequestSurface::Gui { window } => {
                let (outcome, task) = self.dispatch_command_reporting(&command, args, window);
                (RequestOutcome::of(command.as_str(), outcome), task)
            }
            RequestSurface::Headless => {
                let (outcome, task) = self.dispatch_command_headless(&command, args);
                (RequestOutcome::of(command.as_str(), outcome), task)
            }
            RequestSurface::Agent(_) => self.run_agent_request(&command, &args),
        }
    }

    /// The agent channel's runner: a window-layout command runs as its menu
    /// row does and answers with the layout, a query answers through its
    /// handler, any other command runs its operands and is journaled.
    fn run_agent_request(
        &mut self,
        command: &AppCommandId,
        args: &CommandArgs,
    ) -> (RequestOutcome, Task<Message>) {
        if crate::app::agent::is_window_layout(command) {
            let (outcome, task) = self.run_agent_window_layout(command);
            return (RequestOutcome::of(command.as_str(), outcome), task);
        }
        self.agent.serving = true;
        let ran = if is_query(command) {
            self.run_query_headless(command, args)
        } else {
            Some(self.journaled(command, args, |app| app.run_operands(command, args)))
        };
        self.agent.serving = false;
        match ran {
            Some((outcome, task)) => (RequestOutcome::of(command.as_str(), outcome), task),
            None => (
                RequestOutcome::refused("not a query", RefusalCause::NotAvailable),
                Task::none(),
            ),
        }
    }

    /// Why a request with `if_revision` = `expected` may not run, if it may
    /// not: its `document` names nothing, has no revision, or is at another.
    /// Checked again when a waiting agent request's work lands.
    pub(crate) fn revision_refusal(
        &mut self,
        args: &CommandArgs,
        expected: u64,
    ) -> Option<RequestOutcome> {
        let refused =
            |reason: String| Some(RequestOutcome::refused(reason, RefusalCause::Revision));
        let Some(OperandValue::Document(target)) = args.operand("document") else {
            return refused("`if_revision` needs the operand `document`".to_string());
        };
        self.reconcile_document_ids();
        let entry = match self.resolve_document_target(target) {
            Ok(entry) => entry,
            Err(why) => return refused(why),
        };
        match self.document_revision(&entry) {
            Some(now) if now == expected => None,
            Some(now) => refused(format!(
                "{} changed since revision {expected} (it is at {now}); read it again and retry. Nothing ran",
                entry.name
            )),
            None => refused(format!(
                "{} has no revision for `if_revision` to compare",
                entry.name
            )),
        }
    }
}
