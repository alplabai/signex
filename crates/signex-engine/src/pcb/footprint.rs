//! Move, rotate and flip one footprint. Pads, graphics and property texts
//! are stored in the footprint's local frame, so a move or a rotation only
//! touches the footprint's own origin and angle.

use signex_types::pcb::{Footprint, PcbBoard, Point};
use uuid::Uuid;

use super::command::{Applied, BoardCommand, BoardPatch, BoardPatchKind};
use super::error::BoardCommandError;
use super::items::BoardItemId;
use super::layer_side::mirror_in_place;

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

/// Mirror `fp` to the other side. World = R(θ)·local + origin, and mirroring
/// that about the vertical line through the origin equals R(−θ) applied to
/// the local point with x negated. Negation is exact, so two flips restore
/// every bit.
fn flip_footprint(fp: &mut Footprint) {
    mirror_in_place(&mut fp.layer);
    fp.rotation = negate_keeping_zero(fp.rotation);
    for pad in &mut fp.pads {
        mirror_point(&mut pad.position);
        pad.layers.iter_mut().for_each(mirror_in_place);
    }
    for graphic in &mut fp.graphics {
        mirror_in_place(&mut graphic.layer);
        for p in [
            &mut graphic.start,
            &mut graphic.end,
            &mut graphic.center,
            &mut graphic.mid,
            &mut graphic.position,
        ]
        .into_iter()
        .flatten()
        {
            mirror_point(p);
        }
        graphic.points.iter_mut().for_each(mirror_point);
        graphic.rotation = negate_keeping_zero(graphic.rotation);
    }
    for property in &mut fp.properties {
        if let Some(layer) = property.layer.as_mut() {
            mirror_in_place(layer);
        }
        if let Some(p) = property.position.as_mut() {
            mirror_point(p);
        }
        property.rotation = negate_keeping_zero(property.rotation);
    }
}

fn mirror_point(p: &mut Point) {
    p.x = negate_keeping_zero(p.x);
}

/// Negate, keeping a zero as it is so a value at 0 does not read back
/// as -0. Still an involution: 0.0 and -0.0 each map to themselves.
fn negate_keeping_zero(value: f64) -> f64 {
    if value == 0.0 { value } else { -value }
}
