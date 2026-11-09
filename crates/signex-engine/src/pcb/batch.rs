//! Net edits as a board command, and a dry run of a whole batch.

use std::collections::BTreeSet;

use signex_net::board::BoardNetEdit;
use signex_types::pcb::PcbBoard;

use super::command::{Applied, BoardCommand, BoardPatch, BoardPatchKind};
use super::error::BoardCommandError;
use super::items::BoardItemId;

/// Apply a net edit; the inverse is the edit `BoardNetEdit::apply` returns.
/// The patch names the footprints and zones the edit actually wrote.
pub(super) fn set_nets(board: &mut PcbBoard, edit: BoardNetEdit) -> Applied {
    let inverse = edit.apply(board);
    let touched: BTreeSet<BoardItemId> = inverse
        .pads
        .iter()
        .map(|change| BoardItemId::Footprint(change.footprint))
        .chain(inverse.zones.iter().map(|z| BoardItemId::Zone(z.zone)))
        .collect();
    Applied {
        inverse: BoardCommand::SetNets(inverse),
        patch: BoardPatch {
            kind: BoardPatchKind::NetsChanged,
            items: touched.into_iter().collect(),
        },
    }
}

/// The first command of a batch the board would refuse, and why.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("command {index} of the batch is refused: {error}")]
pub struct BatchRefusal {
    pub index: usize,
    pub error: BoardCommandError,
}

/// Would `commands`, run in order, apply to `board`? Runs them on a copy, so
/// `board` is never written. A refusal names the first command refused.
pub fn check_batch(board: &PcbBoard, commands: &[BoardCommand]) -> Result<(), BatchRefusal> {
    let mut scratch = board.clone();
    for (index, command) in commands.iter().enumerate() {
        command
            .clone()
            .apply(&mut scratch)
            .map_err(|error| BatchRefusal { index, error })?;
    }
    Ok(())
}
