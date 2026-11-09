//! Net edits as a board command, and a dry run of a whole batch.

use std::collections::BTreeSet;

use signex_net::board::{BoardNetEdit, NetEditRefusal};
use signex_types::pcb::PcbBoard;

use super::command::{Applied, BoardCommand, BoardPatch, BoardPatchKind};
use super::engine::BoardEngine;
use super::error::BoardCommandError;
use super::items::BoardItemId;

/// Apply a net edit, refused whole when it names a footprint, pad or zone
/// the board does not hold, or one twice, so a stale list is never a silent
/// no-op. The inverse is the edit `BoardNetEdit::apply` returns: only what
/// was written, all of it present again when undo runs.
pub(super) fn set_nets(
    board: &mut PcbBoard,
    edit: BoardNetEdit,
) -> Result<Applied, BoardCommandError> {
    edit.check(board).map_err(refusal)?;
    let inverse = edit.apply(board);
    let touched: BTreeSet<BoardItemId> = inverse
        .pads
        .iter()
        .map(|change| BoardItemId::Footprint(change.footprint))
        .chain(inverse.zones.iter().map(|z| BoardItemId::Zone(z.zone)))
        .collect();
    Ok(Applied {
        inverse: BoardCommand::SetNets(inverse),
        patch: BoardPatch {
            kind: BoardPatchKind::NetsChanged,
            items: touched.into_iter().collect(),
        },
    })
}

fn refusal(why: NetEditRefusal) -> BoardCommandError {
    use BoardCommandError as E;
    match why {
        NetEditRefusal::FootprintNotFound(id) => E::FootprintNotFound(id),
        NetEditRefusal::PadNotFound { footprint, pad } => E::PadNotFound { footprint, pad },
        NetEditRefusal::ZoneNotFound(id) => E::ItemNotFound(BoardItemId::Zone(id)),
        NetEditRefusal::DuplicatePad { footprint, pad } => E::DuplicatePadChange { footprint, pad },
        NetEditRefusal::DuplicateZone(id) => E::DuplicateItem(BoardItemId::Zone(id)),
    }
}

/// The first command of a batch the board would refuse, and why.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("command {index} of the batch is refused: {error}")]
pub struct BatchRefusal {
    pub index: usize,
    pub error: BoardCommandError,
}

impl BoardEngine {
    /// Would `commands`, run in order as one step, apply to this board? Runs
    /// them on a copy, so nothing here changes. A lease refusal names the
    /// first command; any other names the first command refused.
    pub fn check_batch(&self, commands: &[BoardCommand]) -> Result<(), BatchRefusal> {
        self.check_lease(commands)
            .map_err(|error| BatchRefusal { index: 0, error })?;
        let mut scratch = self.board().clone();
        for (index, command) in commands.iter().enumerate() {
            command
                .clone()
                .apply(&mut scratch)
                .map_err(|error| BatchRefusal { index, error })?;
        }
        Ok(())
    }
}
