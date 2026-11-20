//! Arrange in a box (#2409): the selected footprints packed inside a drawn
//! rectangle without overlap, tallest first (`pcb_geom::arrange_in_box`),
//! each origin on the components grid, counted from that grid's origin.
//! One undo step; a locked footprint refuses the whole run, as any
//! placement edit does. Rooms pack their parts with the same order, gap and
//! grid ([`PackGrid`], [`packing_order`]).

use std::collections::BTreeSet;

use pcb_geom::{PointNm, RectNm, mm_to_nm, nm_to_mm, placed_extent};
use signex_types::pcb::{Footprint, PcbBoard, Point};
use signex_types::pcb_options::GridUse;
use uuid::Uuid;

use super::{ToolEdit, ToolRefusal, ToolReport};
use crate::pcb::command::BoardCommand;
use crate::pcb::placement::{editable_footprints, grid_for};

/// The gap kept between two arranged footprints and to the box edge.
pub const ARRANGE_GAP_MM: f64 = 0.5;

/// The board's components grid in nm: packing snaps to multiples of the
/// step, so it runs with the grid's origin moved to zero ([`PackGrid::to_grid`])
/// and its origins are moved back ([`PackGrid::to_board`]).
#[derive(Debug, Clone, Copy)]
pub(crate) struct PackGrid {
    pub step: i64,
    pub gap: i64,
    zero: PointNm,
}

impl PackGrid {
    /// `None` when the grid's origin does not convert to nm.
    pub(crate) fn of(board: &PcbBoard) -> Option<PackGrid> {
        let grid = grid_for(board, GridUse::Components);
        Some(PackGrid {
            step: mm_to_nm(grid.step_mm, "grid").unwrap_or(1).max(1),
            gap: mm_to_nm(ARRANGE_GAP_MM, "gap").unwrap_or(0),
            zero: PointNm::from_mm(grid.origin, "grid.origin").ok()?,
        })
    }

    pub(crate) fn to_grid(self, r: RectNm) -> RectNm {
        RectNm::new(r.min.minus(self.zero), r.max.minus(self.zero))
    }

    /// A packed origin back on the board, nm.
    pub(crate) fn to_board_nm(self, origin: PointNm) -> PointNm {
        origin.plus(self.zero)
    }

    /// A packed origin as a footprint position on the board, mm.
    pub(crate) fn to_board(self, origin: PointNm) -> Point {
        let origin = self.to_board_nm(origin);
        Point::new(nm_to_mm(origin.x), nm_to_mm(origin.y))
    }
}

/// Footprints and their placed extents in packing order: tallest first,
/// which packs rows tighter; ties by designator, then uuid.
pub(crate) fn packing_order(sized: &mut [(&Footprint, RectNm)]) {
    sized.sort_by(|(fa, ea), (fb, eb)| {
        eb.height()
            .cmp(&ea.height())
            .then_with(|| fa.reference.cmp(&fb.reference))
            .then_with(|| fa.uuid.cmp(&fb.uuid))
    });
}

/// Move the footprints `footprints` names into the box spanned by `a` and
/// `b`.
pub fn arrange_in_box(
    board: &PcbBoard,
    footprints: &BTreeSet<Uuid>,
    a: Point,
    b: Point,
) -> Result<ToolEdit, ToolRefusal> {
    let parts = editable_footprints(board, footprints)?;
    let corner = |p: Point| PointNm::from_mm(p, "arrange.corner").ok();
    let (Some(a), Some(b)) = (corner(a), corner(b)) else {
        return Err(ToolRefusal::DoesNotFit);
    };
    let area = RectNm::new(a, b);
    let mut sized = Vec::with_capacity(parts.len());
    for fp in parts {
        let extent = placed_extent(fp).map_err(|_| ToolRefusal::DoesNotFit)?;
        sized.push((fp, extent));
    }
    packing_order(&mut sized);
    let grid = PackGrid::of(board).ok_or(ToolRefusal::DoesNotFit)?;
    let extents: Vec<RectNm> = sized.iter().map(|(_, e)| *e).collect();
    let origins = pcb_geom::arrange_in_box(grid.to_grid(area), &extents, grid.gap, grid.step)
        .ok_or(ToolRefusal::DoesNotFit)?;
    let mut commands = Vec::new();
    let mut changed = Vec::new();
    for ((fp, _), origin) in sized.iter().zip(origins) {
        let to = grid.to_board(origin);
        if fp.position != to {
            changed.push(fp.uuid);
            commands.push(BoardCommand::MoveFootprint {
                footprint: fp.uuid,
                to,
            });
        }
    }
    Ok(ToolEdit {
        commands,
        report: ToolReport {
            changed,
            ..ToolReport::default()
        },
    })
}
