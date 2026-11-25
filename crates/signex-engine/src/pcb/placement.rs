//! Placement edits as one batch of [`BoardCommand`]s (#2240): move, rotate,
//! flip and delete the footprints a selection names, refusing a locked one.
//!
//! Pure: each function reads a board and returns the commands that do the
//! edit, which the caller runs as one undo step. Nothing here writes.

use std::collections::BTreeSet;

use pcb_geom::{ARC_FOLLOW_MAX_GROWTH, ArcFollowFault, TrackEnds};
use signex_types::corner::CornerRule;
use signex_types::pcb::{Footprint, PCB_GRID_MM, PcbBoard, Point, Via};
use signex_types::pcb_options::{GridSpec, GridUse};
use uuid::Uuid;

use super::command::BoardCommand;
use super::error::BoardCommandError;
use super::items::{BoardItem, BoardItemId};

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
    #[error("arc {0} would collapse onto its other end")]
    ArcCollapses(Uuid),
    #[error("arc {0} would grow to more than {1} times its radius")]
    ArcGrows(Uuid, f64),
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

/// The grid placing or carrying `what` on `board` snaps to (#2374): the
/// first of the board's extra grids that applies to it, else the board's
/// own step at the origin.
pub fn grid_for(board: &PcbBoard, what: GridUse) -> GridSpec {
    board.options.grid_for(what, grid_step_mm(board))
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

/// The footprint of `board` whose designator is `designator` (trimmed), for
/// a command that names a component (#2410). Refused when none or several
/// carry it.
pub fn by_designator(board: &PcbBoard, designator: &str) -> Result<Uuid, String> {
    let designator = designator.trim();
    let mut found = board
        .footprints
        .iter()
        .filter(|fp| fp.reference == designator);
    match (found.next(), found.next()) {
        (Some(fp), None) => Ok(fp.uuid),
        (None, _) => Err(format!("no component on the board is named {designator}")),
        (Some(_), Some(_)) => Err(format!(
            "several components on the board are named {designator}; annotate the board first"
        )),
    }
}

/// Move every footprint in `ids` by `delta`. The track ends on their pads
/// move with them (`pcb_geom::tracks_on_pads`, #2379), bent at `corner`
/// ([`stretch_commands`]), in the same batch, so the move stays one undo
/// step.
pub fn move_commands(
    board: &PcbBoard,
    ids: &BTreeSet<Uuid>,
    delta: Point,
    corner: CornerRule,
) -> Result<Vec<BoardCommand>, PlacementRefusal> {
    let mut commands = move_footprints_only(board, ids, delta)?;
    if commands.is_empty() {
        return Ok(commands);
    }
    let ends = pcb_geom::tracks_on_pads(board, ids);
    commands.extend(stretch_commands(board, &ends, delta, corner));
    Ok(commands)
}

/// The tracks of `ends` with their following ends moved by `delta`, kept at
/// 0°, 45° or 90° (#2513, Caner 2026-11-25): a track whose moved end is off
/// those lines from its fixed end bends at one 45° corner by `corner`
/// (`pcb_geom::TrackEnds::stretch`), the leg to its moved end a new track.
/// A `ReplaceItems` of the tracks, then an `AddItems` of the new legs;
/// nothing for no ends.
pub fn stretch_commands(
    board: &PcbBoard,
    ends: &[TrackEnds],
    delta: Point,
    corner: CornerRule,
) -> Vec<BoardCommand> {
    let (tracks, legs) = stretched_tracks(board, ends, delta, corner);
    let mut commands = Vec::new();
    if !tracks.is_empty() {
        commands.push(BoardCommand::ReplaceItems { items: tracks });
    }
    if !legs.is_empty() {
        commands.push(BoardCommand::AddItems { items: legs });
    }
    commands
}

/// The tracks of `ends` stretched by `delta` at `corner`, and their new
/// legs, as board items.
fn stretched_tracks(
    board: &PcbBoard,
    ends: &[TrackEnds],
    delta: Point,
    corner: CornerRule,
) -> (Vec<BoardItem>, Vec<BoardItem>) {
    let (mut tracks, mut legs) = (Vec::new(), Vec::new());
    for track in ends {
        let segment = &board.segments[track.index];
        let (kept, leg) = track.stretch(segment, delta, corner, Uuid::now_v7());
        tracks.push(BoardItem::Segment(kept));
        legs.extend(leg.map(BoardItem::Segment));
    }
    (tracks, legs)
}

/// Move every footprint in `ids` by `delta` and leave the tracks on their
/// pads where they are (a Ctrl drag): their ends no longer touch the pads.
pub fn move_footprints_only(
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

/// The vias `ids` names, refusing an unknown, a locked or no via.
pub fn editable_vias<'a>(
    board: &'a PcbBoard,
    ids: &BTreeSet<Uuid>,
) -> Result<Vec<&'a Via>, PlacementRefusal> {
    if ids.is_empty() {
        return Err(PlacementRefusal::NothingSelected);
    }
    let mut found = Vec::with_capacity(ids.len());
    for &id in ids {
        let via = board
            .vias
            .iter()
            .find(|via| via.uuid == id)
            .ok_or(BoardCommandError::ItemNotFound(BoardItemId::Via(id)))?;
        found.push(via);
    }
    let locked = super::lock::locked_names(board, ids.iter().map(|&id| BoardItemId::Via(id)));
    if locked.is_empty() {
        Ok(found)
    } else {
        Err(PlacementRefusal::Locked(locked))
    }
}

/// Move every via in `ids` by `delta`, keeping its net. The track and copper
/// arc ends on it move with it (`pcb_geom::tracks_on_vias`, `arcs_on_vias`),
/// in the same batch, so the move stays one undo step; a track bends at
/// `corner` as [`stretch_commands`] bends it, its new leg added after.
/// Refused whole when a following arc would collapse or grow past its limit.
pub fn move_vias(
    board: &PcbBoard,
    ids: &BTreeSet<Uuid>,
    delta: Point,
    corner: CornerRule,
) -> Result<Vec<BoardCommand>, PlacementRefusal> {
    let mut commands = move_vias_only(board, ids, delta)?;
    let mut legs = Vec::new();
    if let Some(BoardCommand::ReplaceItems { items }) = commands.first_mut() {
        let ends = pcb_geom::tracks_on_vias(board, ids);
        let (tracks, new_legs) = stretched_tracks(board, &ends, delta, corner);
        items.extend(tracks);
        legs = new_legs;
        for ends in pcb_geom::arcs_on_vias(board, ids) {
            let moved = ends
                .checked_follow(&board.free.arcs[ends.index], delta)
                .map_err(|fault| match fault {
                    ArcFollowFault::Collapses => PlacementRefusal::ArcCollapses(ends.arc),
                    ArcFollowFault::GrowsTooMuch => {
                        PlacementRefusal::ArcGrows(ends.arc, ARC_FOLLOW_MAX_GROWTH)
                    }
                })?;
            items.push(BoardItem::Arc(moved));
        }
    }
    if !legs.is_empty() {
        commands.push(BoardCommand::AddItems { items: legs });
    }
    Ok(commands)
}

/// Move every via in `ids` by `delta` and leave the tracks on them where they
/// are (a Ctrl drag): their ends no longer touch the vias.
pub fn move_vias_only(
    board: &PcbBoard,
    ids: &BTreeSet<Uuid>,
    delta: Point,
) -> Result<Vec<BoardCommand>, PlacementRefusal> {
    let vias = editable_vias(board, ids)?;
    if delta.x == 0.0 && delta.y == 0.0 {
        return Ok(Vec::new());
    }
    let items = vias
        .into_iter()
        .map(|via| {
            let mut moved = via.clone();
            moved.position = Point::new(via.position.x + delta.x, via.position.y + delta.y);
            BoardItem::Via(moved)
        })
        .collect();
    Ok(vec![BoardCommand::ReplaceItems { items }])
}

/// Turn or mirror the footprints in `ids`. One footprint turns about its
/// own origin; several turn about the centre of their origins, put on the
/// grid, and move with it.
pub fn transform_commands(
    board: &PcbBoard,
    ids: &BTreeSet<Uuid>,
    transform: PlacementTransform,
) -> Result<Vec<BoardCommand>, PlacementRefusal> {
    transform_commands_with(board, ids, transform, true)
}

/// [`transform_commands`], the group's centre put on the grid only when
/// `on_grid`: with snapping off it is the exact centre of their origins.
pub fn transform_commands_with(
    board: &PcbBoard,
    ids: &BTreeSet<Uuid>,
    transform: PlacementTransform,
    on_grid: bool,
) -> Result<Vec<BoardCommand>, PlacementRefusal> {
    let footprints = editable_footprints(board, ids)?;
    let pivot = pivot(&footprints, on_grid.then(|| grid_step_mm(board)));
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

/// Delete `items`, refusing the whole edit if a footprint, track or via among
/// them is locked.
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
    let locked = super::lock::locked_names(board, items.iter().copied());
    if !locked.is_empty() {
        return Err(PlacementRefusal::Locked(locked));
    }
    Ok(vec![BoardCommand::DeleteItems { items }])
}

/// Where a group turns: one footprint's origin, else the centre of the
/// origins' bounds, snapped to `grid_mm` when given so on-grid parts stay on
/// the grid.
fn pivot(footprints: &[&Footprint], grid_mm: Option<f64>) -> Point {
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
    let Some(grid_mm) = grid_mm else {
        return centre;
    };
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
