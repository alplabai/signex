use signex_types::pcb::{PcbBoard, Point};
use uuid::Uuid;

use super::error::BoardCommandError;
use super::footprint::{self, FootprintFace};
use super::items::{self, BoardItemId, RemovedItem};

/// One edit of a board. Targets are absolute so the inverse is exact.
#[derive(Debug, Clone, PartialEq)]
pub enum BoardCommand {
    /// Place the footprint's origin at `to` (board mm).
    MoveFootprint { footprint: Uuid, to: Point },
    /// Set the footprint's rotation about its origin to `degrees`.
    Rotate { footprint: Uuid, degrees: f64 },
    /// Move the footprint to the other side of the board with
    /// `pcb_geom::flip_footprint` (ADR-0005 §8): sided layers of the
    /// footprint, pads, graphics and properties swap and the rotation is
    /// stored as `(-rotation) mod 360`. Local geometry is untouched. The
    /// inverse is a [`BoardCommand::RestoreFace`], so undo is exact. The
    /// caller enforces `Footprint::locked` for move, rotate and flip.
    Flip { footprint: Uuid },
    /// Put back the rotation and sided layers a flip replaced; the inverse
    /// of [`BoardCommand::Flip`]. Refused if the footprint's pad, graphic
    /// or property count changed since the face was taken.
    RestoreFace {
        footprint: Uuid,
        face: FootprintFace,
    },
    /// Replace the board outline polygon.
    SetBoardOutline { outline: Vec<Point> },
    /// Remove the named items.
    DeleteItems { items: Vec<BoardItemId> },
    /// Put removed items back at their recorded list indices; the inverse
    /// of [`BoardCommand::DeleteItems`].
    RestoreItems { items: Vec<RemovedItem> },
}

/// What a command changed, for a consumer that redraws or re-derives only
/// what moved.
#[derive(Debug, Clone, PartialEq)]
pub struct BoardPatch {
    pub kind: BoardPatchKind,
    /// The items the command touched. Empty for an outline change.
    pub items: Vec<BoardItemId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardPatchKind {
    FootprintMoved,
    FootprintRotated,
    FootprintFlipped,
    OutlineReplaced,
    ItemsDeleted,
    ItemsRestored,
}

/// The result of applying a command: the command that undoes it and the
/// patch it made.
#[derive(Debug, Clone, PartialEq)]
pub struct Applied {
    pub inverse: BoardCommand,
    pub patch: BoardPatch,
}

impl BoardCommand {
    /// Apply the command to `board`. On error the board is unchanged.
    pub fn apply(self, board: &mut PcbBoard) -> Result<Applied, BoardCommandError> {
        match self {
            Self::MoveFootprint { footprint: id, to } => footprint::move_to(board, id, to),
            Self::Rotate {
                footprint: id,
                degrees,
            } => footprint::rotate_to(board, id, degrees),
            Self::Flip { footprint: id } => footprint::flip(board, id),
            Self::RestoreFace {
                footprint: id,
                face,
            } => footprint::restore_face(board, id, face),
            Self::SetBoardOutline { outline } => set_outline(board, outline),
            Self::DeleteItems { items: ids } => items::delete(board, &ids),
            Self::RestoreItems { items: removed } => items::restore(board, removed),
        }
    }
}

fn set_outline(board: &mut PcbBoard, outline: Vec<Point>) -> Result<Applied, BoardCommandError> {
    if outline.iter().any(|p| !p.x.is_finite() || !p.y.is_finite()) {
        return Err(BoardCommandError::NonFinite { field: "outline" });
    }
    let previous = std::mem::replace(&mut board.outline, outline);
    Ok(Applied {
        inverse: BoardCommand::SetBoardOutline { outline: previous },
        patch: BoardPatch {
            kind: BoardPatchKind::OutlineReplaced,
            items: Vec::new(),
        },
    })
}
