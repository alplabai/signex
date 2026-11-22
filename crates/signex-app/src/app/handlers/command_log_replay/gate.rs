//! The one gate every Replay passes first (#2840): a row runs only if its
//! command is on the safe list and its proof holds. Rules:
//! `docs/COMMAND_LOG.md`, "Replay".

use std::path::Path;
use std::sync::Arc;

use pcb_geom::ItemRef;
use signex_engine::pcb::placement::grid_step_mm;
use signex_engine::pcb::{BoardClipboard, BoardItemId, BoardReplayGuard};
use signex_i18n::{Translatable, ids};
use signex_types::pcb::Point;
use uuid::Uuid;

use super::book::ReplayInput;
use super::rebase;
use super::run::document_name;
use super::safe_list::{ReplayProof, replay_proof};
use super::selection::board_selection;
use super::sheet_guard::{SheetTargetGuard, TargetGuard, copy_would_stack};
use crate::app::Signex;
use crate::app::core::command_log::RecordedTargets;
use crate::app::handlers::clipboard_workflows::DEFAULT_PASTE_OFFSET_MM;
use crate::app::handlers::pcb_placement::deletable_items;
use crate::keymap::AppCommandId;
use crate::panels::command_log::LogRow;

/// The board object a selected item stands for: a pad or a designator is
/// its footprint. `None` for a room, which is no board item.
fn board_item_of(item: ItemRef) -> Option<BoardItemId> {
    Some(match item {
        ItemRef::Pad { footprint, .. }
        | ItemRef::FootprintText { footprint, .. }
        | ItemRef::Footprint(footprint) => BoardItemId::Footprint(footprint),
        ItemRef::Track(id) => BoardItemId::Segment(id),
        ItemRef::Via(id) => BoardItemId::Via(id),
        ItemRef::Zone { zone, .. } => BoardItemId::Zone(zone),
        ItemRef::Text(id) => BoardItemId::Text(id),
        ItemRef::Arc(id) => BoardItemId::Arc(id),
        ItemRef::FreePad(id) => BoardItemId::FreePad(id),
        ItemRef::Dimension(id) => BoardItemId::Dimension(id),
        ItemRef::Graphic { graphic, .. } => BoardItemId::Graphic(graphic),
        ItemRef::Room(_) => return None,
    })
}

fn said(id: signex_i18n::MessageId, row: &LogRow) -> Translatable {
    Translatable::new(id).arg("command", row.label.clone())
}

/// The items the `ids` operand names; `None` when it names none or reads
/// otherwise than as ids.
fn named_ids(row: &LogRow) -> Option<Vec<Uuid>> {
    let text = row.operands.get("ids")?.as_str()?;
    let ids = text
        .split(',')
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(Uuid::parse_str)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    (!ids.is_empty()).then_some(ids)
}

impl Signex {
    /// A row with operands that ran keeps what it wrote on a board.
    pub(super) fn keep_board_operand_guard(&mut self, seq: u64, guard: &BoardReplayGuard) {
        let log = &self.ui_state.command_log.log;
        if !log.row(seq).is_some_and(LogRow::has_given_operands) {
            return;
        }
        let oldest = log.rows().next().map(|row| row.seq);
        self.ui_state
            .command_replay
            .keep_operand_guard(seq, Arc::new(guard.clone()), oldest);
    }

    /// What `targets` hold before `command` runs, for a Delete or Duplicate.
    pub(in crate::app::handlers) fn read_target_guard(
        &self,
        command: &AppCommandId,
        document: Option<&Path>,
        targets: Option<&RecordedTargets>,
    ) -> Option<TargetGuard> {
        if replay_proof(command.as_str()) != Some(ReplayProof::TargetsAsFound) {
            return None;
        }
        let document = document?;
        Some(match targets? {
            RecordedTargets::Sheet(items) => {
                let engine = self.document_state.engines.get(document)?;
                TargetGuard::Sheet(SheetTargetGuard::of_items(engine.document(), items))
            }
            RecordedTargets::Board(items) => {
                let board = self.document_state.pcb_board(document)?;
                let named: Option<Vec<BoardItemId>> =
                    items.iter().map(|item| board_item_of(*item)).collect();
                TargetGuard::Board(named.map_or_else(BoardReplayGuard::uncheckable, |named| {
                    BoardReplayGuard::of_items(board, &named)
                }))
            }
        })
    }

    pub(in crate::app::handlers) fn keep_target_guard(&mut self, seq: u64, guard: TargetGuard) {
        let oldest = self.ui_state.command_log.log.rows().next().map(|r| r.seq);
        self.ui_state
            .command_replay
            .keep_target_guard(seq, Arc::new(guard), oldest);
    }

    /// Whether a Delete or Duplicate target differs from what the row found.
    /// Run once every target is known to be there.
    pub(super) fn targets_changed(&self, row: &LogRow, document: &Path) -> bool {
        let guard = self.ui_state.command_replay.target_guard_of(row.seq);
        !guard.is_some_and(|guard| match guard.as_ref() {
            TargetGuard::Sheet(guard) => (self.document_state.engines.get(document))
                .is_some_and(|engine| guard.holds_as_found(engine.document())),
            TargetGuard::Board(guard) => (self.document_state.pcb_board(document))
                .is_some_and(|board| guard.holds_as_found(board)),
        })
    }

    /// Whether a Duplicate of `targets` would land on an object of the same
    /// kind already at its fixed offset. What cannot be read refuses.
    pub(super) fn duplicate_lands_on_a_copy(
        &self,
        targets: &RecordedTargets,
        document: &Path,
    ) -> bool {
        match targets {
            RecordedTargets::Sheet(items) => (self.document_state.engines.get(document))
                .is_none_or(|engine| {
                    copy_would_stack(engine.document(), items, DEFAULT_PASTE_OFFSET_MM)
                }),
            RecordedTargets::Board(items) => {
                let Some(board) = self.document_state.pcb_board(document) else {
                    return true;
                };
                let named = deletable_items(&board_selection(items));
                let Ok(Some(copy)) = BoardClipboard::copy(board, &named, grid_step_mm(board))
                else {
                    return true;
                };
                let origin = copy.origin();
                let shifted = Point::new(
                    origin.x + DEFAULT_PASTE_OFFSET_MM,
                    origin.y + DEFAULT_PASTE_OFFSET_MM,
                );
                let at = self.board_grid_point(board, copy.grid_use(), shifted);
                copy.lands_on_a_copy(board, at)
            }
        }
    }

    /// Refuse the Replay of `row` unless it is proven safe. `true` when
    /// refused: a new failed row, which keeps the guard so it can run once
    /// its target is back.
    pub(in crate::app::handlers) fn refuse_unproven_replay(&mut self, row: &LogRow) -> bool {
        let Some(reason) = self.replay_refusal(row) else {
            return false;
        };
        let guard = self.ui_state.command_replay.operand_guard_of(row.seq);
        self.refuse_replay(row, reason);
        let log = &self.ui_state.command_log.log;
        let oldest = log.rows().next().map(|new| new.seq);
        let newest = log.rows().next_back().map(|new| new.seq);
        if let (Some(guard), Some(seq)) = (guard, newest.filter(|seq| *seq > row.seq)) {
            self.ui_state
                .command_replay
                .keep_operand_guard(seq, guard, oldest);
        }
        true
    }

    fn replay_refusal(&self, row: &LogRow) -> Option<Translatable> {
        let Some(proof) = replay_proof(row.command.as_str()) else {
            return Some(said(ids::COMMAND_LOG_REPLAY_NOT_REPLAYABLE, row));
        };
        match proof {
            ReplayProof::RecordedObjects => self.recorded_objects_refusal(row),
            ReplayProof::RecordedMove => self.recorded_move_refusal(row),
            ReplayProof::TargetsAsFound => self.targets_refusal(row),
            ReplayProof::BoardValue | ReplayProof::BoardItems | ReplayProof::BoardAsFound => {
                self.board_operands_refusal(row, proof)
            }
        }
    }

    /// A move on a sheet or board with its guard. A library editor's drag
    /// names items by index and has none. With no input the tool never
    /// finished, which the replay itself says. The state check runs with the
    /// input's other refusals, after a gone target is named.
    fn recorded_move_refusal(&self, row: &LogRow) -> Option<Translatable> {
        let input = self.ui_state.command_replay.input_of(row.seq);
        let proven = !row.has_given_operands()
            && match input.as_deref() {
                None => true,
                Some(ReplayInput::Sheet { guard, .. }) => guard.is_some(),
                Some(ReplayInput::BoardMove { guard, .. }) => {
                    guard.as_ref().is_some_and(BoardReplayGuard::is_checkable)
                }
                Some(ReplayInput::Board { guard, .. }) => guard.is_checkable(),
                Some(_) => false,
            };
        (!proven).then(|| said(ids::COMMAND_LOG_REPLAY_NOT_REPLAYABLE, row))
    }

    /// A Delete or Duplicate on recorded sheet or board targets. An editor's
    /// row holds an input instead and has no guard. A row with no targets
    /// goes on to the refusal that says so.
    fn targets_refusal(&self, row: &LogRow) -> Option<Translatable> {
        let book = &self.ui_state.command_replay;
        if row.has_given_operands() || book.input_of(row.seq).is_some() {
            return Some(said(ids::COMMAND_LOG_REPLAY_NOT_REPLAYABLE, row));
        }
        self.ui_state.command_log.log.targets_of(row.seq)?;
        let checkable = book
            .target_guard_of(row.seq)
            .is_some_and(|guard| guard.is_checkable());
        (!checkable).then(|| said(ids::COMMAND_LOG_REPLAY_NOTHING_TO_CHECK, row))
    }

    /// Only whole objects with a check; that check runs with the input's
    /// other refusals ([`Self::replay_recorded_input`]).
    fn recorded_objects_refusal(&self, row: &LogRow) -> Option<Translatable> {
        let input = self.ui_state.command_replay.input_of(row.seq);
        let proven = !row.has_given_operands()
            && match input.as_deref() {
                Some(ReplayInput::Sheet { commands, .. }) => {
                    rebase::only_replaces_objects(commands)
                }
                Some(ReplayInput::Board { guard, .. }) => guard.is_checkable(),
                _ => false,
            };
        (!proven).then(|| said(ids::COMMAND_LOG_REPLAY_NOT_REPLAYABLE, row))
    }

    fn board_operands_refusal(&self, row: &LogRow, proof: ReplayProof) -> Option<Translatable> {
        if !row.has_given_operands() {
            return Some(said(ids::COMMAND_LOG_REPLAY_NOT_REPLAYABLE, row));
        }
        let nothing_to_check = || Some(said(ids::COMMAND_LOG_REPLAY_NOTHING_TO_CHECK, row));
        let Some(guard) = self.ui_state.command_replay.operand_guard_of(row.seq) else {
            return nothing_to_check();
        };
        if !guard.is_checkable() {
            return nothing_to_check();
        }
        if proof == ReplayProof::BoardItems {
            let covered = named_ids(row).is_some_and(|ids| ids.iter().all(|id| guard.covers(*id)));
            if !covered {
                return nothing_to_check();
            }
        }
        let board = (row.document.as_deref()).and_then(|path| self.document_state.pcb_board(path));
        if board.is_some_and(|board| guard.drops_layer_sizes(board)) {
            let document =
                (row.document.as_deref()).map_or_else(|| row.label.clone(), document_name);
            return Some(
                said(ids::COMMAND_LOG_REPLAY_DROPS_LAYER_SIZES, row).arg("document", document),
            );
        }
        let holds = board.is_some_and(|board| match proof {
            ReplayProof::BoardAsFound => guard.holds_as_found(board),
            _ => guard.holds(board),
        });
        if holds {
            return None;
        }
        let document = (row.document.as_deref()).map_or_else(|| row.label.clone(), document_name);
        Some(said(ids::COMMAND_LOG_REPLAY_STATE_CHANGED, row).arg("document", document))
    }
}
