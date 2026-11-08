//! Move, rotate and flip one footprint. Pads, graphics and property texts
//! are stored in the footprint's local frame, so none of the three rewrites
//! their positions. The flip itself is `pcb_geom::flip_footprint`. Lock state
//! is the caller's check, not this module's.

use signex_types::pcb::{Footprint, PcbBoard, Point};
use uuid::Uuid;

use super::command::{Applied, BoardCommand, BoardPatch, BoardPatchKind};
use super::error::BoardCommandError;
use super::items::BoardItemId;

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
    let before = FootprintFace::of(fp);
    pcb_geom::flip_footprint(fp);
    Ok(Applied {
        inverse: BoardCommand::RestoreFace {
            footprint: id,
            face: before,
        },
        patch: patch(BoardPatchKind::FootprintFlipped, id),
    })
}

pub(super) fn restore_face(
    board: &mut PcbBoard,
    id: Uuid,
    face: FootprintFace,
) -> Result<Applied, BoardCommandError> {
    let fp = find(board, id)?;
    if !face.fits(fp) {
        return Err(BoardCommandError::FaceMismatch(id));
    }
    let current = FootprintFace::of(fp);
    face.write(fp);
    Ok(Applied {
        inverse: BoardCommand::RestoreFace {
            footprint: id,
            face: current,
        },
        patch: patch(BoardPatchKind::FootprintFlipped, id),
    })
}

/// The side-dependent state of a footprint: its rotation and every sided
/// layer name. A flip stores the state it replaced, so undo puts back the
/// exact rotation and layers instead of flipping again.
#[derive(Debug, Clone, PartialEq)]
pub struct FootprintFace {
    layer: String,
    rotation: f64,
    pad_layers: Vec<Vec<String>>,
    graphic_layers: Vec<String>,
    property_layers: Vec<Option<String>>,
}

impl FootprintFace {
    fn of(fp: &Footprint) -> Self {
        Self {
            layer: fp.layer.clone(),
            rotation: fp.rotation,
            pad_layers: fp.pads.iter().map(|p| p.layers.clone()).collect(),
            graphic_layers: fp.graphics.iter().map(|g| g.layer.clone()).collect(),
            property_layers: fp.properties.iter().map(|p| p.layer.clone()).collect(),
        }
    }

    /// Whether the footprint still has the pads, graphics and properties
    /// this face was taken from.
    fn fits(&self, fp: &Footprint) -> bool {
        self.pad_layers.len() == fp.pads.len()
            && self.graphic_layers.len() == fp.graphics.len()
            && self.property_layers.len() == fp.properties.len()
    }

    fn write(self, fp: &mut Footprint) {
        fp.layer = self.layer;
        fp.rotation = self.rotation;
        for (pad, layers) in fp.pads.iter_mut().zip(self.pad_layers) {
            pad.layers = layers;
        }
        for (graphic, layer) in fp.graphics.iter_mut().zip(self.graphic_layers) {
            graphic.layer = layer;
        }
        for (property, layer) in fp.properties.iter_mut().zip(self.property_layers) {
            property.layer = layer;
        }
    }
}
