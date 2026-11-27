//! Arrange in Room (#2457): every unlocked member of a room packed inside
//! it again, as Arrange in Box packs (`pcb_geom::pack_in_box`: its order,
//! gap and grid), clear of the locked members and of the other parts that
//! sit in the room. What does not fit goes on in rows below the room, as
//! Pull parts into room does. Pure: the app runs the moves as one
//! `PlaceRoom` step. Rules: `docs/PCB_ROOMS.md`, "Arrange in room".

use pcb_geom::{PointNm, RectNm, placed_extent};
use signex_types::pcb::{Footprint, PcbBoard, Point, Room};
use uuid::Uuid;

use super::{BoxMm, inner_box};
use crate::pcb::board_tools::{PackGrid, packing_order};

/// What Arrange in Room does to a room's members.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RoomArrangement {
    /// New footprint origins, only for members whose origin changes.
    pub moves: Vec<(Uuid, Point)>,
    /// Members that did not fit inside the room: they go in rows below it.
    pub outside: Vec<Uuid>,
}

/// The box `fp`'s placed extent covers on the board, nm.
pub(crate) fn placed_box(fp: &Footprint) -> Option<RectNm> {
    let extent = placed_extent(fp).ok()?;
    let at = PointNm::from_mm(fp.position, "room part").ok()?;
    Some(RectNm::new(at.plus(extent.min), at.plus(extent.max)))
}

/// Arrange in Room on `room`: its unlocked members packed into its outline
/// shrunk by its margin. A polygon room is packed into the box round it. A
/// member whose extent does not convert stays where it is.
pub fn arrange_room(room: &Room, board: &PcbBoard) -> RoomArrangement {
    let Some(inner) = inner_box(room) else {
        return RoomArrangement::default();
    };
    let (Some(grid), Some(area)) = (PackGrid::of(board), inner.as_rect()) else {
        return RoomArrangement::default();
    };
    // In the way: the locked members, and every other part whose extent
    // meets the room.
    let occupied: Vec<RectNm> = board
        .footprints
        .iter()
        .filter_map(|fp| {
            let placed = placed_box(fp)?;
            let stays = if room.holds(fp) {
                fp.locked
            } else {
                placed.intersects(&area)
            };
            stays.then(|| grid.to_grid(placed))
        })
        .collect();
    let mut sized: Vec<(&Footprint, RectNm)> = room
        .members(&board.footprints)
        .filter(|fp| !fp.locked)
        .filter_map(|fp| Some((fp, placed_extent(fp).ok()?)))
        .collect();
    packing_order(&mut sized);
    let extents: Vec<RectNm> = sized.iter().map(|(_, e)| *e).collect();
    let origins =
        pcb_geom::pack_in_box(grid.to_grid(area), &occupied, &extents, grid.gap, grid.step);
    let mut arrangement = RoomArrangement::default();
    for ((fp, extent), origin) in sized.iter().zip(origins) {
        let origin = grid.to_board_nm(origin);
        let covers = BoxMm::of_rect(RectNm::new(
            origin.plus(extent.min),
            origin.plus(extent.max),
        ));
        if !inner.holds(&covers) {
            arrangement.outside.push(fp.uuid);
        }
        let to = Point::new(pcb_geom::nm_to_mm(origin.x), pcb_geom::nm_to_mm(origin.y));
        if fp.position != to {
            arrangement.moves.push((fp.uuid, to));
        }
    }
    arrangement
}
