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

/// How often an unregistered project is tried again while listening.
pub(super) const REGISTRY_RETRY: std::time::Duration = std::time::Duration::from_secs(5);

/// How many times a failed listener is restarted before it stays off.
pub(super) const MAX_RESTARTS: u32 = 5;

/// The wait before restart number `restarts` + 1 (2, 4, 8, 16, 32 s), or
/// `None` once it has been restarted [`MAX_RESTARTS`] times.
pub(super) fn restart_delay(restarts: u32) -> Option<std::time::Duration> {
    (restarts < MAX_RESTARTS).then(|| std::time::Duration::from_secs(2_u64 << restarts))
}

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
            AgentMsg::ListenerStarted { boot_id, endpoint } => {
                // A start that lands after the setting went off, or from a
                // listener the app has since replaced, is ignored; that
                // subscription is already being dropped.
                if !self.ui_state.agent_connections_enabled
                    || self.agent.boot_id() != Some(boot_id.as_str())
                {
                    return Task::none();
                }
                crate::diagnostics::log_info(format!("Agent connections: listening on {endpoint}"));
                self.agent.endpoint = Some(endpoint);
                self.agent.registered = None;
                // The last listener's registry trouble is not this one's.
                self.agent.registry_retry = false;
                self.agent.registry_in_flight = false;
                self.agent.note_touched(boot_id);
                self.sync_agent_registry().unwrap_or_else(Task::none)
            }
            AgentMsg::ListenerHealthy { boot_id } => {
                // Only a listener that has served a connection earns back its
                // restarts; one that binds and then fails does not.
                if self.agent.boot_id() == Some(boot_id.as_str()) {
                    self.agent.restarts = 0;
                }
                Task::none()
            }
            AgentMsg::ListenerFailed { boot_id, reason } => {
                if self.agent.boot_id() != Some(boot_id.as_str()) {
                    return Task::none();
                }
                self.agent.listener_closed();
                let Some(delay) = restart_delay(self.agent.restarts) else {
                    crate::diagnostics::log_warning(format!(
                        "Agent connections: stopped: {reason}; it failed {MAX_RESTARTS} times, so it stays off. Turn \"Allow agent connections\" off and on in Preferences to try again"
                    ));
                    return Task::none();
                };
                self.agent.restarts += 1;
                crate::diagnostics::log_warning(format!(
                    "Agent connections: stopped: {reason}; trying again in {} s",
                    delay.as_secs()
                ));
                // The sleep is made inside the task: `update` runs outside a
                // runtime.
                Task::perform(async move { tokio::time::sleep(delay).await }, move |()| {
                    Message::Agent(AgentMsg::RestartListener {
                        boot_id: boot_id.clone(),
                    })
                })
            }
            AgentMsg::RestartListener { boot_id } => {
                // New ids make a new subscription identity, which iced starts.
                if self.ui_state.agent_connections_enabled
                    && self.agent.boot_id() == Some(boot_id.as_str())
                {
                    self.agent.rotate_ids();
                }
                Task::none()
            }
            AgentMsg::RegistrySynced { boot_id, result } => {
                // The result of a listener that is gone is not this one's.
                if self.agent.boot_id() != Some(boot_id.as_str()) {
                    return Task::none();
                }
                self.agent.registry_in_flight = false;
                match result {
                    Ok(()) => self.agent.registry_retry = false,
                    Err(why) => {
                        // Said once per run of failures, not at every retry.
                        if !self.agent.registry_retry {
                            crate::diagnostics::log_warning(format!(
                                "Agent connections: registry: {why}; trying again every {} s",
                                REGISTRY_RETRY.as_secs()
                            ));
                        }
                        self.agent.registry_retry = true;
                    }
                }
                Task::none()
            }
            AgentMsg::RegistryRetryTick => {
                // A write that has not finished (a peer that does not answer)
                // is not piled on.
                if self.agent.registry_in_flight {
                    return Task::none();
                }
                self.agent.registered = None;
                self.sync_agent_registry().unwrap_or_else(Task::none)
            }
        }
    }

    /// The subscription that listens for agents; none while the "Allow agent
    /// connections" setting is off.
    ///
    /// While a listener is up and the last registry write left a project
    /// unregistered, a timer asks for another try every [`REGISTRY_RETRY`].
    pub(crate) fn agent_listener_subscription(&self) -> Subscription<Message> {
        let listening = listener::subscription(
            self.agent
                .listener_config(self.ui_state.agent_connections_enabled),
        );
        let retry = if self.agent.registry_retry {
            iced::time::every(REGISTRY_RETRY).map(|_| Message::Agent(AgentMsg::RegistryRetryTick))
        } else {
            Subscription::none()
        };
        Subscription::batch([listening, retry])
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
            self.agent.restarts = 0;
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
    /// was last written, whether or not agent connections are on: an open
    /// project is listed (with the endpoint once a listener is up, without one
    /// before), so a headless run never works on a file open here. A headless
    /// app lists nothing: it is the run. Called at the end of every update; the
    /// write runs on a worker.
    pub(crate) fn sync_agent_registry(&mut self) -> Option<Task<Message>> {
        if !self.agent.lists_projects {
            return None;
        }
        let owner = self.agent.owner()?;
        let dir = self.agent.runtime_dir().ok().cloned()?;
        let mut projects: Vec<_> = self
            .document_state
            .projects
            .iter()
            .map(|project| project.path.clone())
            .collect();
        projects.sort();
        // A project that got open some way that did not claim it (the open
        // handler does) is claimed now; a window that closed a project lets go
        // of its lock at once, in step with the update (the entry follows on
        // the worker).
        for project in &projects {
            self.claim_project(project).ok();
        }
        self.agent
            .project_locks
            .retain(|path, _| projects.contains(path));
        if self.agent.registered.as_ref() == Some(&projects) {
            return None;
        }
        self.agent.registered = Some(projects.clone());
        self.agent.registry_in_flight = true;
        self.agent.registry_seq += 1;
        let seq = self.agent.registry_seq;
        let lock = self.agent.registry_lock.clone();
        let boot_id = owner.boot_id.clone();
        Some(Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    registry::sync_in_order(&dir, &owner, &projects, seq, &lock)
                })
                .await
                .unwrap_or_else(|error| Err(format!("the registry write did not finish: {error}")))
            },
            move |result| {
                Message::Agent(AgentMsg::RegistrySynced {
                    boot_id: boot_id.clone(),
                    result,
                })
            },
        ))
    }

    /// The quit path: close the listener and remove its files before `next`,
    /// so an exit that skips the subscription's own cleanup leaves none.
    ///
    /// It also cleans the boot ids a listener of this run reported, with the
    /// setting off, and removes this process's registry entries (the projects
    /// are closing with it) and releases its locks. A headless app, which
    /// listed nothing, adds no task.
    pub(crate) fn agent_cleanup_then(&mut self, next: Task<Message>) -> Task<Message> {
        if !self.agent.lists_projects {
            return next;
        }
        let mut ids = std::mem::take(&mut self.agent.touched);
        if let Some(current) = self.agent.boot_id()
            && !ids.iter().any(|id| id == current)
        {
            ids.push(current.to_string());
        }
        let Ok(dir) = self.agent.runtime_dir().cloned() else {
            return next;
        };
        if ids.is_empty() {
            return next;
        }
        self.agent.listener_closed();
        // Nothing is listed again after the exit path: an update that runs
        // before the process ends must not write the entries back.
        self.agent.lists_projects = false;
        self.agent.project_locks.clear();
        let lock = self.agent.registry_lock.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    for id in &ids {
                        listener::cleanup(&dir, id, &lock, true);
                    }
                })
                .await
                .ok();
            },
            |()| Message::noop(),
        )
        .chain(next)
    }

    /// The window claims the project it is about to open: it takes the shared
    /// lock on the project's lock file, now, in the update that opens it, and
    /// holds it for as long as the project stays open. Many windows can hold
    /// it; a headless `signex-cli` run holds it exclusively, so while one runs
    /// this fails with the run's process and its entry file, and the app does
    /// not open a project an agent is editing without a window. The lock is
    /// what `signex-cli` finds when it looks: nothing can slip in between this
    /// and the registry entry, which is written later, on a worker. One small
    /// file open and a non-blocking lock, the class of IO that reading the
    /// project file here is. Only the window lists (and claims): an app made
    /// for a test, and the headless run itself, do not. A lock that cannot be
    /// made for another reason is said in Messages and the project opens.
    pub(crate) fn claim_project(
        &mut self,
        project: &std::path::Path,
    ) -> Result<(), (u32, std::path::PathBuf)> {
        if !self.agent.lists_projects || self.agent.project_locks.contains_key(project) {
            return Ok(());
        }
        let (Ok(dir), Ok(canonical)) = (
            self.agent.runtime_dir().cloned(),
            std::fs::canonicalize(project),
        ) else {
            return Ok(());
        };
        match registry::claim_shared(&dir, &canonical) {
            Ok(lock) => {
                self.agent.project_locks.insert(project.to_path_buf(), lock);
                Ok(())
            }
            Err(registry::LockError::Held) => Err(registry::headless_holder(&dir, &canonical)),
            Err(registry::LockError::Other(why)) => {
                crate::diagnostics::log_warning(format!(
                    "Agent registry: {} could not be locked: {why}",
                    project.display()
                ));
                Ok(())
            }
        }
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
            // Closing drops a sheet's drafts of every kind (title block,
            // paper edge): the person's unconfirmed typing is not the agent's
            // to finish or lose.
            if entry
                .path
                .as_ref()
                .is_some_and(|path| self.document_state.drafts.has_sheet(path))
            {
                return Err(
                    "the person is editing this sheet's title block or page settings".to_string(),
                );
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
