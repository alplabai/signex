//! Rooms on a board (#2457): the room commands and the geometry they share
//! with the planner. A room's members are the footprints linked to a part
//! on its sheet placement or below it (`Room::holds`). Moving a room moves
//! its unlocked members by the same distance in the same step; a locked room
//! refuses a move or a fit. Every command takes or restores absolute values,
//! so undo is exact. See `docs/PCB_ROOMS.md`.

mod plan;

#[cfg(test)]
mod tests;

pub use plan::{
    PlanShape, PlannedRoom, RoomOptions, RoomPlan, SheetMembers, plan_rooms_from_sheets,
};

use signex_types::pcb::room::is_room_colour;
use signex_types::pcb::{Footprint, PcbBoard, Point, Room, RoomOutline};
use uuid::Uuid;

use super::command::{Applied, BoardCommand, BoardPatch, BoardPatchKind};
use super::error::BoardCommandError;
use super::items::BoardItemId;

/// The gap between parts packed into a room, mm.
pub const PACK_GAP_MM: f64 = 0.5;

/// A box on the board, mm.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoxMm {
    pub min: Point,
    pub max: Point,
}

impl BoxMm {
    pub fn width(&self) -> f64 {
        self.max.x - self.min.x
    }

    pub fn height(&self) -> f64 {
        self.max.y - self.min.y
    }

    /// The box grown by `d` on every side; shrunk for a negative `d`.
    pub fn inflate(&self, d: f64) -> BoxMm {
        BoxMm {
            min: Point::new(self.min.x - d, self.min.y - d),
            max: Point::new(self.max.x + d, self.max.y + d),
        }
    }

    pub fn union(&self, other: &BoxMm) -> BoxMm {
        BoxMm {
            min: Point::new(self.min.x.min(other.min.x), self.min.y.min(other.min.y)),
            max: Point::new(self.max.x.max(other.max.x), self.max.y.max(other.max.y)),
        }
    }

    /// Whether the two boxes overlap; boxes that only touch do not.
    pub fn overlaps(&self, other: &BoxMm) -> bool {
        self.min.x < other.max.x
            && other.min.x < self.max.x
            && self.min.y < other.max.y
            && other.min.y < self.max.y
    }

    /// Whether `inner` lies wholly inside this box, compared in the whole
    /// nm the file stores, so a room fitted round its parts holds them. A
    /// number that is not finite holds nothing.
    pub fn holds(&self, inner: &BoxMm) -> bool {
        match (self.as_nm(), inner.as_nm()) {
            (Some(outer), Some(inner)) => {
                inner[0] >= outer[0]
                    && inner[1] >= outer[1]
                    && inner[2] <= outer[2]
                    && inner[3] <= outer[3]
            }
            _ => false,
        }
    }

    /// `[min x, min y, max x, max y]` in nm.
    fn as_nm(&self) -> Option<[i64; 4]> {
        let nm = |v: f64| pcb_geom::mm_to_nm(v, "room").ok();
        Some([
            nm(self.min.x)?,
            nm(self.min.y)?,
            nm(self.max.x)?,
            nm(self.max.y)?,
        ])
    }
}

/// The box round a footprint's body (`pcb_geom::footprint_body`); a
/// 1 mm box on its origin when the body does not convert.
pub fn part_box(footprint: &Footprint) -> BoxMm {
    match pcb_geom::footprint_body(footprint) {
        Ok(body) => {
            let aabb = body.aabb();
            BoxMm {
                min: Point::new(
                    pcb_geom::nm_to_mm(aabb.min.x),
                    pcb_geom::nm_to_mm(aabb.min.y),
                ),
                max: Point::new(
                    pcb_geom::nm_to_mm(aabb.max.x),
                    pcb_geom::nm_to_mm(aabb.max.y),
                ),
            }
        }
        Err(_) => BoxMm {
            min: Point::new(footprint.position.x - 0.5, footprint.position.y - 0.5),
            max: Point::new(footprint.position.x + 0.5, footprint.position.y + 0.5),
        },
    }
}

/// The box round a room's outline.
pub fn room_box(room: &Room) -> Option<BoxMm> {
    let b = room.outline.bounds()?;
    Some(BoxMm {
        min: Point::new(b.x_mm, b.y_mm),
        max: Point::new(b.x_mm + b.w_mm, b.y_mm + b.h_mm),
    })
}

/// Where a room's members must stay: its box shrunk by the margin. A
/// polygon room is taken by its box here; the design rule check reads the
/// polygon itself (`signex_drc::rooms`).
pub fn inner_box(room: &Room) -> Option<BoxMm> {
    room_box(room).map(|b| b.inflate(-room.margin_mm.max(0.0)))
}

/// The members of `room` on `board` whose body is not inside its inner box.
pub fn members_outside<'a>(room: &'a Room, board: &'a PcbBoard) -> Vec<&'a Footprint> {
    let Some(inner) = inner_box(room) else {
        return Vec::new();
    };
    room.members(&board.footprints)
        .filter(|fp| !inner.holds(&part_box(fp)))
        .collect()
}

/// The rectangle round the bodies of `room`'s members plus its margin;
/// `None` for a room without members.
pub fn fitted_outline(room: &Room, board: &PcbBoard) -> Option<RoomOutline> {
    let around = room
        .members(&board.footprints)
        .map(part_box)
        .reduce(|a, b| a.union(&b))?
        .inflate(room.margin_mm.max(0.0));
    Some(RoomOutline::rect_between(around.min, around.max))
}

/// Where the unlocked members of `room` outside it go so they sit in rows
/// inside it, clear of the members already inside.
pub fn pull_moves(room: &Room, board: &PcbBoard) -> Vec<(Uuid, Point)> {
    pack_outside(room, room.members(&board.footprints).collect())
}

/// [`pull_moves`] for `members` of a room that may not be on the board yet.
pub(crate) fn pack_outside(room: &Room, members: Vec<&Footprint>) -> Vec<(Uuid, Point)> {
    let Some(inner) = inner_box(room) else {
        return Vec::new();
    };
    let (inside, outside): (Vec<&Footprint>, Vec<&Footprint>) = members
        .into_iter()
        .partition(|fp| inner.holds(&part_box(fp)));
    let outside: Vec<&Footprint> = outside.into_iter().filter(|fp| !fp.locked).collect();
    let occupied = inside.into_iter().map(part_box).collect();
    pack(inner, occupied, &outside)
}

/// Place `parts` in rows from the least corner of `inner`, clear of
/// `occupied` and of each other; the footprint origins that put them there.
/// A part that does not fit any more goes on below the last row.
pub(crate) fn pack(
    inner: BoxMm,
    mut occupied: Vec<BoxMm>,
    parts: &[&Footprint],
) -> Vec<(Uuid, Point)> {
    // Enough tries to step past every occupied box of a dense room.
    const MAX_TRIES: usize = 4096;
    let mut out = Vec::with_capacity(parts.len());
    let (mut x, mut y, mut row) = (inner.min.x, inner.min.y, 0.0_f64);
    for fp in parts {
        let body = part_box(fp);
        let (w, h) = (body.width(), body.height());
        for _ in 0..MAX_TRIES {
            if x + w > inner.max.x && x > inner.min.x {
                x = inner.min.x;
                y += row.max(h) + PACK_GAP_MM;
                row = 0.0;
            }
            let slot = BoxMm {
                min: Point::new(x, y),
                max: Point::new(x + w, y + h),
            };
            match occupied.iter().find(|o| o.overlaps(&slot)) {
                Some(block) => x = block.max.x + PACK_GAP_MM,
                None => {
                    let to = Point::new(
                        fp.position.x + (x - body.min.x),
                        fp.position.y + (y - body.min.y),
                    );
                    out.push((fp.uuid, to));
                    occupied.push(slot.inflate(PACK_GAP_MM / 2.0));
                    x += w + PACK_GAP_MM;
                    row = row.max(h);
                    break;
                }
            }
        }
    }
    out
}

fn patch(footprints: &[(Uuid, Point)]) -> BoardPatch {
    BoardPatch {
        kind: BoardPatchKind::RoomsChanged,
        items: footprints
            .iter()
            .map(|(id, _)| BoardItemId::Footprint(*id))
            .collect(),
    }
}

fn index_of(board: &PcbBoard, id: Uuid) -> Result<usize, BoardCommandError> {
    board
        .rooms
        .iter()
        .position(|r| r.uuid == id)
        .ok_or(BoardCommandError::RoomNotFound(id))
}

/// Every rule a stored room keeps: a name, an outline with an area and
/// finite numbers, a `#RRGGBB` colour and a finite margin not below zero.
fn check(room: &Room) -> Result<(), BoardCommandError> {
    let invalid = |field| Err(BoardCommandError::RoomInvalid { field });
    if room.name.trim().is_empty() {
        return invalid("name");
    }
    if !room.outline.is_finite() || !room.outline.has_area() {
        return invalid("outline");
    }
    if !is_room_colour(&room.colour) {
        return invalid("colour");
    }
    if !room.margin_mm.is_finite() || room.margin_mm < 0.0 {
        return invalid("margin");
    }
    Ok(())
}

/// Append `room`. The inverse removes it.
pub(super) fn add(board: &mut PcbBoard, room: Room) -> Result<Applied, BoardCommandError> {
    check(&room)?;
    if board.rooms.iter().any(|r| r.uuid == room.uuid) {
        return Err(BoardCommandError::RoomAlreadyPresent(room.uuid));
    }
    let id = room.uuid;
    board.rooms.push(room);
    Ok(Applied {
        inverse: BoardCommand::RemoveRoom { room: id },
        patch: patch(&[]),
    })
}

/// Remove the room; its members stay where they are.
pub(super) fn remove(board: &mut PcbBoard, id: Uuid) -> Result<Applied, BoardCommandError> {
    let index = index_of(board, id)?;
    let room = board.rooms.remove(index);
    Ok(Applied {
        inverse: BoardCommand::RestoreRoom {
            room: Box::new(room),
            index,
        },
        patch: patch(&[]),
    })
}

/// Put a removed room back at `index`; the inverse of a removal.
pub(super) fn restore(
    board: &mut PcbBoard,
    room: Room,
    index: usize,
) -> Result<Applied, BoardCommandError> {
    if board.rooms.iter().any(|r| r.uuid == room.uuid) {
        return Err(BoardCommandError::RoomAlreadyPresent(room.uuid));
    }
    if index > board.rooms.len() {
        return Err(BoardCommandError::RoomNotFound(room.uuid));
    }
    let id = room.uuid;
    board.rooms.insert(index, room);
    Ok(Applied {
        inverse: BoardCommand::RemoveRoom { room: id },
        patch: patch(&[]),
    })
}

/// Put `room` over the room with its uuid. A locked room that stays locked
/// keeps its outline: unlock it first.
pub(super) fn edit(board: &mut PcbBoard, room: Room) -> Result<Applied, BoardCommandError> {
    check(&room)?;
    let index = index_of(board, room.uuid)?;
    let old = &board.rooms[index];
    if old.locked && room.locked && old.outline != room.outline {
        return Err(BoardCommandError::RoomLocked(room.uuid));
    }
    let previous = std::mem::replace(&mut board.rooms[index], room);
    Ok(Applied {
        inverse: BoardCommand::EditRoom {
            room: Box::new(previous),
        },
        patch: patch(&[]),
    })
}

/// Set the room's outline and put each named footprint at its origin, as
/// one change. The inverse puts back what it replaced.
pub(super) fn place(
    board: &mut PcbBoard,
    id: Uuid,
    outline: RoomOutline,
    footprints: Vec<(Uuid, Point)>,
) -> Result<Applied, BoardCommandError> {
    let index = index_of(board, id)?;
    if !outline.is_finite() || !outline.has_area() {
        return Err(BoardCommandError::RoomInvalid { field: "outline" });
    }
    let mut old_positions = Vec::with_capacity(footprints.len());
    for (fp, to) in &footprints {
        if !to.x.is_finite() || !to.y.is_finite() {
            return Err(BoardCommandError::NonFinite { field: "position" });
        }
        if old_positions.iter().any(|(seen, _)| seen == fp) {
            return Err(BoardCommandError::DuplicateItem(BoardItemId::Footprint(
                *fp,
            )));
        }
        let at = board
            .footprints
            .iter()
            .find(|f| f.uuid == *fp)
            .ok_or(BoardCommandError::FootprintNotFound(*fp))?
            .position;
        old_positions.push((*fp, at));
    }
    for (fp, to) in &footprints {
        if let Some(f) = board.footprints.iter_mut().find(|f| f.uuid == *fp) {
            f.position = *to;
        }
    }
    let previous = std::mem::replace(&mut board.rooms[index].outline, outline);
    Ok(Applied {
        inverse: BoardCommand::PlaceRoom {
            room: id,
            outline: previous,
            footprints: old_positions,
        },
        patch: patch(&footprints),
    })
}

/// Move the room so its box starts at `to`, and its unlocked members by the
/// same distance. Refused for a locked room.
pub(super) fn move_room(
    board: &mut PcbBoard,
    id: Uuid,
    to: Point,
) -> Result<Applied, BoardCommandError> {
    let room = &board.rooms[index_of(board, id)?];
    if room.locked {
        return Err(BoardCommandError::RoomLocked(id));
    }
    if !to.x.is_finite() || !to.y.is_finite() {
        return Err(BoardCommandError::NonFinite { field: "position" });
    }
    let from = room_box(room).ok_or(BoardCommandError::RoomInvalid { field: "outline" })?;
    let (dx, dy) = (to.x - from.min.x, to.y - from.min.y);
    let outline = room.outline.translated(dx, dy);
    let footprints = room
        .members(&board.footprints)
        .filter(|fp| !fp.locked)
        .map(|fp| (fp.uuid, Point::new(fp.position.x + dx, fp.position.y + dy)))
        .collect();
    place(board, id, outline, footprints)
}

/// Fit the room round its members' bodies plus its margin. Refused for a
/// locked room and a room without members.
pub(super) fn fit(board: &mut PcbBoard, id: Uuid) -> Result<Applied, BoardCommandError> {
    let room = &board.rooms[index_of(board, id)?];
    if room.locked {
        return Err(BoardCommandError::RoomLocked(id));
    }
    let outline = fitted_outline(room, board).ok_or(BoardCommandError::RoomHasNoParts(id))?;
    place(board, id, outline, Vec::new())
}

/// Pack the unlocked members outside the room into it.
pub(super) fn pull(board: &mut PcbBoard, id: Uuid) -> Result<Applied, BoardCommandError> {
    let room = &board.rooms[index_of(board, id)?];
    let outline = room.outline.clone();
    let moves = pull_moves(room, board);
    place(board, id, outline, moves)
}
