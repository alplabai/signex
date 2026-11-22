//! The board sketch with the board items it owns, as one board command
//! (decision 2, Caner 2026-11-20): `SetBoardSketchBake` and its inverse
//! `RestoreBoardSketchBake`. See `docs/PCB_SKETCH.md`, "Targets".

use std::collections::HashSet;

use serde::Serialize;
use signex_types::numeric::first_non_finite_path;
use signex_types::pcb::{BoardGraphic, PcbBoard, Point, Zone};
use signex_types::pcb_sketch::{BakedBoard, BoardSketch};
use signex_types::sketch::SketchData;
use signex_types::sketch::board_attr::{BOARD_ITEM_KINDS, SketchBoardItems};
use signex_types::sketch::pattern_instance::{instance_owner_key, owner_key_of};
use uuid::Uuid;

use super::refuse_non_finite;
use crate::pcb::{Applied, BoardCommand, BoardCommandError, BoardPatch, BoardPatchKind};

/// What a [`BoardCommand::SetBoardSketchBake`] replaced, to put back bit for
/// bit: the stored sketch, the outline when the bake wrote one, the whole
/// zone and drawing lists, and the position of each footprint it moved.
#[derive(Debug, Clone, PartialEq)]
pub struct SketchBakeSnapshot {
    sketch: Option<Box<BoardSketch>>,
    outline: Option<Vec<Point>>,
    zones: Vec<Zone>,
    graphics: Vec<BoardGraphic>,
    positions: Vec<(Uuid, Point)>,
}

impl BoardCommand {
    /// [`BoardCommand::set_board_sketch`] with the pours, mechanical
    /// drawings and component anchors the sketch baked to
    /// (`signex_bake::bake_board_with_items`). Records in the sketch which
    /// of those it baked, so it keeps owning them once their entities go.
    pub fn set_board_sketch_with_items(
        mut sketch: SketchData,
        baked: BakedBoard,
        items: SketchBoardItems,
    ) -> Self {
        for kind in BOARD_ITEM_KINDS {
            let count = items.count(kind).unwrap_or(0);
            sketch.record_baked(kind, count > 0);
        }
        let (stored, outline) = BoardSketch::from_bake(sketch, baked);
        BoardCommand::SetBoardSketchBake {
            sketch: Some(Box::new(stored)),
            outline,
            items: Box::new(items),
        }
    }
}

pub(in crate::pcb) fn set(
    board: &mut PcbBoard,
    sketch: Option<Box<BoardSketch>>,
    outline: Option<Vec<Point>>,
    items: SketchBoardItems,
) -> Result<Applied, BoardCommandError> {
    if let Some(stored) = sketch.as_deref() {
        refuse_non_finite(stored)?;
    }
    if outline.iter().flatten().any(|p| !finite(*p)) {
        return Err(BoardCommandError::NonFinite { field: "outline" });
    }
    refuse_non_finite_items(&items)?;
    if let Some(anchor) = items
        .anchors
        .iter()
        .find(|a| !board.footprints.iter().any(|fp| fp.uuid == a.footprint))
    {
        return Err(BoardCommandError::FootprintNotFound(anchor.footprint));
    }

    let owned = OwnedItems::of(board.sketch.as_ref(), sketch.as_deref());
    let zones = board
        .zones
        .iter()
        .filter(|zone| !owned.holds(zone.uuid))
        .cloned()
        .chain(items.zones)
        .collect();
    let graphics = board
        .graphics
        .iter()
        .filter(|graphic| !owned.holds(graphic.uuid))
        .cloned()
        .chain(items.graphics)
        .collect();
    // One entry per footprint, its position before the command: a footprint
    // two anchors name ends at the last and comes back to where it began.
    let mut positions: Vec<(Uuid, Point)> = Vec::new();
    for anchor in &items.anchors {
        let Some(fp) = board
            .footprints
            .iter_mut()
            .find(|fp| fp.uuid == anchor.footprint && !fp.locked)
        else {
            continue;
        };
        let before = std::mem::replace(&mut fp.position, anchor.position);
        if !positions.iter().any(|(id, _)| *id == fp.uuid) {
            positions.push((fp.uuid, before));
        }
    }
    let snapshot = SketchBakeSnapshot {
        zones: std::mem::replace(&mut board.zones, zones),
        graphics: std::mem::replace(&mut board.graphics, graphics),
        sketch: std::mem::replace(&mut board.sketch, sketch.map(|stored| *stored)).map(Box::new),
        outline: outline.map(|points| std::mem::replace(&mut board.outline, points)),
        positions,
    };
    Ok(applied(snapshot))
}

/// Put back what a [`BoardCommand::SetBoardSketchBake`] replaced. Only the
/// footprints are checked: the values were on the board before.
pub(in crate::pcb) fn restore(
    board: &mut PcbBoard,
    snapshot: SketchBakeSnapshot,
) -> Result<Applied, BoardCommandError> {
    if let Some((gone, _)) = snapshot
        .positions
        .iter()
        .find(|(id, _)| !board.footprints.iter().any(|fp| fp.uuid == *id))
    {
        return Err(BoardCommandError::FootprintNotFound(*gone));
    }
    let mut positions = Vec::with_capacity(snapshot.positions.len());
    for (id, at) in snapshot.positions {
        if let Some(fp) = board.footprints.iter_mut().find(|fp| fp.uuid == id) {
            positions.push((id, std::mem::replace(&mut fp.position, at)));
        }
    }
    let undone = SketchBakeSnapshot {
        zones: std::mem::replace(&mut board.zones, snapshot.zones),
        graphics: std::mem::replace(&mut board.graphics, snapshot.graphics),
        sketch: std::mem::replace(&mut board.sketch, snapshot.sketch.map(|s| *s)).map(Box::new),
        outline: snapshot
            .outline
            .map(|points| std::mem::replace(&mut board.outline, points)),
        positions,
    };
    Ok(applied(undone))
}

fn applied(inverse: SketchBakeSnapshot) -> Applied {
    Applied {
        inverse: BoardCommand::RestoreBoardSketchBake(Box::new(inverse)),
        patch: BoardPatch {
            kind: BoardPatchKind::BoardSketchReplaced,
            items: Vec::new(),
        },
    }
}

/// What a board sketch owns on the board: the items of every entity of the
/// sketch it replaces and of the one it stores, pattern instances included.
struct OwnedItems {
    entities: HashSet<Uuid>,
    instance_owners: HashSet<u128>,
}

impl OwnedItems {
    fn of(before: Option<&BoardSketch>, after: Option<&BoardSketch>) -> Self {
        let entities: HashSet<Uuid> = [before, after]
            .into_iter()
            .flatten()
            .flat_map(|stored| stored.sketch.entities.iter().map(|e| e.id.0))
            .collect();
        let instance_owners = entities.iter().copied().map(instance_owner_key).collect();
        Self {
            entities,
            instance_owners,
        }
    }

    fn holds(&self, item: Uuid) -> bool {
        self.entities.contains(&item)
            || owner_key_of(item).is_some_and(|key| self.instance_owners.contains(&key))
    }
}

fn finite(p: Point) -> bool {
    p.x.is_finite() && p.y.is_finite()
}

/// A NaN or infinity in a baked item would make the board unsaveable.
fn refuse_non_finite_items(items: &SketchBoardItems) -> Result<(), BoardCommandError> {
    #[derive(Serialize)]
    struct Rows<'a> {
        zones: &'a [Zone],
        graphics: &'a [BoardGraphic],
    }
    if items.anchors.iter().any(|a| !finite(a.position)) {
        return Err(BoardCommandError::NonFinite { field: "position" });
    }
    let rows = Rows {
        zones: &items.zones,
        graphics: &items.graphics,
    };
    match first_non_finite_path(&rows) {
        Ok(None) => Ok(()),
        Ok(Some(path)) => Err(BoardCommandError::NonFiniteSketch(path)),
        Err(error) => Err(BoardCommandError::NonFiniteSketch(error.to_string())),
    }
}

#[cfg(test)]
#[path = "board_sketch_items_tests.rs"]
mod tests;
