//! Move, rotate and flip one footprint. Pads, graphics and property texts
//! are stored in the footprint's local frame, so none of the three rewrites
//! them. Lock state is the caller's check, not this module's.

use signex_types::pcb::{Footprint, PcbBoard, Point};
use uuid::Uuid;

use super::command::{Applied, BoardCommand, BoardPatch, BoardPatchKind};
use super::error::BoardCommandError;
use super::items::BoardItemId;
use super::layer_side::swap_side_in_place;

fn find(board: &mut PcbBoard, id: Uuid) -> Result<&mut Footprint, BoardCommandError> {
    board
        .footprints
        .iter_mut()
        .find(|fp| fp.uuid == id)
        .ok_or(BoardCommandError::FootprintNotFound(id))
}

fn patch(kind: BoardPatchKind, id: Uuid) -> BoardPatch {
    BoardPatch {
        kind,
        items: vec![BoardItemId::Footprint(id)],
    }
}

pub(super) fn move_to(
    board: &mut PcbBoard,
    id: Uuid,
    to: Point,
) -> Result<Applied, BoardCommandError> {
    if !to.x.is_finite() || !to.y.is_finite() {
        return Err(BoardCommandError::NonFinite { field: "position" });
    }
    let fp = find(board, id)?;
    let previous = std::mem::replace(&mut fp.position, to);
    Ok(Applied {
        inverse: BoardCommand::MoveFootprint {
            footprint: id,
            to: previous,
        },
        patch: patch(BoardPatchKind::FootprintMoved, id),
    })
}

pub(super) fn rotate_to(
    board: &mut PcbBoard,
    id: Uuid,
    degrees: f64,
) -> Result<Applied, BoardCommandError> {
    if !degrees.is_finite() {
        return Err(BoardCommandError::NonFinite { field: "rotation" });
    }
    let fp = find(board, id)?;
    let previous = std::mem::replace(&mut fp.rotation, degrees);
    Ok(Applied {
        inverse: BoardCommand::Rotate {
            footprint: id,
            degrees: previous,
        },
        patch: patch(BoardPatchKind::FootprintRotated, id),
    })
}

pub(super) fn flip(board: &mut PcbBoard, id: Uuid) -> Result<Applied, BoardCommandError> {
    let fp = find(board, id)?;
    flip_footprint(fp);
    Ok(Applied {
        inverse: BoardCommand::Flip { footprint: id },
        patch: patch(BoardPatchKind::FootprintFlipped, id),
    })
}

/// Flip `fp` to the other side. Pads, graphics and properties keep their
/// library-local geometry (ADR-0005); only sided layers swap and the
/// rotation is negated, which with the placement-time x mirror gives
/// M·R(θ)·l + o. Negation is exact, so two flips restore every bit.
///
/// Property positions are read as footprint-local like pads, so they stay
/// untouched too. Unknown pad columns pass through unchanged.
fn flip_footprint(fp: &mut Footprint) {
    swap_side_in_place(&mut fp.layer);
    fp.rotation = negate_keeping_zero(fp.rotation);
    for pad in &mut fp.pads {
        pad.layers.iter_mut().for_each(swap_side_in_place);
    }
    for graphic in &mut fp.graphics {
        swap_side_in_place(&mut graphic.layer);
    }
    for property in &mut fp.properties {
        if let Some(layer) = property.layer.as_mut() {
            swap_side_in_place(layer);
        }
    }
}

/// Negate, keeping a zero as it is so a value at 0 does not read back
/// as -0. Still an involution: 0.0 and -0.0 each map to themselves.
fn negate_keeping_zero(value: f64) -> f64 {
    if value == 0.0 { value } else { -value }
}
