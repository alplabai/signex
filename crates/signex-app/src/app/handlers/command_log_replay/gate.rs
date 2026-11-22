//! The one gate every Replay passes first (#2840): a row runs only if its
//! command is on the safe list and its proof holds. Rules:
//! `docs/COMMAND_LOG.md`, "Replay".

use std::sync::Arc;

use signex_engine::pcb::BoardReplayGuard;
use signex_i18n::{Translatable, ids};
use uuid::Uuid;

use super::book::ReplayInput;
use super::rebase;
use super::run::document_name;
use super::safe_list::{ReplayProof, replay_proof};
use crate::app::Signex;
use crate::panels::command_log::LogRow;

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
            ReplayProof::BoardValue | ReplayProof::BoardItems | ReplayProof::BoardAsFound => {
                self.board_operands_refusal(row, proof)
            }
        }
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
