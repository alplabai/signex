//! The app's side of the agent channel (#1924): everything a request does
//! happens here, inside `Signex::update`, so it is one step of the same queue
//! as a click. A request names its document, runs through the command's own
//! handler (the engine channel) and never reads or writes the active tab,
//! focus, panels, selection or tool. Every request is logged to Messages.

use iced::{Subscription, Task};
use serde_json::Value;

use super::reply::{Status, changed_between, messages_since, outcome_verdict};
use super::wire::{self, Request};
use super::{AgentCall, AgentMsg, Input, allow, listener, registry};
use crate::app::command::queries::is_query;
use crate::app::command::{CommandArgs, OperandValue};
use crate::app::{DocumentKey, Message, Signex};
use crate::keymap::AppCommandId;

/// What running one request came to: the status, its reason and its data.
type Verdict = (Status, Option<String>, Option<Value>);

/// The longest reason or command text the Messages line repeats.
const LOGGED_TEXT_CHARS: usize = 200;

fn refused(reason: impl Into<String>) -> Verdict {
    (Status::Refused, Some(reason.into()), None)
}

/// `text` as one safe, bounded piece of a Messages line: control characters
/// and anything past [`LOGGED_TEXT_CHARS`] are dropped.
fn printable(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_control() { '?' } else { c })
        .take(LOGGED_TEXT_CHARS)
        .collect()
}

impl Signex {
    /// Route one agent message.
    pub(crate) fn dispatch_agent_message(&mut self, message: AgentMsg) -> Task<Message> {
        match message {
            AgentMsg::Request(call) => self.handle_agent_request(call),
            AgentMsg::PeerRefused { pid, reason } => {
                crate::diagnostics::log_warning(format!(
                    "Agent connection refused ({}): {}",
                    pid_text(pid),
                    printable(&reason)
                ));
                Task::none()
            }
            AgentMsg::ListenerStarted { endpoint } => {
                // A start that lands after the setting went off is ignored;
                // the subscription is already being dropped.
                if !self.ui_state.agent_connections_enabled {
                    return Task::none();
                }
                crate::diagnostics::log_info(format!("Agent connections: listening on {endpoint}"));
                self.agent.endpoint = Some(endpoint);
                self.agent.registered = None;
                self.sync_agent_registry().unwrap_or_else(Task::none)
            }
            AgentMsg::ListenerFailed(why) => {
                crate::diagnostics::log_warning(format!("Agent connections: stopped: {why}"));
                self.agent.listener_closed();
                Task::none()
            }
            AgentMsg::RegistrySynced(result) => {
                if let Err(why) = result {
                    crate::diagnostics::log_warning(format!("Agent connections: registry: {why}"));
                }
                Task::none()
            }
        }
    }

    /// The subscription that listens for agents; none while the "Allow agent
    /// connections" setting is off.
    pub(crate) fn agent_listener_subscription(&self) -> Subscription<Message> {
        listener::subscription(
            self.agent
                .listener_config(self.ui_state.agent_connections_enabled),
        )
    }

    /// The Save of Preferences committed the draft of "Allow agent
    /// connections". Turning it off forgets the listener here; its
    /// subscription ends with the next `subscription()` and closes the socket
    /// and every connection.
    pub(crate) fn commit_agent_connections_pref(&mut self) {
        let wanted = self.ui_state.preferences_draft_agent_connections_enabled;
        if wanted == self.ui_state.agent_connections_enabled {
            return;
        }
        self.ui_state.agent_connections_enabled = wanted;
        if wanted {
            crate::diagnostics::log_info(
                "Agent connections: on; an agent running as you can now connect",
            );
        } else {
            self.agent.listener_closed();
            self.agent.rotate_ids();
            crate::diagnostics::log_info(
                "Agent connections: off; the socket is closed and connections are dropped",
            );
        }
    }

    /// Write the registry for the projects open now, if they changed since it
    /// was last written and a listener is up. Called at the end of every
    /// update; the write runs on a worker.
    pub(crate) fn sync_agent_registry(&mut self) -> Option<Task<Message>> {
        let owner = self.agent.owner()?;
        let dir = self.agent.runtime_dir().ok().cloned()?;
        let mut projects: Vec<_> = self
            .document_state
            .projects
            .iter()
            .map(|project| project.path.clone())
            .collect();
        projects.sort();
        if self.agent.registered.as_ref() == Some(&projects) {
            return None;
        }
        self.agent.registered = Some(projects.clone());
        let lock = self.agent.registry_lock.clone();
        Some(Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    let _held = lock
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    registry::sync(&dir, &owner, &projects)
                })
                .await
                .unwrap_or_else(|error| Err(format!("the registry write did not finish: {error}")))
            },
            |result| Message::Agent(AgentMsg::RegistrySynced(result)),
        ))
    }

    /// The quit path: close the listener and remove its files before `next`,
    /// so an exit that skips the subscription's own cleanup leaves none.
    pub(crate) fn agent_cleanup_then(&mut self, next: Task<Message>) -> Task<Message> {
        let Some(config) = self
            .agent
            .listener_config(self.ui_state.agent_connections_enabled)
        else {
            return next;
        };
        self.agent.listener_closed();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    listener::cleanup(&config.dir, &config.boot_id);
                })
                .await
                .ok();
            },
            |()| Message::noop(),
        )
        .chain(next)
    }

    fn handle_agent_request(&mut self, call: AgentCall) -> Task<Message> {
        let mark = crate::diagnostics::newest_entry_id();
        let before = self.change_snapshot();
        let (id, (status, reason, data), task) = self.agent_verdict(&call.input);
        let changed = changed_between(&before, &self.change_snapshot());
        // What the command logged, before this request's own line.
        let messages = messages_since(mark);
        let named = id.as_deref().map_or_else(
            || "(no command)".to_string(),
            |id| format!("`{}`", printable(id)),
        );
        let why = reason
            .as_deref()
            .map(|why| format!(": {}", printable(why)))
            .unwrap_or_default();
        let line = format!(
            "Agent request ({}): {named} -> {}{why}",
            pid_text(call.pid),
            status_text(status)
        );
        if status.is_ok() {
            crate::diagnostics::log_info(line);
        } else {
            crate::diagnostics::log_warning(line);
        }
        call.reply.send(wire::reply_line(
            id.as_deref(),
            status,
            reason.as_deref(),
            data.as_ref(),
            &messages,
            &changed,
        ));
        task
    }

    /// Read, check and run one request. The id is returned only once the
    /// token proved the request is from a client that holds it.
    fn agent_verdict(&mut self, input: &Input) -> (Option<String>, Verdict, Task<Message>) {
        let line = match input {
            Input::Line(line) => line,
            Input::Oversized => {
                let why = format!(
                    "the request is larger than {} bytes",
                    wire::MAX_REQUEST_BYTES
                );
                return (None, refused(why), Task::none());
            }
            Input::NotUtf8 => {
                return (None, refused("the request is not UTF-8 text"), Task::none());
            }
        };
        // A connection that outlived the setting is not served.
        if !self.ui_state.agent_connections_enabled || self.agent.endpoint.is_none() {
            return (None, refused("agent connections are off"), Task::none());
        }
        let request = match wire::parse_request(line) {
            Ok(request) => request,
            Err(why) => return (None, refused(why), Task::none()),
        };
        if !self
            .agent
            .token()
            .is_some_and(|token| token.matches(&request.token))
        {
            return (
                None,
                refused("the session token is missing or wrong"),
                Task::none(),
            );
        }
        let id = request.id.clone();
        let (verdict, task) = self.run_agent_command(request);
        (Some(id), verdict, task)
    }

    fn run_agent_command(&mut self, request: Request) -> (Verdict, Task<Message>) {
        let none = Task::none;
        let Ok(command) = AppCommandId::new(request.id.as_str()) else {
            return (refused("not a command id"), none());
        };
        if let Some(why) = allow::refusal(&command) {
            return (refused(why), none());
        }
        let texts: Vec<String> = request
            .operands
            .iter()
            .map(|(name, text)| format!("{name}={text}"))
            .collect();
        let refs: Vec<&str> = texts.iter().map(String::as_str).collect();
        let args = match CommandArgs::parse_operands(&command, &refs) {
            Ok(args) => args,
            Err(error) => return (refused(error.to_string()), none()),
        };
        if let Err(why) = self.check_agent_document(&command, &args) {
            return (refused(why), none());
        }
        self.agent.serving = true;
        let ran = if is_query(&command) {
            self.run_query_headless(&command, &args)
        } else {
            Some(self.run_operands(&command, &args))
        };
        self.agent.serving = false;
        match ran {
            Some((outcome, task)) => (outcome_verdict(command.as_str(), outcome, None), task),
            None => (refused("not a query"), none()),
        }
    }

    /// A command that is not a query acts on the document it names, one of an
    /// open project's, and never on the person's active one. Closing it must
    /// not move the person's tab either.
    fn check_agent_document(
        &mut self,
        command: &AppCommandId,
        args: &CommandArgs,
    ) -> Result<(), String> {
        let id = command.as_str();
        self.reconcile_document_ids();
        let target = match args.operand("document") {
            Some(OperandValue::Document(target)) => target,
            None if is_query(command) => return Ok(()),
            _ => {
                return Err(format!(
                    "`{id}` needs an explicit `document`: an agent never acts on the person's active one"
                ));
            }
        };
        let entry = self.resolve_document_target(target)?;
        if entry.project.is_none() {
            return Err(format!(
                "{} is not part of a project open in this app",
                entry.name
            ));
        }
        if id == "close_active_document" {
            let active = self
                .document_state
                .tabs
                .get(self.document_state.active_tab)
                .and_then(DocumentKey::of);
            if entry.key.is_some() && entry.key == active {
                return Err(format!(
                    "{} is the person's active document; closing it would move their tab",
                    entry.name
                ));
            }
            if entry.open_in_window {
                return Err(format!(
                    "{} is open in a window of its own; closing it would close that window",
                    entry.name
                ));
            }
        }
        Ok(())
    }
}

fn pid_text(pid: Option<u32>) -> String {
    pid.map_or_else(|| "pid unknown".to_string(), |pid| format!("pid {pid}"))
}

fn status_text(status: Status) -> &'static str {
    match status {
        Status::Ran => "ran",
        Status::Answered => "answered",
        Status::Refused => "refused",
        Status::Failed => "failed",
    }
}
