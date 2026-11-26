//! What the rooms commands answer with (#2935): rooms, their parts and the
//! parts a command moved, in board mm, a point as `[x, y]`. The shapes are
//! in `docs/CLI.md`, "Rooms and placement".

use serde_json::{Value, json};
use signex_engine::pcb::rooms::members_outside;
use signex_types::pcb::{PcbBoard, Point, Room, RoomOutline};
use uuid::Uuid;

pub(super) fn point(p: Point) -> Value {
    json!([p.x, p.y])
}

/// The outline's area, mm²: a polygon's by its corners in order.
fn area_mm2(outline: &RoomOutline) -> f64 {
    match outline {
        RoomOutline::Rect { w_mm, h_mm, .. } => w_mm * h_mm,
        RoomOutline::Polygon(points) => {
            let n = points.len();
            let twice: f64 = (0..n)
                .map(|i| {
                    let (a, b) = (points[i], points[(i + 1) % n]);
                    a.x * b.y - b.x * a.y
                })
                .sum();
            twice.abs() / 2.0
        }
    }
}

fn outline(outline: &RoomOutline) -> Value {
    match outline {
        RoomOutline::Rect {
            x_mm,
            y_mm,
            w_mm,
            h_mm,
        } => json!({
            "shape": "rectangle", "x": x_mm, "y": y_mm, "width": w_mm, "height": h_mm,
        }),
        RoomOutline::Polygon(points) => json!({
            "shape": "polygon",
            "corners": points.iter().map(|p| point(*p)).collect::<Vec<_>>(),
        }),
    }
}

/// A room as the commands answer it: its settings, its outline and area,
/// and each member part, `inside` when its body is inside the outline
/// shrunk by the margin.
pub(super) fn room(room: &Room, board: &PcbBoard) -> Value {
    let outside: Vec<Uuid> = members_outside(room, board)
        .iter()
        .map(|fp| fp.uuid)
        .collect();
    let parts: Vec<Value> = room
        .members(&board.footprints)
        .map(|fp| {
            json!({
                "id": fp.uuid.to_string(),
                "designator": fp.reference,
                "inside": !outside.contains(&fp.uuid),
            })
        })
        .collect();
    json!({
        "id": room.uuid.to_string(),
        "name": room.name,
        "sheet": room.source.as_ref().map(|s| s.sheet.to_string()),
        "channel": room.source.as_ref().and_then(|s| s.channel),
        "side": room.layer.token(),
        "outline": outline(&room.outline),
        "area_mm2": area_mm2(&room.outline),
        "color": room.colour,
        "keep_parts_inside": room.keep_parts_inside,
        "margin_mm": room.margin_mm,
        "locked": room.locked,
        "parts": parts,
    })
}

/// Each footprint of `moves` with where it was on `board` and where it goes.
pub(super) fn moves(board: &PcbBoard, moves: &[(Uuid, Point)]) -> Vec<Value> {
    moves
        .iter()
        .filter_map(|(id, to)| {
            let fp = board.footprints.iter().find(|fp| fp.uuid == *id)?;
            Some(json!({
                "id": id.to_string(),
                "designator": fp.reference,
                "from": point(fp.position),
                "to": point(*to),
            }))
        })
        .collect()
}

/// The designators of the footprints `ids` names, in that order.
pub(super) fn designators(board: &PcbBoard, ids: &[Uuid]) -> Vec<String> {
    ids.iter()
        .filter_map(|id| board.footprints.iter().find(|fp| fp.uuid == *id))
        .map(|fp| fp.reference.clone())
        .collect()
}
