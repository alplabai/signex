//! Replay of a row with operands that edited a board (#2840): one path for
//! every such command. Rules: `docs/COMMAND_LOG.md`, "Replay".

use std::sync::Arc;

use signex_engine::pcb::BoardReplayGuard;
use signex_i18n::{Translatable, ids};

use super::run::document_name;
use super::scope::{BoardOperandReplay, board_operand_replay};
use crate::app::Signex;
use crate::panels::command_log::LogRow;

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

    /// Refuse the Replay of `row` when the board it edited has moved on, or
    /// it recorded no board state to check. `true` when refused: a new failed
    /// row, which keeps the guard so it refuses the same way.
    pub(in crate::app::handlers) fn refuse_stale_board_operands(&mut self, row: &LogRow) -> bool {
        let rule = board_operand_replay(row.command.as_str());
        if rule == Some(BoardOperandReplay::Recomputes) {
            return false;
        }
        let guard = self.ui_state.command_replay.operand_guard_of(row.seq);
        let carried = match guard {
            Some(guard) if self.board_operands_fit(row, &guard, rule) => return false,
            Some(guard) => guard,
            // It failed, so it wrote nothing: it runs and fails or succeeds.
            None if rule.is_none() || row.outcome.is_failed() => return false,
            None => Arc::new(BoardReplayGuard::uncheckable()),
        };
        let document = row
            .document
            .as_deref()
            .map_or_else(|| row.label.clone(), document_name);
        let reason = Translatable::new(ids::COMMAND_LOG_REPLAY_STATE_CHANGED)
            .arg("command", row.label.clone())
            .arg("document", document);
        self.refuse_replay(row, reason);
        let log = &self.ui_state.command_log.log;
        let oldest = log.rows().next().map(|new| new.seq);
        let newest = log.rows().next_back().map(|new| new.seq);
        if let Some(seq) = newest.filter(|seq| *seq > row.seq) {
            self.ui_state
                .command_replay
                .keep_operand_guard(seq, carried, oldest);
        }
        true
    }

    fn board_operands_fit(
        &self,
        row: &LogRow,
        guard: &BoardReplayGuard,
        rule: Option<BoardOperandReplay>,
    ) -> bool {
        let board = row
            .document
            .as_deref()
            .and_then(|path| self.document_state.pcb_board(path));
        board.is_some_and(|board| match rule {
            Some(BoardOperandReplay::SetsValue | BoardOperandReplay::Recomputes) => {
                guard.holds(board)
            }
            Some(BoardOperandReplay::OnlyAsFound) | None => guard.holds_as_found(board),
        })
    }
}
