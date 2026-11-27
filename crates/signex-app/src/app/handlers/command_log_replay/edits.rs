//! Replay of a recorded edit: the same engine commands, offset or editor
//! messages on the same items, as one undo step and one new row.

use std::path::Path;

use iced::Task;
use pcb_geom::ItemRef;
use signex_engine::Command;
use signex_engine::pcb::placement::PlacementRefusal;
use signex_engine::pcb::{BoardCommand, BoardReplayGuard};
use signex_i18n::{Translatable, ids};

use super::book::ReplayInput;
use super::rebase;
use super::run::{
    document_busy, document_closed, document_name, no_change, refused, refused_with, targets_gone,
    tool_active,
};
use super::selection::Shown;
use crate::app::command::CommandOutcome;
use crate::app::core::command_log::RecordedTargets;
use crate::app::core::mutation_outcome::MutationOutcome;
use crate::app::handlers::pcb_item_move::{carry_of_drop, drag_commands};
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
        Ok(patches) if patches.is_empty() => refused(no_change()),
        Ok(_) => CommandOutcome::Ran,
        Err(error) => refused_with(error.to_string()),
    }
}

/// The commands a recorded board move builds on `board` as it is now: what
/// it carried by its offset, the tracks that follow bent at its recorded
/// corner (#2513), or left in place after a Ctrl drag, built as the drop
/// built them (`pcb_item_move::drag_commands`). `None` for another input.
fn rebuilt_board_move(
    board: &signex_types::pcb::PcbBoard,
    input: &ReplayInput,
) -> Option<Result<Vec<BoardCommand>, PlacementRefusal>> {
    let ReplayInput::BoardMove { drop, .. } = input else {
        return None;
    };
    let carry = carry_of_drop(drop);
    // A replay adds its corner legs as new tracks, so fresh ids.
    let mut ids = signex_types::object_id::UuidV7Ids;
    Some(drag_commands(board, &carry, drop.delta, &mut ids))
}

/// Whether a recorded board move no longer fits: a target left where the move
/// found or left it, or the offset would now also carry a track it never
/// wrote. A move the board refuses goes on to say why. The corner legs it
/// adds are new tracks and write over nothing.
fn board_move_is_stale(
    board: &signex_types::pcb::PcbBoard,
    input: &ReplayInput,
    guard: Option<&BoardReplayGuard>,
) -> bool {
    let Some(guard) = guard.filter(|guard| guard.holds(board)) else {
        return true;
    };
    rebuilt_board_move(board, input).is_some_and(|built| {
        built.is_ok_and(|commands| !guard.move_writes_only_its_targets(board, &commands))
    })
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
        if let ReplayInput::Editor { path, targets, .. }
        | ReplayInput::PadDrag { path, targets, .. } = input
        {
            return match self.missing_editor_targets(path, targets) {
                None => Some(document_closed(path)),
                Some(missing) if !missing.is_empty() => Some(targets_gone(path, &missing)),
                Some(_) => None,
            };
        }
        if let ReplayInput::EditorPanel { path, targets, .. } = input {
            return self.editor_panel_refusal(path, targets);
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
        let is_stale = match input {
            ReplayInput::Sheet {
                commands,
                pre_images,
                guard,
                ..
            } => (self.document_state.engines.get(document)).is_some_and(|e| {
                rebase::is_stale(e.document(), commands, pre_images)
                    || guard.as_ref().is_some_and(|g| !g.holds(e.document()))
            }),
            ReplayInput::Board { guard, .. } => {
                (self.document_state.pcb_board(document)).is_none_or(|board| !guard.holds(board))
            }
            ReplayInput::BoardMove { guard, .. } => (self.document_state.pcb_board(document))
                .is_none_or(|board| board_move_is_stale(board, input, guard.as_ref())),
            _ => false,
        };
        if is_stale {
            return Some(
                Translatable::new(ids::COMMAND_LOG_REPLAY_STATE_CHANGED)
                    .arg("command", row.label.clone())
                    .arg("document", document_name(document)),
            );
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
        match input {
            ReplayInput::Editor {
                path,
                targets,
                messages,
            } => self.run_editor_input(path, targets, messages),
            ReplayInput::PadDrag {
                path,
                targets,
                idx,
                start,
                to,
                with_selection,
            } => self.run_pad_drag(
                path,
                targets,
                *idx,
                (to.0 - start.0, to.1 - start.1),
                *with_selection,
            ),
            ReplayInput::EditorPanel {
                path,
                targets,
                message,
            } => self.run_editor_panel(path, targets, message),
            ReplayInput::FootprintSwap {
                footprints,
                template,
            } => match row.document.as_deref() {
                Some(document) => self.replay_footprint_swap(document, footprints, template),
                None => (refused(no_change()), Task::none()),
            },
            _ => {
                let outcome = match row.document.as_deref() {
                    Some(document) => self.run_document_input(document, input),
                    None => refused(no_change()),
                };
                (outcome, Task::none())
            }
        }
    }

    /// An input that runs on the row's sheet or board.
    fn run_document_input(&mut self, document: &Path, input: &ReplayInput) -> CommandOutcome {
        match input {
            ReplayInput::Sheet {
                commands,
                pre_images,
                ..
            } => {
                let engine = self.document_state.engines.get(document);
                let now = engine.and_then(|e| rebase::rebased(e.document(), commands, pre_images));
                match now {
                    Some(commands) => self.run_sheet_commands(document, &commands),
                    None => refused(no_change()),
                }
            }
            ReplayInput::Board { commands, .. } => self.run_board_commands(document, commands),
            ReplayInput::BoardMove { .. } => self.run_board_move(document, input),
            ReplayInput::ReuseBlock {
                name,
                anchor,
                degrees,
            } => match self.place_reuse_block(document, name, *anchor, *degrees) {
                Ok(_) => CommandOutcome::Ran,
                Err(why) => refused_with(why),
            },
            _ => refused(no_change()),
        }
    }

    /// A recorded Change Footprint on `document`, with what its step left
    /// to do.
    fn replay_footprint_swap(
        &mut self,
        document: &Path,
        footprints: &[uuid::Uuid],
        template: &signex_types::pcb::Footprint,
    ) -> (CommandOutcome, Task<Message>) {
        let before = self.board_revision(document);
        let task = self.swap_footprints(document, footprints, Ok(Box::new(template.clone())));
        let outcome = if self.board_revision(document) == before {
            refused(no_change())
        } else {
            CommandOutcome::Ran
        };
        self.repaint_board_selection(document);
        (outcome, task)
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
                    return refused(document_busy(document));
                }
                let outcome = self.apply_recorded_sheet(commands);
                self.clear_main_selection();
                self.return_to_tab(back.as_ref());
                self.give_back_main_selection(held);
                outcome
            }
            None => refused(document_closed(document)),
        }
    }

    fn run_sheet_in_window(
        &mut self,
        document: &Path,
        commands: Vec<Command>,
        window: iced::window::Id,
    ) -> CommandOutcome {
        let mut outcome = refused(no_change());
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
                refusal.map_or_else(|| refused(no_change()), refused_with)
            }
        }
    }

    fn run_board_commands(&mut self, document: &Path, commands: &[BoardCommand]) -> CommandOutcome {
        let result = self.execute_board_commands(document, commands.to_vec(), None);
        let outcome = board_outcome(result);
        self.repaint_board_selection(document);
        outcome
    }

    /// The recorded offset on the footprints or vias as they are now, the
    /// tracks on them bent at the recorded corner.
    fn run_board_move(&mut self, document: &Path, input: &ReplayInput) -> CommandOutcome {
        let Some(board) = self.document_state.pcb_board(document) else {
            return refused(document_closed(document));
        };
        match rebuilt_board_move(board, input) {
            Some(Ok(commands)) => self.run_board_commands(document, &commands),
            Some(Err(refusal)) => refused_with(refusal.to_string()),
            None => refused(no_change()),
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
        // A carried object or room that is gone fails its guard instead.
        ReplayInput::BoardMove { drop, .. } => RecordedTargets::Board(
            (drop.footprints.iter().map(|id| ItemRef::Footprint(*id)))
                .chain(drop.vias.iter().map(|id| ItemRef::Via(*id)))
                .collect(),
        ),
        ReplayInput::ReuseBlock { anchor, .. } => {
            RecordedTargets::Board(vec![ItemRef::Footprint(*anchor)])
        }
        ReplayInput::FootprintSwap { footprints, .. } => RecordedTargets::Board(
            footprints
                .iter()
                .map(|id| ItemRef::Footprint(*id))
                .collect(),
        ),
        ReplayInput::Editor { .. }
        | ReplayInput::PadDrag { .. }
        | ReplayInput::EditorPanel { .. }
        | ReplayInput::MadeNewObjects => RecordedTargets::Sheet(Vec::new()),
    }
}
