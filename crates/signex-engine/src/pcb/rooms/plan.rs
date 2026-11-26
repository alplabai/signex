//! Create Rooms from Sheets (#2457): one room per sheet placement, named
//! after the sheet, `<sheet>_CH1..CHN` for a sheet placed N times. Pure: the
//! board and the sheets go in, the rooms and the part moves come out; the
//! app runs [`RoomPlan::commands`] as one undo step and draws the same plan
//! as the dialog's preview. Rules: `docs/PCB_ROOMS.md`, "Create Rooms from
//! Sheets".

use signex_types::object_id::IdSource;
use signex_types::pcb::room::palette_colour;
use signex_types::pcb::{Footprint, PcbBoard, Point, Room, RoomLayer, RoomOutline, RoomSource};
use signex_types::schematic::InstancePath;
use uuid::Uuid;

use super::{BoxMm, inner_box, pack_outside, part_box};
use crate::pcb::BoardCommand;
use crate::pcb::board_tools::ARRANGE_GAP_MM;

/// How the planned rooms are drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlanShape {
    /// Round the members already on the board, plus the margin; a sheet
    /// with none on the board gets a grid cell.
    #[default]
    AroundParts,
    /// The board split into an even grid, one cell per room.
    EvenGrid,
}

/// The dialog's options.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoomOptions {
    pub shape: PlanShape,
    pub margin_mm: f64,
    pub layer: RoomLayer,
    /// Pack the members outside their room into it.
    pub move_parts: bool,
    /// Turn on Keep parts inside for every new room.
    pub keep_rule: bool,
    /// Replace a room with a planned room's name; else the old room stays and
    /// the sheet gets none.
    pub replace_same_name: bool,
}

impl Default for RoomOptions {
    fn default() -> Self {
        RoomOptions {
            shape: PlanShape::AroundParts,
            margin_mm: signex_types::pcb::room::ROOM_DEFAULT_MARGIN_MM,
            layer: RoomLayer::Top,
            move_parts: true,
            keep_rule: true,
            replace_same_name: false,
        }
    }
}

/// One sheet of the design and where it is placed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetMembers {
    /// The sheet's name, which names its rooms.
    pub name: String,
    /// Its file, as the dialog lists it.
    pub file: String,
    /// Every placement of the sheet, in design order.
    pub placements: Vec<InstancePath>,
}

/// A room the plan makes.
#[derive(Debug, Clone, PartialEq)]
pub struct PlannedRoom {
    pub room: Room,
    /// The sheet file it is for.
    pub file: String,
    /// Its members on the board.
    pub members: Vec<Uuid>,
    /// The room of the same name it replaces.
    pub replaces: Option<Uuid>,
}

/// What Create Rooms from Sheets does.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RoomPlan {
    pub rooms: Vec<PlannedRoom>,
    /// Footprint origins: the members packed into their rooms.
    pub moves: Vec<(Uuid, Point)>,
    /// Rooms already on the board whose names the plan would have taken;
    /// their sheets get no new room.
    pub kept: Vec<String>,
}

impl RoomPlan {
    /// The commands that make the plan, to run as one undo step: the
    /// replaced rooms go, the new rooms come, then the parts move.
    pub fn commands(&self) -> Vec<BoardCommand> {
        let removed = self
            .rooms
            .iter()
            .filter_map(|p| p.replaces)
            .map(|room| BoardCommand::RemoveRoom { room });
        let added = self.rooms.iter().map(|p| BoardCommand::AddRoom {
            room: Box::new(p.room.clone()),
        });
        let moved = self
            .moves
            .iter()
            .map(|&(footprint, to)| BoardCommand::MoveFootprint { footprint, to });
        removed.chain(added).chain(moved).collect()
    }

    /// Keep only the rooms whose name `keep` accepts, and the moves of
    /// their members: the rooms a person left unticked, or a command left
    /// out, are not made.
    pub fn retain_rooms(&mut self, keep: impl Fn(&str) -> bool) {
        self.rooms.retain(|p| keep(&p.room.name));
        let members: std::collections::BTreeSet<Uuid> = self
            .rooms
            .iter()
            .flat_map(|p| p.members.iter().copied())
            .collect();
        self.moves.retain(|(fp, _)| members.contains(fp));
    }

    /// How many rooms are channels of a sheet placed more than once.
    pub fn channel_rooms(&self) -> usize {
        self.rooms
            .iter()
            .filter(|p| p.room.source.as_ref().is_some_and(|s| s.channel.is_some()))
            .count()
    }
}

/// The rooms `sheets` get on `board` with `options`; `ids` names each.
pub fn plan_rooms_from_sheets(
    board: &PcbBoard,
    sheets: &[SheetMembers],
    options: &RoomOptions,
    ids: &mut dyn IdSource,
) -> RoomPlan {
    let region = board_region(board);
    let mut plan = RoomPlan::default();
    let mut drafts: Vec<Draft> = Vec::new();
    let names: Vec<String> = sheets.iter().flat_map(room_names).collect();
    let staying = board
        .rooms
        .iter()
        .filter(|r| !names.contains(&r.name))
        .count();
    for (s, sheet) in sheets.iter().enumerate() {
        let colour = palette_colour(staying + s);
        let count = sheet.placements.len();
        for (i, placement) in sheet.placements.iter().enumerate() {
            let channel = (count > 1).then_some(i as u32 + 1);
            let name = match channel {
                Some(n) => format!("{}_CH{n}", sheet.name),
                None => sheet.name.clone(),
            };
            let existing = board.rooms.iter().find(|r| r.name == name);
            if existing.is_some() && !options.replace_same_name {
                plan.kept.push(name);
                continue;
            }
            let source = RoomSource {
                sheet: placement.clone(),
                channel,
            };
            let members: Vec<&Footprint> = board
                .footprints
                .iter()
                .filter(|fp| {
                    fp.schematic_link.as_ref().is_some_and(|link| {
                        signex_types::pcb::room::is_under(&link.sheet, &source.sheet)
                    })
                })
                .collect();
            drafts.push(Draft {
                name,
                file: sheet.file.clone(),
                colour: existing.map_or(colour.to_string(), |r| r.colour.clone()),
                source,
                members,
                replaces: existing.map(|r| r.uuid),
            });
        }
    }
    let cells = grid_cells(region, drafts.len());
    for (k, draft) in drafts.into_iter().enumerate() {
        let on_board: Vec<BoxMm> = draft
            .members
            .iter()
            .map(|fp| part_box(fp))
            .filter(|b| region.holds(b))
            .collect();
        let around = on_board
            .iter()
            .copied()
            .reduce(|a, b| a.union(&b))
            .map(|b| b.inflate(options.margin_mm));
        let (area, grows) = match (options.shape, around) {
            (PlanShape::AroundParts, Some(around)) => (around, true),
            _ => (cells[k], false),
        };
        let mut room = Room {
            source: Some(draft.source),
            layer: options.layer,
            keep_parts_inside: options.keep_rule,
            margin_mm: options.margin_mm,
            ..Room::drawn(
                ids.next_id(),
                draft.name,
                &draft.colour,
                RoomOutline::rect_between(area.min, area.max),
            )
        };
        if options.move_parts {
            let packed = inner_box(&room)
                .map(|inner| pack_outside(board, inner, draft.members.clone()))
                .unwrap_or_default();
            if grows {
                // A room round its parts grows over the rows packed below it.
                let edge = ARRANGE_GAP_MM + options.margin_mm.max(0.0);
                let grown = packed
                    .iter()
                    .fold(area, |a, p| a.union(&p.covers.inflate(edge)));
                room.outline = RoomOutline::rect_between(grown.min, grown.max);
            }
            plan.moves
                .extend(packed.iter().map(|p| (p.footprint, p.to)));
        }
        plan.rooms.push(PlannedRoom {
            members: draft.members.iter().map(|fp| fp.uuid).collect(),
            room,
            file: draft.file,
            replaces: draft.replaces,
        });
    }
    plan
}

/// A planned room before its outline is known.
struct Draft<'a> {
    name: String,
    file: String,
    colour: String,
    source: RoomSource,
    members: Vec<&'a Footprint>,
    replaces: Option<Uuid>,
}

/// The names `sheet`'s rooms take.
fn room_names(sheet: &SheetMembers) -> Vec<String> {
    match sheet.placements.len() {
        0 => Vec::new(),
        1 => vec![sheet.name.clone()],
        n => (1..=n).map(|i| format!("{}_CH{i}", sheet.name)).collect(),
    }
}

/// The board's box: its outline, else its parts, else 100 by 80 mm.
pub(crate) fn board_region(board: &PcbBoard) -> BoxMm {
    let from_points = |points: &mut dyn Iterator<Item = Point>| {
        points
            .map(|p| BoxMm { min: p, max: p })
            .reduce(|a, b| a.union(&b))
    };
    from_points(&mut board.outline.iter().copied())
        .filter(|b| b.width() > 0.0 && b.height() > 0.0)
        .or_else(|| {
            board
                .footprints
                .iter()
                .map(part_box)
                .reduce(|a, b| a.union(&b))
        })
        .unwrap_or(BoxMm {
            min: Point::ZERO,
            max: Point::new(100.0, 80.0),
        })
}

/// `count` cells splitting `region` evenly, row by row, each inset by the
/// arrange gap so neighbouring rooms do not touch.
fn grid_cells(region: BoxMm, count: usize) -> Vec<BoxMm> {
    if count == 0 {
        return Vec::new();
    }
    let aspect = (region.width() / region.height().max(f64::MIN_POSITIVE)).max(f64::MIN_POSITIVE);
    let columns = ((count as f64 * aspect).sqrt().ceil() as usize).clamp(1, count);
    let rows = count.div_ceil(columns);
    let (w, h) = (
        region.width() / columns as f64,
        region.height() / rows as f64,
    );
    (0..count)
        .map(|k| {
            let (col, row) = ((k % columns) as f64, (k / columns) as f64);
            let min = Point::new(region.min.x + col * w, region.min.y + row * h);
            BoxMm {
                min,
                max: Point::new(min.x + w, min.y + h),
            }
            .inflate(-ARRANGE_GAP_MM / 2.0)
        })
        .collect()
}
