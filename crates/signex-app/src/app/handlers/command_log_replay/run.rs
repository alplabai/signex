//! Replay's entry, and the commands whose real handler runs again on the
//! recorded selection targets.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use iced::Task;
use pcb_geom::ItemRef;
use serde_json::Map;
use signex_i18n::{RenderTranslatable, Translatable, ids};
use signex_types::designator::seed_counters;

use super::book::ReplayInput;
use super::scope::{ReplayScope, acts_on_all_when_empty, replay_scope};
use super::selection::{Shown, TabKey, board_selection};
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::handlers::clipboard_workflows::{CopyOutcome, DEFAULT_PASTE_OFFSET_MM};
use crate::app::project_sheets::{plan_project_sheets_for, resolve_child_reference_quiet};
use crate::app::state::PasteCounters;
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;
use crate::panels::command_log::{LogRow, LogSource};
use crate::panels::command_log::{RECORDED_TARGETS, RecordedTargets};

impl Signex {
    /// What `command`, about to run, acts on: `None` unless it is a person's
    /// outermost command that acts on a selection and has one.
    pub(crate) fn command_targets(
        &self,
        command: &AppCommandId,
        has_operands: bool,
        window: Option<iced::window::Id>,
    ) -> Option<RecordedTargets> {
        let is_person = matches!(self.dispatch_source(window), LogSource::Gui { .. });
        let on_selection = matches!(
            replay_scope(command.as_str()),
            ReplayScope::OnTargets | ReplayScope::SelectsFrom
        );
        if has_operands || !is_person || self.ui_state.command_log.nesting != 0 || !on_selection {
            return None;
        }
        let undocked = window.filter(|id| self.is_document_window(*id));
        let interaction = &self.interaction_state;
        let board = match undocked {
            Some(id) => self
                .undocked_pcb_path(id)
                .and_then(|_| interaction.pcb_selections.get(&id)),
            None => self.has_active_pcb().then_some(&interaction.pcb_selection),
        };
        if let Some(board) = board {
            let items: Vec<ItemRef> = board.selection().items().collect();
            let keeps = !items.is_empty() || acts_on_all_when_empty(command.as_str());
            return keeps.then_some(RecordedTargets::Board(items));
        }
        let (canvas, engine) = match undocked {
            Some(id) => (
                interaction.canvas_for_window(id),
                self.document_state.engine_for_window(id, &self.ui_state),
            ),
            None => (
                interaction.active_canvas(),
                self.document_state.active_engine(),
            ),
        };
        engine?;
        (!canvas.selected.is_empty()).then(|| RecordedTargets::Sheet(canvas.selected.clone()))
    }

    /// Replay a row that has no operands of its own: its recorded edit, or
    /// its command on its recorded targets.
    pub(in crate::app::handlers) fn replay_without_operands(
        &mut self,
        row: &LogRow,
        command: &AppCommandId,
    ) -> Task<Message> {
        if let Some(input) = self.ui_state.command_replay.input_of(row.seq) {
            return self.replay_recorded_input(row, command, &input);
        }
        match replay_scope(command.as_str()) {
            ReplayScope::SetsSelection => {
                self.dispatch_command_reporting(command, CommandArgs::none(), None)
                    .1
            }
            ReplayScope::NoTarget => {
                let held = self.take_main_selection();
                let (_, task) = self.dispatch_command_reporting(command, CommandArgs::none(), None);
                self.give_back_main_selection(held);
                task
            }
            ReplayScope::NothingToRepeat => {
                let reason = Translatable::new(ids::COMMAND_LOG_REPLAY_NOTHING_TO_REPEAT)
                    .arg("command", row.label.clone());
                self.refuse_replay(row, reason);
                Task::none()
            }
            ReplayScope::Continues(_) => {
                let reason = Translatable::new(ids::COMMAND_LOG_REPLAY_NOT_FINISHED)
                    .arg("command", row.label.clone());
                self.refuse_replay(row, reason);
                Task::none()
            }
            ReplayScope::OnTargets | ReplayScope::SelectsFrom => {
                self.replay_recorded_targets(row, command)
            }
        }
    }

    fn replay_recorded_targets(&mut self, row: &LogRow, command: &AppCommandId) -> Task<Message> {
        let targets = self.ui_state.command_log.log.targets_of(row.seq).cloned();
        match (targets, row.document.clone()) {
            (Some(targets), Some(document)) => {
                self.replay_on_targets(row, command, &targets, &document)
            }
            _ => {
                self.refuse_replay(row, Translatable::new(ids::COMMAND_LOG_REPLAY_NO_TARGETS));
                Task::none()
            }
        }
    }

    /// Run `command` on `targets` in `document`, or refuse with what is
    /// missing and change nothing.
    fn replay_on_targets(
        &mut self,
        row: &LogRow,
        command: &AppCommandId,
        targets: &RecordedTargets,
        document: &Path,
    ) -> Task<Message> {
        let Some(shown) = self.where_shown(document) else {
            self.refuse_replay(row, document_closed(document));
            return Task::none();
        };
        let missing = self.missing_targets(document, targets);
        if !missing.is_empty() {
            self.refuse_replay(row, targets_gone(document, &missing));
            return Task::none();
        }
        if self.targets_changed(row, document) {
            let reason = Translatable::new(ids::COMMAND_LOG_REPLAY_STATE_CHANGED)
                .arg("command", row.label.clone())
                .arg("document", document_name(document));
            self.refuse_replay(row, reason);
            return Task::none();
        }
        // A selection command with a tool running would act on the tool
        // (a part in hand turns), and a tab switch would end it.
        if !matches!(shown, Shown::Window(_)) && self.tool_in_progress() {
            self.refuse_replay(row, tool_active(document));
            return Task::none();
        }
        if command.as_str() == "duplicate"
            && matches!(targets, RecordedTargets::Sheet(_))
            && !self.read_duplicate_numbers(document)
        {
            self.refuse_replay(row, numbers_unread(document));
            return Task::none();
        }
        // A paste still waiting for its numbers has yet to place its copy.
        if command.as_str() == "duplicate"
            && self.ui_state.command_replay.has_waiting_paste(document)
        {
            let reason = Translatable::new(ids::COMMAND_LOG_REPLAY_PASTE_WAITING)
                .arg("document", document_name(document));
            self.refuse_replay(row, reason);
            return Task::none();
        }
        // The offset is fixed: a second copy would stack on the first.
        if command.as_str() == "duplicate" && self.duplicate_lands_on_a_copy(targets, document) {
            let reason = Translatable::new(ids::COMMAND_LOG_REPLAY_COPY_IN_PLACE)
                .arg("command", row.label.clone())
                .arg("document", document_name(document));
            self.refuse_replay(row, reason);
            return Task::none();
        }
        self.ui_state.command_replay.replaying = true;
        let task = match shown {
            Shown::Window(window) => self.replay_in_window(command, targets, document, window),
            Shown::Main => self.replay_in_main(command, targets),
            Shown::Behind { back } => {
                self.replay_behind(row, command, targets, document, back.as_ref())
            }
        };
        self.ui_state.command_replay.replaying = false;
        task
    }

    /// In the tab in view. A command that makes a selection leaves it.
    fn replay_in_main(
        &mut self,
        command: &AppCommandId,
        targets: &RecordedTargets,
    ) -> Task<Message> {
        let held = self.take_main_selection();
        self.put_main_targets(targets);
        let task = self.dispatch_on_targets(command, targets, None);
        if replay_scope(command.as_str()) == ReplayScope::SelectsFrom {
            self.update_selection_info();
            return task;
        }
        self.clear_main_selection();
        self.give_back_main_selection(held);
        task
    }

    /// In a tab behind the one in view, which comes back afterwards.
    fn replay_behind(
        &mut self,
        row: &LogRow,
        command: &AppCommandId,
        targets: &RecordedTargets,
        document: &Path,
        back: Option<&TabKey>,
    ) -> Task<Message> {
        let held = self.take_main_selection();
        if !self.focus_open_sheet(document) {
            self.give_back_main_selection(held);
            self.refuse_replay(row, document_busy(document));
            return Task::none();
        }
        self.put_main_targets(targets);
        let task = self.dispatch_on_targets(command, targets, None);
        self.clear_main_selection();
        self.return_to_tab(back);
        self.give_back_main_selection(held);
        task
    }

    /// In the undocked `window`, whose own selection is put back.
    fn replay_in_window(
        &mut self,
        command: &AppCommandId,
        targets: &RecordedTargets,
        document: &Path,
        window: iced::window::Id,
    ) -> Task<Message> {
        let revision = self.board_revision(document);
        let interaction = &mut self.interaction_state;
        let (held_sheet, held_board) = match targets {
            RecordedTargets::Sheet(items) => {
                let canvas = interaction.canvases.get_mut(&window);
                let held =
                    canvas.map(|canvas| std::mem::replace(&mut canvas.selected, items.clone()));
                (held, None)
            }
            RecordedTargets::Board(items) => {
                let selection = interaction.pcb_selections.entry(window).or_default();
                selection.sync(Some(document), revision);
                let held = selection.selection().clone();
                selection.put_selection(board_selection(items));
                (None, Some(held))
            }
        };
        let task = self.dispatch_on_targets(command, targets, Some(window));
        if let Some(held) = held_sheet {
            let kept = self.still_on_sheet(document, held);
            if let Some(canvas) = self.interaction_state.canvases.get_mut(&window) {
                canvas.selected = kept;
                canvas.clear_overlay_cache();
            }
        }
        if let Some(held) = held_board {
            let revision = self.board_revision(document);
            if let Some(selection) = self.interaction_state.pcb_selections.get_mut(&window) {
                selection.sync(Some(document), revision);
                selection.put_selection(held);
            }
            self.refresh_undocked_pcb_highlights(document);
        }
        self.refresh_window_panel_ctxs(None);
        task
    }

    /// The command's own handler on the selection now holding the targets.
    /// Duplicate finishes here, at once and off the clipboard.
    fn dispatch_on_targets(
        &mut self,
        command: &AppCommandId,
        targets: &RecordedTargets,
        window: Option<iced::window::Id>,
    ) -> Task<Message> {
        if command.as_str() != "duplicate" {
            return self
                .dispatch_command_reporting(command, CommandArgs::none(), window)
                .1;
        }
        let mut operands = Map::new();
        operands.insert(RECORDED_TARGETS.to_string(), targets.to_operand());
        let run = |app: &mut Self| {
            let ran = match window {
                Some(id) => {
                    let mut ran = false;
                    let _done = app.run_in_window_document(id, |app| {
                        ran = app.duplicate_keeping_clipboard();
                        Task::none()
                    });
                    ran
                }
                None => app.duplicate_keeping_clipboard(),
            };
            let outcome = if ran {
                CommandOutcome::Ran
            } else {
                refused(no_change())
            };
            (outcome, Task::none())
        };
        self.logged_command_on(command, operands, Some(targets.clone()), window, run)
            .1
    }

    /// Make the project's designators ready for a Duplicate on `document`.
    /// Project-wide numbering is read from the open sheets only, as a replay
    /// cannot wait for a disk read: `false` when one of them is not open.
    fn read_duplicate_numbers(&mut self, document: &Path) -> bool {
        let mode = self
            .interaction_state
            .paste_designators_override
            .unwrap_or(self.ui_state.paste_designators);
        if !mode.reads_project() {
            return true;
        }
        let Some(references) = self.open_project_references(document) else {
            return false;
        };
        self.interaction_state.paste_counters = Some(PasteCounters {
            target: document.to_path_buf(),
            counters: seed_counters(references.iter().map(String::as_str)),
            used: references.iter().map(|r| r.trim().to_owned()).collect(),
            ..PasteCounters::default()
        });
        true
    }

    /// Every numbered designator of `document`'s project, walking its pages
    /// and child sheets. `None` when one of those sheets is not open.
    fn open_project_references(&self, document: &Path) -> Option<Vec<String>> {
        let plan = plan_project_sheets_for(&self.document_state, document);
        let mut queue: Vec<PathBuf> = plan.pages().to_vec();
        queue.push(plan.root().to_path_buf());
        queue.push(document.to_path_buf());
        let mut seen = HashSet::new();
        let mut references = Vec::new();
        while let Some(path) = queue.pop() {
            if !seen.insert(path.clone()) {
                continue;
            }
            let sheet = self.document_state.engines.get(&path)?.document();
            references.extend(
                sheet
                    .symbols
                    .iter()
                    .filter(|s| !s.is_power && !s.reference.starts_with('#'))
                    .map(|s| s.reference.clone()),
            );
            let folder = path.parent()?;
            for child in &sheet.child_sheets {
                queue.push(resolve_child_reference_quiet(folder, &child.filename).ok()?);
            }
        }
        Some(references)
    }

    /// Duplicate the selection beside itself; the person's clipboards stay.
    fn duplicate_keeping_clipboard(&mut self) -> bool {
        let state = &self.interaction_state;
        let held = (
            state.clipboard.clone(),
            state.clipboard_text.clone(),
            state.pcb_clipboard.clone(),
        );
        let ran = if self.has_active_pcb() {
            // The task would write the system clipboard: it is not run.
            self.duplicate_pcb_selection().is_some()
        } else if self.handle_selection_copy_requested() == CopyOutcome::Carried {
            self.paste_with_offset(DEFAULT_PASTE_OFFSET_MM, DEFAULT_PASTE_OFFSET_MM);
            true
        } else {
            false
        };
        let state = &mut self.interaction_state;
        (state.clipboard, state.clipboard_text, state.pcb_clipboard) = held;
        ran
    }

    /// Refuse a Replay: a new failed row, and a Messages line because a
    /// paused log or a filter can hide that row. The new row keeps the input,
    /// so it can be replayed once the cause is fixed.
    pub(super) fn refuse_replay(&mut self, row: &LogRow, reason: Translatable) {
        let reason = reason.render_en_us();
        crate::diagnostics::log_warning(&reason);
        let input = self.ui_state.command_replay.input_of(row.seq);
        let guard = self.ui_state.command_replay.target_guard_of(row.seq);
        self.record_refused_replay(row, reason);
        if let Some(input) = input {
            self.carry_replay_input(row, &input);
        }
        let log = &self.ui_state.command_log.log;
        let oldest = log.rows().next().map(|new| new.seq);
        let newest = log.rows().next_back().map(|new| new.seq);
        if let (Some(guard), Some(seq)) = (guard, newest.filter(|seq| *seq > row.seq)) {
            self.ui_state
                .command_replay
                .keep_target_guard(seq, guard, oldest);
        }
    }

    /// The newest row, when it is newer than `row`, takes its input and
    /// document: a replay may run on a document that is not in view.
    pub(super) fn carry_replay_input(&mut self, row: &LogRow, input: &ReplayInput) {
        let log = &mut self.ui_state.command_log.log;
        let newest = log.rows().next_back().map(|new| new.seq);
        if let Some(seq) = newest.filter(|seq| *seq > row.seq) {
            log.set_document(seq, row.document.clone());
            self.set_replay_input(seq, input.clone());
        }
    }
}

pub(super) fn document_name(document: &Path) -> String {
    document.file_name().map_or_else(
        || document.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

pub(super) fn document_closed(document: &Path) -> Translatable {
    Translatable::new(ids::COMMAND_LOG_REPLAY_DOCUMENT_CLOSED)
        .arg("document", document_name(document))
}

pub(super) fn targets_gone(document: &Path, missing: &[String]) -> Translatable {
    Translatable::new(ids::COMMAND_LOG_REPLAY_TARGET_GONE)
        .arg("count", missing.len())
        .arg("document", document_name(document))
        .arg("missing", missing.join(", "))
}

pub(super) fn tool_active(document: &Path) -> Translatable {
    Translatable::new(ids::COMMAND_LOG_REPLAY_TOOL_ACTIVE).arg("document", document_name(document))
}

pub(super) fn document_busy(document: &Path) -> Translatable {
    Translatable::new(ids::COMMAND_LOG_REPLAY_DOCUMENT_BUSY)
        .arg("document", document_name(document))
}

fn numbers_unread(document: &Path) -> Translatable {
    Translatable::new(ids::COMMAND_LOG_REPLAY_NUMBERS_UNREAD)
        .arg("document", document_name(document))
}

pub(super) fn no_change() -> Translatable {
    Translatable::new(ids::COMMAND_LOG_REPLAY_NO_CHANGE)
}

pub(super) fn refused(reason: Translatable) -> CommandOutcome {
    CommandOutcome::Disabled { reason }
}

/// A refusal in the words of the engine or handler that made it.
pub(super) fn refused_with(reason: String) -> CommandOutcome {
    refused(Translatable::new(ids::COMMAND_LOG_REPLAY_REFUSED).arg("reason", reason))
}
