//! Replay of a recorded edit: the same engine commands, offset or editor
//! messages on the same items, as one undo step and one new row.

use std::path::Path;

use iced::Task;
use pcb_geom::ItemRef;
use signex_engine::Command;
use signex_engine::pcb::{BoardCommand, placement};
use signex_i18n::{RenderTranslatable, Translatable, ids};
use signex_types::pcb::Point;

use super::book::ReplayInput;
use super::run::{document_busy, document_closed, no_change, targets_gone, tool_active};
use super::selection::Shown;
use crate::app::command::CommandOutcome;
use crate::app::core::command_log::RecordedTargets;
use crate::app::core::mutation_outcome::MutationOutcome;
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;
use crate::panels::command_log::LogRow;

/// `commands` with a new id for a union they create.
fn with_new_union_ids(commands: &[Command]) -> Vec<Command> {
    let mut commands = commands.to_vec();
    for command in &mut commands {
        if let Command::CreateUnion { union } = command {
            union.uuid = uuid::Uuid::now_v7();
        }
    }
    commands
}

fn board_outcome<E: std::fmt::Display>(
    result: Result<Vec<signex_engine::pcb::BoardPatch>, E>,
) -> CommandOutcome {
    match result {
        Ok(patches) if patches.is_empty() => CommandOutcome::disabled(no_change()),
        Ok(_) => CommandOutcome::Ran,
        Err(error) => CommandOutcome::disabled(error.to_string()),
    }
}

impl Signex {
    /// Run `input` again, or refuse with what is missing and change nothing.
    pub(super) fn replay_recorded_input(
        &mut self,
        row: &LogRow,
        command: &AppCommandId,
        input: &ReplayInput,
    ) -> Task<Message> {
        if let Some(reason) = self.recorded_input_refusal(row, input) {
            self.refuse_replay(row, reason);
            return Task::none();
        }
        self.ui_state.command_replay.replaying = true;
        let (_, task) = self.logged_command_on(command, row.operands.clone(), None, None, |app| {
            app.run_recorded_input(row, input)
        });
        self.ui_state.command_replay.replaying = false;
        self.carry_replay_input(row, input);
        task
    }

    /// Why `input` cannot run now.
    fn recorded_input_refusal(&self, row: &LogRow, input: &ReplayInput) -> Option<Translatable> {
        if matches!(input, ReplayInput::MadeNewObjects) {
            return Some(
                Translatable::new(ids::COMMAND_LOG_REPLAY_MADE_OBJECTS)
                    .arg("command", row.label.clone()),
            );
        }
        if let ReplayInput::Editor { path, targets, .. } = input {
            return match self.missing_editor_targets(path, targets) {
                None => Some(document_closed(path)),
                Some(missing) if !missing.is_empty() => Some(targets_gone(path, &missing)),
                Some(_) => None,
            };
        }
        let Some(document) = row.document.as_deref() else {
            return Some(Translatable::new(ids::COMMAND_LOG_REPLAY_NO_TARGETS));
        };
        let Some(shown) = self.where_shown(document) else {
            return Some(document_closed(document));
        };
        let missing = self.missing_targets(document, &input_targets(input));
        if !missing.is_empty() {
            return Some(targets_gone(document, &missing));
        }
        // A sheet behind the one in view needs a tab switch, which would end
        // the tool. A board edit runs by path and needs none.
        let needs_switch =
            matches!(shown, Shown::Behind { .. }) && matches!(input, ReplayInput::Sheet { .. });
        (needs_switch && self.tool_in_progress()).then(|| tool_active(document))
    }

    fn run_recorded_input(
        &mut self,
        row: &LogRow,
        input: &ReplayInput,
    ) -> (CommandOutcome, Task<Message>) {
        let document = row.document.as_deref();
        match (input, document) {
            (ReplayInput::Sheet { commands, .. }, Some(document)) => {
                (self.run_sheet_commands(document, commands), Task::none())
            }
            (ReplayInput::Board { commands, .. }, Some(document)) => {
                (self.run_board_commands(document, commands), Task::none())
            }
            (
                ReplayInput::BoardMove {
                    footprints,
                    delta,
                    leaves_tracks,
                },
                Some(document),
            ) => (
                self.run_board_move(document, footprints, *delta, *leaves_tracks),
                Task::none(),
            ),
            (
                ReplayInput::Editor {
                    path,
                    targets,
                    messages,
                },
                _,
            ) => self.run_editor_input(path, targets, messages),
            _ => (CommandOutcome::disabled(no_change()), Task::none()),
        }
    }

    fn run_sheet_commands(&mut self, document: &Path, commands: &[Command]) -> CommandOutcome {
        let commands = with_new_union_ids(commands);
        match self.where_shown(document) {
            Some(Shown::Main) => {
                let held = self.take_main_selection();
                let outcome = self.apply_recorded_sheet(commands);
                self.give_back_main_selection(held);
                outcome
            }
            Some(Shown::Window(window)) => self.run_sheet_in_window(document, commands, window),
            Some(Shown::Behind { back }) => {
                let held = self.take_main_selection();
                if !self.focus_open_sheet(document) {
                    self.give_back_main_selection(held);
                    return CommandOutcome::disabled(document_busy(document).render_en_us());
                }
                let outcome = self.apply_recorded_sheet(commands);
                self.clear_main_selection();
                self.return_to_tab(back.as_ref());
                self.give_back_main_selection(held);
                outcome
            }
            None => CommandOutcome::disabled(document_closed(document).render_en_us()),
        }
    }

    fn run_sheet_in_window(
        &mut self,
        document: &Path,
        commands: Vec<Command>,
        window: iced::window::Id,
    ) -> CommandOutcome {
        let mut outcome = CommandOutcome::disabled(no_change());
        let _done = self.run_in_window_document(window, |app| {
            outcome = app.apply_recorded_sheet(commands);
            Task::none()
        });
        let held = self
            .interaction_state
            .canvases
            .get(&window)
            .map(|canvas| canvas.selected.clone())
            .unwrap_or_default();
        let kept = self.still_on_sheet(document, held);
        if let Some(canvas) = self.interaction_state.canvases.get_mut(&window) {
            canvas.selected = kept;
            canvas.clear_overlay_cache();
        }
        outcome
    }

    /// One engine batch: one undo step.
    fn apply_recorded_sheet(&mut self, commands: Vec<Command>) -> CommandOutcome {
        let (outcome, refusal) = self.apply_engine_commands_reporting(commands, true, true);
        match outcome {
            MutationOutcome::Changed => CommandOutcome::Ran,
            MutationOutcome::Unchanged | MutationOutcome::Failed => {
                CommandOutcome::disabled(refusal.unwrap_or_else(no_change))
            }
        }
    }

    fn run_board_commands(&mut self, document: &Path, commands: &[BoardCommand]) -> CommandOutcome {
        let result = self.execute_board_commands(document, commands.to_vec(), None);
        let outcome = board_outcome(result);
        self.repaint_board_selection(document);
        outcome
    }

    /// The recorded offset on the footprints as they are now.
    fn run_board_move(
        &mut self,
        document: &Path,
        footprints: &std::collections::BTreeSet<uuid::Uuid>,
        delta: Point,
        leaves_tracks: bool,
    ) -> CommandOutcome {
        let Some(board) = self.document_state.pcb_board(document) else {
            return CommandOutcome::disabled(document_closed(document).render_en_us());
        };
        let built = if leaves_tracks {
            placement::move_footprints_only(board, footprints, delta)
        } else {
            placement::move_commands(board, footprints, delta)
        };
        match built {
            Ok(commands) => self.run_board_commands(document, &commands),
            Err(refusal) => CommandOutcome::disabled(refusal.to_string()),
        }
    }

    fn repaint_board_selection(&mut self, document: &Path) {
        if self.active_tab_path().as_deref() == Some(document) {
            self.refresh_pcb_highlight();
        }
    }
}

/// The items `input` acts on, to check they are all still there.
fn input_targets(input: &ReplayInput) -> RecordedTargets {
    match input {
        ReplayInput::Sheet { targets, .. } => RecordedTargets::Sheet(targets.clone()),
        ReplayInput::Board { targets, .. } => RecordedTargets::Board(targets.clone()),
        ReplayInput::BoardMove { footprints, .. } => RecordedTargets::Board(
            footprints
                .iter()
                .map(|id| ItemRef::Footprint(*id))
                .collect(),
        ),
        ReplayInput::Editor { .. } | ReplayInput::MadeNewObjects => {
            RecordedTargets::Sheet(Vec::new())
        }
    }
}
