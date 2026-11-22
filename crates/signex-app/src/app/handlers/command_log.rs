//! The app side of the Command Log (#2458): a row per catalog command that
//! ran, recorded once per outer dispatch, and the panel's actions (filters,
//! Replay, copy, Save log as…, Jump to). The model is
//! `app::core::command_log`; the panel is `panels::command_log`. Rules:
//! `docs/COMMAND_LOG.md`.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use iced::Task;
use serde_json::{Map, Value};
use signex_i18n::{ids, t};
use signex_types::schematic::{SelectedItem, SelectedKind};

use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::core::command_log::RecordedTargets;
use crate::app::document_ids::DocumentTarget;
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;
use crate::panels::PanelMsg;
use crate::panels::command_log::{
    COPIED_FOR, CommandLogMsg, CopyTarget, JumpTarget, LogOutcome, LogRow, LogSource,
};

#[path = "command_log_replay/mod.rs"]
mod replay;
pub(crate) use replay::ReplayBook;
#[cfg(test)]
#[path = "command_log_tests.rs"]
mod tests;

/// The default name Save log as… offers.
const SAVE_FILE_NAME: &str = "command-log.jsonl";

/// A command being run: when it started, `None` when it runs inside another
/// logged command (an undocked window's undo, an agent request) and is not
/// logged again.
pub(crate) struct LogTicket {
    started: Option<Instant>,
}

/// The message a Command Log action sends back to the panel.
fn panel_message(msg: CommandLogMsg) -> Message {
    Message::Dock(crate::dock::DockMessage::Panel(PanelMsg::CommandLog(msg)))
}

/// What a dispatch outcome reads as in the log: the reason of a refusal
/// verbatim.
pub(crate) fn dispatch_outcome(id: &str, outcome: &CommandOutcome) -> LogOutcome {
    match outcome {
        CommandOutcome::Ran | CommandOutcome::Answered(_) => LogOutcome::Ok(None),
        // In English, as the log reads the other outcomes.
        CommandOutcome::Disabled { reason } => {
            LogOutcome::Failed(signex_i18n::RenderTranslatable::render_en_us(reason))
        }
        CommandOutcome::UnknownId => LogOutcome::Failed(format!("unknown command `{id}`")),
        CommandOutcome::NoBridgeArm => {
            LogOutcome::Failed(format!("`{id}` has no dispatch arm yet"))
        }
        CommandOutcome::NotHeadless => {
            LogOutcome::Failed("not runnable outside a headless run".to_string())
        }
        CommandOutcome::Deferred { .. } => LogOutcome::Warn("waiting for its reads".to_string()),
    }
}

/// What an agent reply reads as in the log: its reason as the text.
fn verdict_outcome(status: crate::app::agent::Status, reason: Option<String>) -> LogOutcome {
    use crate::app::agent::Status;
    match status {
        Status::Ran | Status::Answered => LogOutcome::Ok(reason),
        Status::Refused => LogOutcome::Failed(reason.unwrap_or_else(|| "refused".to_string())),
        Status::Failed => LogOutcome::Failed(reason.unwrap_or_else(|| "failed".to_string())),
    }
}

/// The operands of `args` as the log keeps them: name to the text given.
pub(crate) fn operands_of(args: &CommandArgs) -> Map<String, Value> {
    args.operand_texts()
        .iter()
        .map(|(name, text)| (name.clone(), Value::String(text.clone())))
        .collect()
}

impl Signex {
    /// Start a command; [`Self::command_log_leave`] ends it. Only the
    /// outermost one, outside an agent request, is logged.
    pub(crate) fn command_log_enter(&mut self) -> LogTicket {
        let state = &mut self.ui_state.command_log;
        let outer = state.nesting == 0 && !self.agent.serving;
        state.nesting += 1;
        LogTicket {
            started: outer.then(Instant::now),
        }
    }

    /// End a command; how long it took when it is to be logged.
    pub(crate) fn command_log_leave(&mut self, ticket: LogTicket) -> Option<Duration> {
        let state = &mut self.ui_state.command_log;
        state.nesting = state.nesting.saturating_sub(1);
        ticket.started.map(|started| started.elapsed())
    }

    /// Run `command` through `run` and log it once, as the person's (or a
    /// headless run's) command from `window`.
    pub(crate) fn logged_command(
        &mut self,
        command: &AppCommandId,
        operands: Map<String, Value>,
        window: Option<iced::window::Id>,
        run: impl FnOnce(&mut Self) -> (CommandOutcome, Task<Message>),
    ) -> (CommandOutcome, Task<Message>) {
        self.logged_command_on(command, operands, None, window, run)
    }

    /// [`Self::logged_command`] for a command that acts on `targets`, read
    /// before it ran: its row keeps them for Replay.
    pub(crate) fn logged_command_on(
        &mut self,
        command: &AppCommandId,
        operands: Map<String, Value>,
        targets: Option<RecordedTargets>,
        window: Option<iced::window::Id>,
        run: impl FnOnce(&mut Self) -> (CommandOutcome, Task<Message>),
    ) -> (CommandOutcome, Task<Message>) {
        let ticket = self.command_log_enter();
        // The window's document as it ran: a command may close or switch it.
        let document = self.command_document(window, &operands);
        let is_outer = ticket.started.is_some();
        if is_outer {
            self.begin_replay_capture();
        }
        let (outcome, task) = run(self);
        let captured = if is_outer {
            self.end_replay_capture()
        } else {
            None
        };
        if let Some(took) = self.command_log_leave(ticket) {
            let logged = dispatch_outcome(command.as_str(), &outcome);
            let ran = !logged.is_failed();
            let source = self.dispatch_source(window);
            let document = if targets.is_some() || captured.is_some() {
                document
            } else {
                self.command_document(window, &operands)
            };
            let seq =
                self.record_command(command.as_str(), operands, logged, took, source, document);
            if let Some(seq) = seq {
                self.keep_replay_input(seq, command, targets, captured, ran);
            }
        }
        (outcome, task)
    }

    /// Log an agent request once it has its verdict. A request whose token
    /// did not match names no command (`id` is `None`) and is not logged; one
    /// whose reply waits for its reads is logged as waiting and takes its
    /// outcome when it answers ([`Self::settle_agent_log_row`]).
    pub(crate) fn log_agent_request(
        &mut self,
        ticket: LogTicket,
        pid: Option<u32>,
        input: &crate::app::agent::Input,
        id: Option<&str>,
        (status, reason): (crate::app::agent::Status, Option<String>),
        waits: bool,
    ) {
        let took = self.command_log_leave(ticket);
        let (Some(took), Some(id), crate::app::agent::Input::Line(line)) = (took, id, input) else {
            return;
        };
        // Only the arguments and the client's name; never the token.
        let request: Value = serde_json::from_str(line).unwrap_or(Value::Null);
        let operands = request
            .get("args")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        let client = request
            .get("client")
            .and_then(Value::as_str)
            .map(str::to_string);
        let outcome = if waits {
            LogOutcome::Warn("waiting for its reads".to_string())
        } else {
            verdict_outcome(status, reason)
        };
        let document = self.named_document(&operands);
        let source = LogSource::of_agent(client, pid);
        let seq = self.record_command(id, operands, outcome, took, source, document);
        if waits && let Some(seq) = seq {
            self.ui_state.command_log.log.await_reply(pid, id, seq);
        }
    }

    /// A waiting agent request of `pid` for `id` was answered: its row takes
    /// the outcome.
    pub(crate) fn settle_agent_log_row(
        &mut self,
        pid: Option<u32>,
        id: &str,
        status: crate::app::agent::Status,
        reason: Option<&str>,
    ) {
        let log = &mut self.ui_state.command_log.log;
        if let Some(seq) = log.take_awaiting(pid, id) {
            log.settle(seq, verdict_outcome(status, reason.map(str::to_string)));
            self.sync_command_log_panel_ctx();
        }
    }

    /// Who a dispatch came from: the person in the main window or an
    /// undocked one, or the agent of a headless run.
    fn dispatch_source(&self, window: Option<iced::window::Id>) -> LogSource {
        if let crate::app::agent::Origin::Agent { pid, client } = &self.agent.origin {
            return LogSource::of_agent(client.clone(), *pid);
        }
        let undocked = window
            .filter(|w| self.ui_state.main_window_id != Some(*w))
            .and_then(|w| self.document_state.sheet_path_for_window(w, &self.ui_state))
            .and_then(|path| path.file_name())
            .map(|name| name.to_string_lossy().into_owned());
        LogSource::Gui { window: undocked }
    }

    /// The document a command acted on: the one its `document` operand
    /// names, else the one its window shows.
    fn command_document(
        &self,
        window: Option<iced::window::Id>,
        operands: &Map<String, Value>,
    ) -> Option<PathBuf> {
        if let Some(named) = self.named_document(operands) {
            return Some(named);
        }
        match window.filter(|w| self.ui_state.main_window_id != Some(*w)) {
            Some(w) => self
                .document_state
                .sheet_path_for_window(w, &self.ui_state)
                .cloned(),
            None => self.active_tab_path(),
        }
    }

    /// The file of the document the `document` operand names, when it
    /// resolves.
    pub(crate) fn named_document(&self, operands: &Map<String, Value>) -> Option<PathBuf> {
        let text = operands.get("document")?.as_str()?;
        let target = DocumentTarget::parse(text).ok()?;
        self.resolve_document_target(&target).ok()?.path
    }

    /// Add a row for `command` and show it in the panel. Returns its number,
    /// `None` while logging is paused.
    pub(crate) fn record_command(
        &mut self,
        command: &str,
        operands: Map<String, Value>,
        outcome: LogOutcome,
        took: Duration,
        source: LogSource,
        document: Option<PathBuf>,
    ) -> Option<u64> {
        let meta = crate::keymap::metadata_for_str(command);
        let label = meta.map_or_else(
            || command.to_string(),
            |meta| t!(self.ui_state.i18n, meta.label).into_string(),
        );
        let undoable = meta.is_some_and(|meta| meta.flags.undoable);
        let view_only = meta.is_some_and(|meta| {
            matches!(meta.category, "view" | "select") && !meta.flags.mutates_doc
        });
        let project = self.project_file_of(document.as_deref());
        let row = LogRow {
            seq: 0,
            at: chrono::Local::now(),
            source,
            command: command.to_string(),
            label,
            operands,
            outcome,
            took,
            document,
            project,
            undoable,
            view_only,
            undone_at: None,
        };
        let seq = self.ui_state.command_log.log.record(row);
        if seq.is_some() {
            self.sync_command_log_panel_ctx();
        }
        seq
    }

    /// The project file of `document`, else of the document in view.
    fn project_file_of(&self, document: Option<&Path>) -> Option<PathBuf> {
        let state = &self.document_state;
        document
            .and_then(|path| state.document_project(path))
            .or_else(|| state.active_document_project())
            .map(|project| project.path.clone())
    }

    /// Give the panel context (and every undocked window's) the log as it
    /// is now, with the agent socket's endpoint.
    pub(crate) fn sync_command_log_panel_ctx(&mut self) {
        let endpoint = self.agent_listening_endpoint();
        self.ui_state.command_log.endpoint = endpoint;
        let shared = Arc::new((*self.ui_state.command_log).clone());
        self.document_state.panel_ctx.command_log = Arc::clone(&shared);
        for ctx in self.document_state.window_panel_ctx.values_mut() {
            ctx.command_log = Arc::clone(&shared);
        }
    }

    /// The footer's endpoint changed (the listener came up or went away):
    /// show the new one.
    pub(crate) fn sync_command_log_endpoint(&mut self) {
        if self.ui_state.command_log.endpoint != self.agent_listening_endpoint() {
            self.sync_command_log_panel_ctx();
        }
    }

    /// Run one Command Log panel action.
    pub(crate) fn handle_command_log_message(&mut self, msg: CommandLogMsg) -> Task<Message> {
        let state = &mut self.ui_state.command_log;
        let task = match msg {
            CommandLogMsg::FilterChanged(query) => {
                state.log.filters.query = query;
                Task::none()
            }
            CommandLogMsg::SourceToggled(kind) => {
                state.log.toggle_source(kind);
                Task::none()
            }
            CommandLogMsg::ResultFilterPicked(result) => {
                state.log.filters.result = result;
                Task::none()
            }
            CommandLogMsg::RowSelected(seq) => {
                state.selected = Some(seq);
                Task::none()
            }
            CommandLogMsg::PauseToggled => {
                state.log.paused = !state.log.paused;
                Task::none()
            }
            CommandLogMsg::Cleared => {
                state.log.clear();
                state.selected = None;
                state.copied = None;
                Task::none()
            }
            CommandLogMsg::FollowNewestToggled => {
                state.follow_newest = !state.follow_newest;
                Task::none()
            }
            CommandLogMsg::MoreToggled => {
                state.more_open = !state.more_open;
                Task::none()
            }
            CommandLogMsg::ViewCommandsToggled => {
                state.log.show_view_commands = !state.log.show_view_commands;
                Task::none()
            }
            CommandLogMsg::CopiedFlashEnded(flash) => {
                if state.copied.is_some_and(|(_, current)| current == flash) {
                    state.copied = None;
                }
                Task::none()
            }
            CommandLogMsg::CopyCliRequested(seq) => self.copy_from_row(seq, CopyTarget::Cli),
            CommandLogMsg::CopyErrorRequested(seq) => self.copy_from_row(seq, CopyTarget::Error),
            CommandLogMsg::ReplayRequested(seq) => self.replay_logged_command(seq),
            CommandLogMsg::JumpRequested(seq) => {
                self.jump_to_logged_object(seq);
                Task::none()
            }
            CommandLogMsg::SaveRequested => self.save_command_log(),
            CommandLogMsg::Saved(result) => {
                command_log_saved(&self.ui_state.i18n, result);
                Task::none()
            }
        };
        self.sync_command_log_panel_ctx();
        task
    }

    /// Put the row's CLI line or full error on the clipboard and show
    /// "Copied" on its button for a moment.
    fn copy_from_row(&mut self, seq: u64, target: CopyTarget) -> Task<Message> {
        let Some(row) = self.ui_state.command_log.log.row(seq) else {
            return Task::none();
        };
        let copied = match target {
            CopyTarget::Cli => {
                let placeholder =
                    t!(self.ui_state.i18n, ids::COMMAND_LOG_CLI_NO_PROJECT).into_string();
                row.cli_line(&placeholder)
            }
            CopyTarget::Error => row.outcome.text().unwrap_or_default().to_string(),
        };
        let flash = self.ui_state.command_log.start_flash(target);
        Task::batch([
            iced::clipboard::write(copied).discard(),
            Task::perform(async { tokio::time::sleep(COPIED_FOR).await }, move |()| {
                panel_message(CommandLogMsg::CopiedFlashEnded(flash))
            }),
        ])
    }

    /// Replay: run the row's command again as the person's command, on what
    /// it acted on, and log it as a new row. Rules: `docs/COMMAND_LOG.md`.
    fn replay_logged_command(&mut self, seq: u64) -> Task<Message> {
        let Some(row) = self.ui_state.command_log.log.row(seq).cloned() else {
            return Task::none();
        };
        let Ok(command) = AppCommandId::new(row.command.as_str()) else {
            let line = t!(
                self.ui_state.i18n,
                ids::COMMAND_LOG_NOT_A_COMMAND,
                "command" => row.command.as_str(),
            );
            crate::diagnostics::log_warning(line.into_string());
            return Task::none();
        };
        if !row.has_given_operands() {
            return self.replay_without_operands(&row, &command);
        }
        let texts = row.operand_texts();
        let refs: Vec<&str> = texts.iter().map(String::as_str).collect();
        match CommandArgs::parse_operands(&command, &refs) {
            Ok(args) => {
                self.logged_command(&command, row.operands.clone(), None, |app| {
                    app.run_operands(&command, &args)
                })
                .1
            }
            Err(error) => {
                self.record_refused_replay(&row, error.to_string());
                Task::none()
            }
        }
    }

    /// Log a Replay that did not run as a new failed GUI row with `reason`,
    /// on the document and targets of the row it replays.
    fn record_refused_replay(&mut self, row: &LogRow, reason: String) {
        let document = row
            .document
            .clone()
            .or_else(|| self.command_document(None, &row.operands));
        let targets = self.ui_state.command_log.log.targets_of(row.seq).cloned();
        let seq = self.record_command(
            row.command.as_str(),
            row.operands.clone(),
            LogOutcome::Failed(reason),
            Duration::ZERO,
            LogSource::Gui { window: None },
            document,
        );
        if let (Some(seq), Some(targets)) = (seq, targets) {
            self.ui_state.command_log.log.keep_targets(seq, targets);
            self.sync_command_log_panel_ctx();
        }
    }

    /// Jump to: show the object the row's arguments name on its sheet, the
    /// sheet brought forward when it is open. Said in Messages when it is
    /// not on the sheet in view.
    fn jump_to_logged_object(&mut self, seq: u64) {
        let Some(row) = self.ui_state.command_log.log.row(seq).cloned() else {
            return;
        };
        let Some(target) = row.jump_target() else {
            return;
        };
        if let Some(document) = row.document.as_deref()
            && self.active_tab_path().as_deref() != Some(document)
        {
            let _focused = self.focus_open_sheet(document);
        }
        let found = match &target {
            JumpTarget::Point((x, y)) => Some((*x, *y, None)),
            JumpTarget::Designator(name) => self.active_schematic().and_then(|sheet| {
                sheet
                    .symbols
                    .iter()
                    .find(|symbol| symbol.reference == *name)
                    .map(|symbol| {
                        let item = SelectedItem::new(symbol.uuid, SelectedKind::Symbol);
                        (symbol.position.x, symbol.position.y, Some(item))
                    })
            }),
            JumpTarget::Net(name) => self.active_schematic().and_then(|sheet| {
                sheet
                    .labels
                    .iter()
                    .find(|label| label.text == *name)
                    .map(|label| {
                        let item = SelectedItem::new(label.uuid, SelectedKind::Label);
                        (label.position.x, label.position.y, Some(item))
                    })
            }),
        };
        match found {
            Some((x, y, select)) => self.focus_world_point(x, y, select),
            None => {
                let name = match &target {
                    JumpTarget::Designator(name) | JumpTarget::Net(name) => name.as_str(),
                    JumpTarget::Point(_) => "",
                };
                let line = t!(self.ui_state.i18n, ids::COMMAND_LOG_NOT_IN_VIEW, "name" => name);
                crate::diagnostics::log_info(line.into_string());
            }
        }
    }

    /// Save log as…: ask for a file, then write the log there as JSON Lines,
    /// both off the update thread. Nothing is written unless asked.
    fn save_command_log(&mut self) -> Task<Message> {
        let i18n = &self.ui_state.i18n;
        let dialog = self
            .ui_state
            .io
            .picker
            .dialog()
            .title_in(i18n, ids::COMMAND_LOG_SAVE_TITLE)
            .filter_in(i18n, ids::COMMAND_LOG_SAVE_FILTER, &["jsonl"])
            .set_file_name(SAVE_FILE_NAME);
        let text = self.ui_state.command_log.log.to_jsonl();
        Task::perform(
            async move {
                let Some(path) = dialog.save_file().await else {
                    return Ok(None);
                };
                tokio::task::spawn_blocking(move || {
                    signex_types::atomic_io::atomic_write(&path, text.as_bytes())
                        .map(|()| Some(path.clone()))
                        .map_err(|error| format!("{}: {error}", path.display()))
                })
                .await
                .unwrap_or_else(|error| Err(error.to_string()))
            },
            |result| panel_message(CommandLogMsg::Saved(result)),
        )
    }
}

/// Say in Messages where the log went, or why it did not.
fn command_log_saved(i18n: &signex_i18n::I18n, result: Result<Option<PathBuf>, String>) {
    match result {
        Ok(Some(path)) => {
            let path = path.display().to_string();
            let line = t!(i18n, ids::COMMAND_LOG_SAVED_TO, "path" => path);
            crate::diagnostics::log_info(line.into_string());
        }
        Ok(None) => {}
        Err(why) => {
            let line = t!(i18n, ids::COMMAND_LOG_NOT_SAVED, "why" => why);
            crate::diagnostics::log_warning(line.into_string());
        }
    }
}
