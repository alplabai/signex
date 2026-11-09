//! Placement edits as one batch of [`BoardCommand`]s (#2240): move, rotate,
//! flip and delete the footprints a selection names, refusing a locked one.
//!
//! Pure: each function reads a board and returns the commands that do the
//! edit, which the caller runs as one undo step. Nothing here writes.

use std::collections::BTreeSet;

use signex_types::pcb::{Footprint, PCB_GRID_MM, PcbBoard, Point};
use uuid::Uuid;

use super::command::BoardCommand;
use super::error::BoardCommandError;
use super::items::BoardItemId;

/// A quarter turn or a mirror of placed footprints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlacementTransform {
    /// Turn by `quarter_turns` x 90°, counter-clockwise in the model's axes
    /// (the sense `Footprint::rotation` and the renderer use).
    Rotate { quarter_turns: i32 },
    /// Mirror left to right, which puts each footprint on the other side
    /// (`BoardCommand::Flip`).
    FlipLeftRight,
    /// Mirror top to bottom: a flip and a half turn.
    FlipTopBottom,
}

/// Why a placement edit did nothing.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum PlacementRefusal {
    #[error("nothing is selected that this can act on")]
    NothingSelected,
    #[error("{} is locked", .0.join(", "))]
    Locked(Vec<String>),
    #[error(transparent)]
    Command(#[from] BoardCommandError),
}

/// The snap step of `board`: its setup's grid, else the PCB default.
pub fn grid_step_mm(board: &PcbBoard) -> f64 {
    board
        .setup
        .as_ref()
        .map(|setup| setup.grid_size)
        .filter(|step| step.is_finite() && *step > 0.0)
        .unwrap_or(PCB_GRID_MM)
}

/// The move that takes `anchor` to the grid point nearest `anchor + raw`.
/// An unusable grid leaves `raw` as it is.
pub fn snap_delta(anchor: Point, raw: Point, grid_mm: f64) -> Point {
    if !grid_mm.is_finite() || grid_mm <= 0.0 {
        return raw;
    }
    let snap = |v: f64| (v / grid_mm).round() * grid_mm;
    Point::new(
        snap(anchor.x + raw.x) - anchor.x,
        snap(anchor.y + raw.y) - anchor.y,
    )
}

/// The footprints `ids` names, refusing an unknown, a locked or no footprint.
pub fn editable_footprints<'a>(
    board: &'a PcbBoard,
    ids: &BTreeSet<Uuid>,
) -> Result<Vec<&'a Footprint>, PlacementRefusal> {
    if ids.is_empty() {
        return Err(PlacementRefusal::NothingSelected);
    }
    let mut found = Vec::with_capacity(ids.len());
    for &id in ids {
        let footprint = board
            .footprints
            .iter()
            .find(|fp| fp.uuid == id)
            .ok_or(BoardCommandError::FootprintNotFound(id))?;
        found.push(footprint);
    }
    let locked: Vec<String> = found
        .iter()
        .filter(|fp| fp.locked)
        .map(|fp| name(fp))
        .collect();
    if locked.is_empty() {
        Ok(found)
    } else {
        Err(PlacementRefusal::Locked(locked))
    }
}

/// The name a notice gives `footprint`: its reference, else its id.
pub fn name(footprint: &Footprint) -> String {
    if footprint.reference.trim().is_empty() {
        footprint.uuid.to_string()
    } else {
        footprint.reference.clone()
    }
}

/// Move every footprint in `ids` by `delta`.
pub fn move_commands(
    board: &PcbBoard,
    ids: &BTreeSet<Uuid>,
    delta: Point,
) -> Result<Vec<BoardCommand>, PlacementRefusal> {
    let footprints = editable_footprints(board, ids)?;
    if delta.x == 0.0 && delta.y == 0.0 {
        return Ok(Vec::new());
    }
    Ok(footprints
        .iter()
        .map(|fp| BoardCommand::MoveFootprint {
            footprint: fp.uuid,
            to: Point::new(fp.position.x + delta.x, fp.position.y + delta.y),
        })
        .collect())
}

/// Turn or mirror the footprints in `ids`. One footprint turns about its
/// own origin; several turn about the centre of their origins, put on the
/// grid, and move with it.
pub fn transform_commands(
    board: &PcbBoard,
    ids: &BTreeSet<Uuid>,
    transform: PlacementTransform,
) -> Result<Vec<BoardCommand>, PlacementRefusal> {
    let footprints = editable_footprints(board, ids)?;
    let pivot = pivot(&footprints, grid_step_mm(board));
    let mut commands = Vec::new();
    for fp in footprints {
        let id = fp.uuid;
        let (dx, dy) = (fp.position.x - pivot.x, fp.position.y - pivot.y);
        let (offset, rotation) = match transform {
            PlacementTransform::Rotate { quarter_turns } => {
                let offset = match quarter_turns.rem_euclid(4) {
                    0 => (dx, dy),
                    1 => (-dy, dx),
                    2 => (-dx, -dy),
                    _ => (dy, -dx),
                };
                let degrees = f64::from(quarter_turns.rem_euclid(4)) * 90.0;
                (offset, Some((fp.rotation + degrees).rem_euclid(360.0)))
            }
            PlacementTransform::FlipLeftRight => {
                commands.push(BoardCommand::Flip { footprint: id });
                ((-dx, dy), None)
            }
            PlacementTransform::FlipTopBottom => {
                commands.push(BoardCommand::Flip { footprint: id });
                // The flip stores `-rotation`; the half turn makes it 180 - r.
                ((dx, -dy), Some((180.0 - fp.rotation).rem_euclid(360.0)))
            }
        };
        if let Some(degrees) = rotation {
            commands.push(BoardCommand::Rotate {
                footprint: id,
                degrees,
            });
        }
        let to = Point::new(pivot.x + offset.0, pivot.y + offset.1);
        if to != fp.position {
            commands.push(BoardCommand::MoveFootprint { footprint: id, to });
        }
    }
    Ok(commands)
}

/// Delete `items`, refusing the whole edit if a footprint among them is
/// locked.
pub fn delete_commands(
    board: &PcbBoard,
    items: Vec<BoardItemId>,
) -> Result<Vec<BoardCommand>, PlacementRefusal> {
    if items.is_empty() {
        return Err(PlacementRefusal::NothingSelected);
    }
    let footprints: BTreeSet<Uuid> = items
        .iter()
        .filter_map(|item| match *item {
            BoardItemId::Footprint(id) => Some(id),
            _ => None,
        })
        .collect();
    if !footprints.is_empty() {
        editable_footprints(board, &footprints)?;
    }
    Ok(vec![BoardCommand::DeleteItems { items }])
}

/// Where a group turns: one footprint's origin, else the centre of the
/// origins' bounds snapped to the grid, so on-grid parts stay on the grid.
fn pivot(footprints: &[&Footprint], grid_mm: f64) -> Point {
    if let [only] = footprints {
        return only.position;
    }
    let (mut lo, mut hi) = (
        Point::new(f64::INFINITY, f64::INFINITY),
        Point::new(f64::NEG_INFINITY, f64::NEG_INFINITY),
    );
    for fp in footprints {
        lo = Point::new(lo.x.min(fp.position.x), lo.y.min(fp.position.y));
        hi = Point::new(hi.x.max(fp.position.x), hi.y.max(fp.position.y));
    }
    let centre = Point::new((lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0);
    let snapped = snap_delta(Point::new(0.0, 0.0), centre, grid_mm);
    if snapped.x.is_finite() && snapped.y.is_finite() {
        snapped
    } else {
        centre
    }
}

#[cfg(test)]
#[path = "placement_tests.rs"]
mod tests;
