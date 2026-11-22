//! Recording a finished edit: inside a logged command it becomes that row's
//! input, after a tool it goes to the row that started the tool, and a move
//! or Properties edit made without a command gets a row of its own.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use pcb_geom::ItemRef;
use serde_json::Map;
use signex_engine::Command;
use signex_engine::pcb::BoardCommand;
use signex_types::schematic::SelectedItem;

use super::book::{PendingTool, ReplayInput};
use super::scope::{EditFamily, ReplayScope, replay_scope};
use crate::app::Signex;
use crate::app::core::command_log::{RECORDED_INPUT, RecordedTargets};
use crate::keymap::AppCommandId;
use crate::panels::command_log::{LogOutcome, LogSource};

/// Keystrokes of one field this close together are one row.
const TYPING_RUN: Duration = Duration::from_secs(2);

/// Whether a sheet command makes an object with a new id.
fn makes_object(command: &Command) -> bool {
    matches!(
        command,
        Command::PlaceWireSegmentAsIs { .. }
            | Command::PlaceWireSegment { .. }
            | Command::PlaceBus { .. }
            | Command::PlaceLabel { .. }
            | Command::PlaceSymbol { .. }
            | Command::PlaceChildSheet { .. }
            | Command::PlaceSheetPin { .. }
            | Command::PlaceJunction { .. }
            | Command::PlaceNoConnect { .. }
            | Command::PlaceNoErcDirective { .. }
            | Command::PlaceBusEntry { .. }
            | Command::PlaceTextNote { .. }
            | Command::PlaceSchDrawing { .. }
            | Command::PlaceDirective { .. }
            | Command::PlaceHarnessConnector { .. }
            | Command::PlaceHarnessEntry { .. }
            | Command::PlaceHarnessWire { .. }
            | Command::BreakWire { .. }
    )
}

fn sheet_family(commands: &[Command]) -> EditFamily {
    let all = |test: fn(&Command) -> bool| commands.iter().all(test);
    if all(|c| {
        matches!(
            c,
            Command::MoveSelection { .. }
                | Command::MoveEach { .. }
                | Command::MoveSymbolAbsolute { .. }
                | Command::MoveWireEnd { .. }
                | Command::SetRunEnd { .. }
        )
    }) {
        EditFamily::Move
    } else if all(|c| {
        matches!(
            c,
            Command::CreateUnion { .. }
                | Command::UpdateUnion { .. }
                | Command::SetUnionMembers { .. }
                | Command::BreakUnion { .. }
        )
    }) {
        EditFamily::Union
    } else if all(is_property_edit) {
        EditFamily::Properties
    } else {
        EditFamily::Other
    }
}

fn is_property_edit(command: &Command) -> bool {
    matches!(
        command,
        Command::UpdateText { .. }
            | Command::UpdateLabelProps { .. }
            | Command::SetSymbolRotation { .. }
            | Command::SetSymbolLocked { .. }
            | Command::SetItemLocked { .. }
            | Command::SetSymbolDnp { .. }
            | Command::SetSymbolFlag { .. }
            | Command::SetWireWidth { .. }
            | Command::SetWireColor { .. }
            | Command::SetSymbolBodyStyle { .. }
            | Command::UpdateSymbolTextSize { .. }
            | Command::UpdateSymbolFootprint { .. }
            | Command::SetSymbolFootprintBinding { .. }
            | Command::UpdateSymbolFields { .. }
            | Command::SetSymbolField { .. }
            | Command::RemoveSymbolField { .. }
            | Command::UpdateTextNoteProps { .. }
            | Command::UpdateSchDrawing { .. }
            | Command::UpdateChildSheetStyle { .. }
            | Command::SetTextFontFamily { .. }
            | Command::SetDrawingPlacement { .. }
            | Command::UpdateDirective { .. }
            | Command::SetNoErcRules { .. }
            | Command::UpdateHarnessConnector { .. }
            | Command::SetHarnessEntryType { .. }
    )
}

/// The catalog command a row made by a sheet edit is named after.
fn sheet_row_command(family: EditFamily, commands: &[Command]) -> Option<&'static str> {
    match family {
        EditFamily::Move => Some("move_selection"),
        EditFamily::Properties => Some("edit_object_properties"),
        EditFamily::Union => Some(match commands.first() {
            Some(Command::CreateUnion { .. }) => "create_union",
            Some(Command::SetUnionMembers { .. }) => "add_to_union",
            Some(Command::BreakUnion { .. }) => "break_union",
            _ => "edit_object_properties",
        }),
        EditFamily::Other => None,
    }
}

/// The items a sheet edit names itself, beside the selection it ran on.
fn named_items(commands: &[Command]) -> Vec<SelectedItem> {
    commands
        .iter()
        .flat_map(|command| match command {
            Command::MoveSelection { items, .. }
            | Command::RotateSelection { items, .. }
            | Command::MirrorSelection { items, .. }
            | Command::TransformSelection { items, .. }
            | Command::DeleteSelection { items } => items.clone(),
            Command::MoveEach { moves, .. } => moves.iter().map(|(item, _, _)| *item).collect(),
            _ => Vec::new(),
        })
        .collect()
}

fn board_family(commands: &[BoardCommand], from_panel: bool) -> EditFamily {
    let all = |test: fn(&BoardCommand) -> bool| commands.iter().all(test);
    if all(|c| {
        matches!(
            c,
            BoardCommand::MoveFootprint { .. }
                | BoardCommand::Rotate { .. }
                | BoardCommand::Flip { .. }
        )
    }) {
        EditFamily::Move
    } else if from_panel
        || all(|c| {
            matches!(
                c,
                BoardCommand::SetFootprintText { .. }
                    | BoardCommand::SetFootprintProperty { .. }
                    | BoardCommand::SetFootprintComponent { .. }
                    | BoardCommand::SetFootprintLocked { .. }
                    | BoardCommand::SetItemsLocked { .. }
            )
        })
    {
        EditFamily::Properties
    } else {
        EditFamily::Other
    }
}

/// Whether a board batch adds objects, whose ids would collide on a replay.
fn board_makes_objects(commands: &[BoardCommand]) -> bool {
    commands.iter().any(|command| {
        matches!(
            command,
            BoardCommand::AddItems { .. }
                | BoardCommand::AddGraphics { .. }
                | BoardCommand::AddRoom { .. }
                | BoardCommand::PlaceRoom { .. }
        )
    })
}

/// The board items a batch names by footprint.
fn board_named_items(commands: &[BoardCommand]) -> Vec<ItemRef> {
    commands
        .iter()
        .filter_map(|command| match command {
            BoardCommand::MoveFootprint { footprint, .. }
            | BoardCommand::Rotate { footprint, .. }
            | BoardCommand::Flip { footprint }
            | BoardCommand::SetFootprintLocked { footprint, .. } => {
                Some(ItemRef::Footprint(*footprint))
            }
            _ => None,
        })
        .collect()
}

impl Signex {
    /// Whether a finished edit is the person's, to be recorded.
    pub(super) fn records_replay_input(&self) -> bool {
        !self.ui_state.command_replay.replaying
            && !self.agent.serving
            && !self.agent.origin.is_agent()
    }

    /// A copy of `commands` for [`Self::note_sheet_edit`], when recorded.
    pub(crate) fn sheet_edit_to_record(&self, commands: &[Command]) -> Option<Vec<Command>> {
        let whole_document = commands
            .iter()
            .any(|command| matches!(command, Command::ReplaceDocument { .. }));
        (self.records_replay_input() && !whole_document).then(|| commands.to_vec())
    }

    /// `commands` changed the sheet being edited.
    pub(crate) fn note_sheet_edit(&mut self, commands: Vec<Command>) {
        let Some(document) = self.document_state.edit_path().map(Path::to_path_buf) else {
            return;
        };
        let family = sheet_family(&commands);
        let row = sheet_row_command(family, &commands);
        let input = if commands.iter().any(makes_object) {
            ReplayInput::MadeNewObjects
        } else {
            let mut targets = self.interaction_state.active_canvas().selected.clone();
            if family == EditFamily::Move {
                targets.clear();
            }
            targets.extend(named_items(&commands));
            targets.sort_by_key(|item| item.uuid);
            targets.dedup();
            ReplayInput::Sheet { commands, targets }
        };
        self.note_finished_edit(document, family, row, input);
    }

    /// A copy of `commands` for [`Self::note_board_edit`], when recorded.
    pub(crate) fn board_edit_to_record(
        &self,
        commands: &[BoardCommand],
    ) -> Option<Vec<BoardCommand>> {
        self.records_replay_input().then(|| commands.to_vec())
    }

    /// `commands` changed the board at `path`.
    pub(crate) fn note_board_edit(&mut self, path: &Path, commands: Vec<BoardCommand>) {
        let book = &mut self.ui_state.command_replay;
        let from_panel = std::mem::take(&mut book.board_properties);
        let dropped = book.board_move.take();
        let family = board_family(&commands, from_panel);
        let row = match family {
            EditFamily::Move => Some("move_selection"),
            EditFamily::Properties => Some("edit_object_properties"),
            _ => None,
        };
        let input = match dropped {
            Some(input) => input,
            None if board_makes_objects(&commands) => ReplayInput::MadeNewObjects,
            None => self.board_input(path, commands),
        };
        self.note_finished_edit(path.to_path_buf(), family, row, input);
    }

    fn board_input(&self, path: &Path, commands: Vec<BoardCommand>) -> ReplayInput {
        let shown = self.active_tab_path().as_deref() == Some(path);
        let mut targets: Vec<ItemRef> = board_named_items(&commands);
        if shown {
            let selection = self.interaction_state.pcb_selection.selection();
            let edits_items = commands.iter().any(|command| {
                matches!(
                    command,
                    BoardCommand::ReplaceItems { .. } | BoardCommand::SetItemsLocked { .. }
                )
            });
            if edits_items {
                targets.extend(selection.items());
            }
        }
        targets.sort();
        targets.dedup();
        ReplayInput::Board { commands, targets }
    }

    /// Hand a finished edit to the dispatch running, the tool that started
    /// it, or a row of its own.
    pub(super) fn note_finished_edit(
        &mut self,
        document: PathBuf,
        family: EditFamily,
        row: Option<&'static str>,
        input: ReplayInput,
    ) {
        let book = &mut self.ui_state.command_replay;
        if let Some(captured) = book.capturing.as_mut() {
            captured.push(input);
            return;
        }
        let takes = book.pending.as_ref().is_some_and(|tool| {
            tool.takes.accepts(family) && tool.document.as_deref() == Some(document.as_path())
        });
        if takes && let Some(tool) = book.pending.take() {
            self.set_replay_input(tool.seq, input);
            return;
        }
        if matches!(input, ReplayInput::MadeNewObjects) {
            return;
        }
        if let Some(id) = row {
            self.record_edit_row(id, document, family, input);
        }
    }

    /// A row for an edit no command started; keystrokes of one field share
    /// one row.
    fn record_edit_row(
        &mut self,
        id: &'static str,
        document: PathBuf,
        family: EditFamily,
        input: ReplayInput,
    ) {
        if family == EditFamily::Properties
            && let Some(seq) = self.typing_run_row(id, &document, &input)
        {
            self.ui_state.command_replay.last_gesture = Some((seq, Instant::now()));
            self.set_replay_input(seq, input);
            return;
        }
        let source = self.edit_row_source();
        let Some(seq) = self.record_command(
            id,
            Map::new(),
            LogOutcome::Ok(None),
            Duration::ZERO,
            source,
            Some(document),
        ) else {
            return;
        };
        self.ui_state.command_replay.last_gesture = Some((seq, Instant::now()));
        self.set_replay_input(seq, input);
    }

    fn edit_row_source(&self) -> LogSource {
        self.dispatch_source(self.interaction_state.acting_window)
    }

    /// The newest row, when `input` is the next keystroke of its edit.
    fn typing_run_row(&self, id: &str, document: &Path, input: &ReplayInput) -> Option<u64> {
        let book = &self.ui_state.command_replay;
        let (seq, at) = book.last_gesture?;
        let log = &self.ui_state.command_log.log;
        let newest = log.rows().next_back()?;
        let same = newest.seq == seq
            && newest.command == id
            && newest.document.as_deref() == Some(document)
            && at.elapsed() < TYPING_RUN
            && book.input_of(seq)?.same_edit_as(input);
        same.then_some(seq)
    }

    /// Row `seq` took `input`: keep it and show it in the row's detail.
    pub(super) fn set_replay_input(&mut self, seq: u64, input: ReplayInput) {
        let log = &mut self.ui_state.command_log.log;
        log.set_recorded(seq, RECORDED_INPUT, input.summary());
        let oldest = log.rows().next().map(|row| row.seq);
        self.ui_state
            .command_replay
            .keep(seq, Arc::new(input), oldest);
        self.sync_command_log_panel_ctx();
    }

    /// An outermost command starts: record the edits it makes.
    pub(in crate::app::handlers) fn begin_replay_capture(&mut self) {
        if self.records_replay_input() {
            self.ui_state.command_replay.capturing = Some(Vec::new());
        }
    }

    /// It ended: what it edited, as one input.
    pub(in crate::app::handlers) fn end_replay_capture(&mut self) -> Option<ReplayCaptured> {
        let inputs = self.ui_state.command_replay.capturing.take()?;
        ReplayInput::merge_all(inputs).map(ReplayCaptured)
    }

    /// Row `seq` of `command` is logged: keep its targets or input, or wait
    /// for the tool it started.
    pub(in crate::app::handlers) fn keep_replay_input(
        &mut self,
        seq: u64,
        command: &AppCommandId,
        targets: Option<RecordedTargets>,
        captured: Option<ReplayCaptured>,
        ran: bool,
    ) {
        let scope = replay_scope(command.as_str());
        if let Some(targets) = targets {
            self.ui_state.command_log.log.keep_targets(seq, targets);
            self.sync_command_log_panel_ctx();
            return;
        }
        let takes_edit = matches!(scope, ReplayScope::OnTargets | ReplayScope::Continues(_));
        if takes_edit && let Some(ReplayCaptured(input)) = captured {
            self.set_replay_input(seq, input);
            return;
        }
        if let ReplayScope::Continues(takes) = scope
            && ran
            && self.records_replay_input()
        {
            let document = self
                .ui_state
                .command_log
                .log
                .row(seq)
                .and_then(|row| row.document.clone());
            self.ui_state.command_replay.pending = Some(PendingTool {
                seq,
                takes,
                document,
            });
        }
    }
}

/// The edits one dispatch made.
pub(crate) struct ReplayCaptured(ReplayInput);
