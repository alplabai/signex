use signex_net::board::BoardNetEdit;
use signex_types::pcb::{Footprint, PcbBoard, Point, SymbolLink};
use uuid::Uuid;

use super::batch;
use super::error::BoardCommandError;
use super::footprint::{self, FootprintFace};
use super::footprint_content::{self, FootprintText};
use super::items::{self, BoardItem, BoardItemId, RemovedItem};

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
    /// Append new items to the end of their lists. Refused when one is
    /// already on the board or named twice, when a position or angle is
    /// not a number, or when a pad uuid is already on the board. The inverse is a
    /// [`BoardCommand::DeleteItems`].
    AddItems { items: Vec<BoardItem> },
    /// Swap the footprint's pads, graphics and name for those of `with`, a
    /// library footprint in its top-side form. The board footprint keeps its
    /// uuid, position, rotation, side (a bottom one flips the template), lock,
    /// schematic link, designator and value, its library link when `with` has
    /// none, and its property placement and properties `with` lacks; the n-th
    /// new pad of a number takes the net and lock of the n-th old pad of it.
    /// Refused for a template not on a readable top-side layer, with a pad
    /// off the number line or a taken uuid, or a different count of a shared
    /// pad number. The
    /// inverse is a [`BoardCommand::RestoreFootprint`].
    ReplaceFootprint {
        footprint: Uuid,
        with: Box<Footprint>,
    },
    /// Put a whole footprint back over the one with its uuid; the inverse of
    /// [`BoardCommand::ReplaceFootprint`].
    RestoreFootprint { footprint: Box<Footprint> },
    /// Set the designator and value, and the Reference and Value property
    /// texts with them. The inverse is a [`BoardCommand::RestoreFootprintText`].
    SetFootprintText {
        footprint: Uuid,
        reference: String,
        value: String,
    },
    /// Put back the designator, value and property texts a text edit
    /// replaced. Refused if the footprint's property count changed since.
    RestoreFootprintText {
        footprint: Uuid,
        text: FootprintText,
    },
    /// Set or clear the footprint's schematic link (ADR-0005 §11).
    SetFootprintLink {
        footprint: Uuid,
        link: Option<SymbolLink>,
    },
    /// Write a net table and pad nets (`signex_net::board`); its inverse is
    /// the edit [`BoardNetEdit::apply`] returns. Refused whole when a change
    /// names a footprint, pad or zone that is gone, or one twice.
    SetNets(BoardNetEdit),
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
    ItemsAdded,
    FootprintReplaced,
    FootprintTextSet,
    FootprintLinkSet,
    /// The net table, and pad and zone nets. The items are the footprints
    /// and zones written; a table-only change names none.
    NetsChanged,
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
            Self::AddItems { items: added } => items::add(board, added),
            Self::ReplaceFootprint {
                footprint: id,
                with,
            } => footprint_content::replace(board, id, *with),
            Self::RestoreFootprint { footprint: fp } => footprint_content::restore(board, *fp),
            Self::SetFootprintText {
                footprint: id,
                reference,
                value,
            } => footprint_content::set_text(board, id, reference, value),
            Self::RestoreFootprintText {
                footprint: id,
                text,
            } => footprint_content::restore_text(board, id, text),
            Self::SetFootprintLink {
                footprint: id,
                link,
            } => footprint_content::set_link(board, id, link),
            Self::SetNets(edit) => batch::set_nets(board, edit),
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
