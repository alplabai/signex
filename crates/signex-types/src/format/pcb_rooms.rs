//! The optional `[[rooms]]` array of a `.snxpcb` (#2457): the board's
//! rooms (`pcb::room`).
//!
//! Written only when the board has rooms, so a board without any re-saves
//! byte-identical and a file from before rooms loads unchanged. A board with
//! rooms writes `snxpcb/1.6` (`format::snxpcb_token`), so a build that knows
//! only 1.5 opens it read-only. Lengths are integer nm like every other
//! on-disk geometry. See `docs/PCB_ROOMS.md`, "File format".

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::FormatError;
use super::units::{NonFiniteField, mm_to_nm, nm_to_mm};
use crate::pcb::room::is_room_colour;
use crate::pcb::{PcbBoard, Point, Room, RoomLayer, RoomOutline, RoomSource};
use crate::schematic::InstancePath;

const ROOMS: &str = "rooms";
const RECTANGLE: &str = "rectangle";
const POLYGON: &str = "polygon";

/// One `[[rooms]]` entry as stored.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(in crate::format) struct RoomRaw {
    uuid: Uuid,
    name: String,
    /// The sheet placement, in the `/`-joined instance path form.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sheet: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    channel: Option<u32>,
    layer: String,
    shape: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    x: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    y: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    width: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    height: Option<i64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    points: Vec<[i64; 2]>,
    colour: String,
    #[serde(default)]
    keep_parts_inside: bool,
    #[serde(default)]
    margin: i64,
    #[serde(default)]
    locked: bool,
}

fn refused(row: usize, field: &str, message: impl Into<String>) -> FormatError {
    FormatError::TsvFieldParse {
        block: ROOMS.to_string(),
        row,
        field: field.to_string(),
        message: message.into(),
    }
}

/// The rooms of the file; none when it has no `[[rooms]]`. A room that
/// cannot be read is refused, naming its row.
pub(in crate::format) fn read(raw: Option<Vec<RoomRaw>>) -> Result<Vec<Room>, FormatError> {
    raw.unwrap_or_default()
        .into_iter()
        .enumerate()
        .map(|(row, r)| read_room(row, r))
        .collect()
}

fn read_room(row: usize, r: RoomRaw) -> Result<Room, FormatError> {
    let source = match r.sheet {
        Some(text) => Some(RoomSource {
            sheet: InstancePath::parse(&text)
                .ok_or_else(|| refused(row, "sheet", format!("{text:?} is not a sheet path")))?,
            channel: r.channel,
        }),
        None if r.channel.is_some() => {
            return Err(refused(row, "channel", "a channel needs a sheet"));
        }
        None => None,
    };
    let layer = RoomLayer::from_token(&r.layer).ok_or_else(|| {
        refused(
            row,
            "layer",
            format!("{:?} is not top, bottom or both", r.layer),
        )
    })?;
    let outline = match r.shape.as_str() {
        RECTANGLE => {
            let cell = |v: Option<i64>, field: &str| {
                v.ok_or_else(|| refused(row, field, "a rectangle room needs it"))
            };
            RoomOutline::Rect {
                x_mm: nm_to_mm(cell(r.x, "x")?),
                y_mm: nm_to_mm(cell(r.y, "y")?),
                w_mm: nm_to_mm(cell(r.width, "width")?),
                h_mm: nm_to_mm(cell(r.height, "height")?),
            }
        }
        POLYGON => RoomOutline::Polygon(
            r.points
                .iter()
                .map(|[x, y]| Point::new(nm_to_mm(*x), nm_to_mm(*y)))
                .collect(),
        ),
        other => {
            return Err(refused(
                row,
                "shape",
                format!("{other:?} is not rectangle or polygon"),
            ));
        }
    };
    if !outline.has_area() {
        return Err(refused(
            row,
            "shape",
            "a room needs a positive size or three corners",
        ));
    }
    if !is_room_colour(&r.colour) {
        return Err(refused(
            row,
            "colour",
            format!("{:?} is not #rrggbb", r.colour),
        ));
    }
    if r.margin < 0 {
        return Err(refused(row, "margin", "a margin cannot be negative"));
    }
    Ok(Room {
        uuid: r.uuid,
        name: r.name,
        source,
        layer,
        outline,
        colour: r.colour,
        keep_parts_inside: r.keep_parts_inside,
        margin_mm: nm_to_mm(r.margin),
        locked: r.locked,
    })
}

fn write_room(room: &Room) -> Result<RoomRaw, NonFiniteField> {
    let (shape, x, y, width, height, points) = match &room.outline {
        RoomOutline::Rect {
            x_mm,
            y_mm,
            w_mm,
            h_mm,
        } => (
            RECTANGLE,
            Some(mm_to_nm(*x_mm, "x")?),
            Some(mm_to_nm(*y_mm, "y")?),
            Some(mm_to_nm(*w_mm, "width")?),
            Some(mm_to_nm(*h_mm, "height")?),
            Vec::new(),
        ),
        RoomOutline::Polygon(corners) => (
            POLYGON,
            None,
            None,
            None,
            None,
            corners
                .iter()
                .map(|p| Ok([mm_to_nm(p.x, "points")?, mm_to_nm(p.y, "points")?]))
                .collect::<Result<Vec<_>, NonFiniteField>>()?,
        ),
    };
    Ok(RoomRaw {
        uuid: room.uuid,
        name: room.name.clone(),
        sheet: room.source.as_ref().map(|s| s.sheet.to_string()),
        channel: room.source.as_ref().and_then(|s| s.channel),
        layer: room.layer.token().to_string(),
        shape: shape.to_string(),
        x,
        y,
        width,
        height,
        points,
        colour: room.colour.clone(),
        keep_parts_inside: room.keep_parts_inside,
        margin: mm_to_nm(room.margin_mm, "margin")?,
        locked: room.locked,
    })
}

/// Append `[[rooms]]` to `out`; nothing for a board without rooms.
pub(in crate::format) fn write(out: &mut String, board: &PcbBoard) -> Result<(), FormatError> {
    if board.rooms.is_empty() {
        return Ok(());
    }
    let rooms = board
        .rooms
        .iter()
        .enumerate()
        .map(|(row, room)| {
            write_room(room).map_err(|e| FormatError::NonFiniteField {
                block: ROOMS.to_string(),
                row,
                field: e.0.to_string(),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    #[derive(Serialize)]
    struct Wrapper {
        rooms: Vec<RoomRaw>,
    }
    out.push('\n');
    out.push_str(&toml::to_string_pretty(&Wrapper { rooms })?);
    Ok(())
}
