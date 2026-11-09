//! Footprints on their way to a new place (#2240): a drag in progress, or
//! an armed Edit ▸ Move waiting for its clicks. Pure state; the board is
//! untouched until the drop commits one `MoveFootprint` batch.

use std::collections::BTreeSet;
use std::path::PathBuf;

use signex_engine::pcb::placement::{grid_step_mm, snap_delta};
use signex_types::pcb::{PcbBoard, Point};
use uuid::Uuid;

/// How the carry started, which decides what a click does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CarryKind {
    /// A press on a selected footprint, moved past the drag threshold.
    Drag,
    /// Edit ▸ Move: the first click is the reference point (and, with
    /// nothing selected, picks the footprint), the second the drop.
    Move,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PcbCarry {
    pub kind: CarryKind,
    /// The board the carry belongs to; another tab drops it.
    pub path: PathBuf,
    /// Empty while a Move waits to pick a footprint.
    pub footprints: BTreeSet<Uuid>,
    /// The reference point, board mm; `None` until a Move's first click.
    pub anchor: Option<Point>,
    /// The snapped offset the preview shows.
    pub delta: Point,
}

impl PcbCarry {
    pub fn new(kind: CarryKind, path: PathBuf, footprints: BTreeSet<Uuid>) -> Self {
        Self {
            kind,
            path,
            footprints,
            anchor: None,
            delta: Point::new(0.0, 0.0),
        }
    }
}

/// The offset that takes the carried footprints from `from` towards `to`:
/// the origin of the footprint nearest `from` lands on the board's grid
/// (`PcbSetup::grid_size`, else the PCB default).
pub fn carry_delta(board: &PcbBoard, footprints: &BTreeSet<Uuid>, from: Point, to: Point) -> Point {
    let raw = Point::new(to.x - from.x, to.y - from.y);
    let distance = |p: Point| (p.x - from.x).powi(2) + (p.y - from.y).powi(2);
    let anchor = board
        .footprints
        .iter()
        .filter(|fp| footprints.contains(&fp.uuid))
        .map(|fp| fp.position)
        .min_by(|a, b| distance(*a).total_cmp(&distance(*b)));
    match anchor {
        Some(anchor) => snap_delta(anchor, raw, grid_step_mm(board)),
        None => raw,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_nearest_origin_lands_on_the_grid() {
        let board: PcbBoard = serde_json::from_value(serde_json::json!({
            "uuid": Uuid::nil(),
            "setup": {"grid_size": 0.5},
            "footprints": [
                {"uuid": Uuid::from_u128(1), "position": {"x": 1.1, "y": 1.0}},
                {"uuid": Uuid::from_u128(2), "position": {"x": 9.0, "y": 9.0}}
            ]
        }))
        .unwrap();
        let both: BTreeSet<Uuid> = [Uuid::from_u128(1), Uuid::from_u128(2)].into();
        let delta = carry_delta(&board, &both, Point::new(1.0, 1.0), Point::new(2.0, 1.3));
        // U1 at 1.1 + 1.0 = 2.1 snaps to 2.0; y 1.0 + 0.3 snaps to 1.5.
        assert!(
            (delta.x - 0.9).abs() < 1e-9 && (delta.y - 0.5).abs() < 1e-9,
            "{delta:?}"
        );
    }
}
