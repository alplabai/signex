//! Part outside its room (#2457): a member part of a room with Keep parts
//! inside whose body is not inside the room's outline shrunk by its margin.
//! The body is the box round `pcb_geom::checked_body` (the courtyard outline
//! without its display stroke, else the pads); the outline is the
//! room's rectangle or polygon. Only rooms with the rule on are checked, so a
//! board without such rooms gets the report it got before. See
//! `docs/PCB_ROOMS.md`, "Design rule".

use pcb_geom::{GeomError, ItemRef, PointNm, Polygon, Rejected, checked_body, mm_to_nm};
use signex_types::pcb::{Footprint, PcbBoard, Room};

use crate::findings::mm;
use crate::{Violation, ViolationKind};

/// How far `footprint`'s body reaches past `room`'s outline shrunk by its
/// margin, nm; 0 when it is inside. An error when the body or the outline
/// does not convert.
pub fn overhang_nm(room: &Room, footprint: &Footprint) -> Result<i64, GeomError> {
    let ring = room
        .outline
        .corners()
        .into_iter()
        .map(|p| PointNm::from_mm(p, "room.outline"))
        .collect::<Result<Vec<_>, _>>()?;
    let margin = mm_to_nm(room.margin_mm.max(0.0), "room.margin")?;
    let body = checked_body(footprint)?.aabb();
    let region = Polygon::new(ring.clone(), Vec::new());
    let worst = body
        .corners()
        .into_iter()
        .map(|p| {
            let edge = distance_to_ring(&ring, p);
            let inside = if region.contains_point(p) {
                edge
            } else {
                -edge
            };
            margin - inside
        })
        .max()
        .unwrap_or(0);
    Ok(worst.max(0))
}

/// The distance from `p` to the nearest edge of the closed `ring`, whole nm
/// rounded up so a part a hair outside never reads as on the edge.
fn distance_to_ring(ring: &[PointNm], p: PointNm) -> i64 {
    (0..ring.len())
        .map(|i| segment_distance(ring[i], ring[(i + 1) % ring.len()], p))
        .fold(f64::INFINITY, f64::min)
        .ceil() as i64
}

fn segment_distance(a: PointNm, b: PointNm, p: PointNm) -> f64 {
    let (dx, dy) = ((b.x - a.x) as f64, (b.y - a.y) as f64);
    let (px, py) = ((p.x - a.x) as f64, (p.y - a.y) as f64);
    let length_sq = dx * dx + dy * dy;
    let t = if length_sq > 0.0 {
        ((px * dx + py * dy) / length_sq).clamp(0.0, 1.0)
    } else {
        0.0
    };
    ((px - t * dx).powi(2) + (py - t * dy).powi(2)).sqrt()
}

/// Every part outside its room on `board`, unsorted, and the parts whose
/// body could not be measured.
pub(crate) fn violations(board: &PcbBoard) -> (Vec<Violation>, Vec<Rejected>) {
    let mut found = Vec::new();
    let mut skipped: Vec<Rejected> = Vec::new();
    for room in board.rooms.iter().filter(|r| r.keep_parts_inside) {
        let margin = mm_to_nm(room.margin_mm.max(0.0), "room.margin").unwrap_or(0);
        for footprint in room.members(&board.footprints) {
            let item = ItemRef::Footprint(footprint.uuid);
            match overhang_nm(room, footprint) {
                Ok(0) => {}
                Ok(overhang) => {
                    let at = PointNm::from_mm(footprint.position, "footprint.position")
                        .unwrap_or(PointNm::new(0, 0));
                    found.push(Violation {
                        kind: ViolationKind::PartOutsideRoom,
                        items: vec![item, ItemRef::Room(room.uuid)],
                        nets: (0, 0),
                        layers: 0,
                        at,
                        measured_nm: overhang,
                        required_nm: margin,
                        keepout: None,
                        rule: None,
                    });
                }
                Err(error) if !skipped.iter().any(|r| r.item == item) => {
                    skipped.push(Rejected { item, error });
                }
                Err(_) => {}
            }
        }
    }
    (found, skipped)
}

/// `R2 is 1.2 mm outside room dcdc` for a part outside its room on `board`;
/// `None` for any other finding.
pub fn describe_on(board: &PcbBoard, v: &Violation) -> Option<String> {
    if v.kind != ViolationKind::PartOutsideRoom {
        return None;
    }
    let mut part = None;
    let mut room = None;
    for item in &v.items {
        match *item {
            ItemRef::Footprint(id) => {
                part = board.footprints.iter().find(|f| f.uuid == id);
            }
            ItemRef::Room(id) => room = board.rooms.iter().find(|r| r.uuid == id),
            _ => {}
        }
    }
    Some(format!(
        "{} is {} mm outside room {}",
        part.map_or("a part", |f| f.reference.as_str()),
        mm(v.measured_nm),
        room.map_or("?", |r| r.name.as_str())
    ))
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use signex_types::pcb::{RoomOutline, RoomSource};
    use signex_types::schematic::InstancePath;
    use uuid::Uuid;

    use super::*;
    use crate::{DrcRules, check};

    /// A 0.5 mm square pad part, R2, linked to sheet 0xA, at `x`.
    fn board(x: f64) -> PcbBoard {
        serde_json::from_value(json!({
            "uuid": Uuid::nil(),
            "footprints": [{
                "uuid": Uuid::from_u128(2), "reference": "R2", "layer": "TopCopper",
                "position": {"x": x, "y": 5.0},
                "pads": [{
                    "uuid": Uuid::from_u128(20), "number": "1", "pad_type": "smd",
                    "shape": "rect", "position": {"x": 0.0, "y": 0.0},
                    "size": {"x": 0.5, "y": 0.5}, "drill": null,
                    "layers": ["TopCopper"], "net": null
                }],
                "schematic_link": {"sheet": [Uuid::from_u128(0xA)], "units": [Uuid::from_u128(9)]}
            }]
        }))
        .unwrap()
    }

    fn with_room(mut b: PcbBoard, keep: bool) -> PcbBoard {
        let mut room = Room::drawn(
            Uuid::from_u128(100),
            "dcdc".into(),
            "#A058C8",
            RoomOutline::Rect {
                x_mm: 0.0,
                y_mm: 0.0,
                w_mm: 10.0,
                h_mm: 10.0,
            },
        );
        room.source = Some(RoomSource {
            sheet: InstancePath::from_chain(vec![Uuid::from_u128(0xA)]),
            channel: None,
        });
        room.keep_parts_inside = keep;
        room.margin_mm = 0.5;
        b.rooms.push(room);
        b
    }

    fn found(b: &PcbBoard) -> Vec<Violation> {
        check::run(b, &DrcRules::new(200_000))
            .violations
            .into_iter()
            .filter(|v| v.kind == ViolationKind::PartOutsideRoom)
            .collect()
    }

    #[test]
    fn a_part_inside_its_room_is_clean() {
        assert!(found(&with_room(board(5.0), true)).is_empty());
    }

    #[test]
    fn a_part_outside_its_room_is_named_with_the_distance() {
        // The body reaches x = 11.25; the room's inside ends at 9.5.
        let b = with_room(board(11.0), true);
        let v = found(&b);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].measured_nm, 1_750_000);
        assert_eq!(v[0].required_nm, 500_000);
        assert_eq!(
            v[0].items,
            vec![
                ItemRef::Footprint(Uuid::from_u128(2)),
                ItemRef::Room(Uuid::from_u128(100))
            ]
        );
        assert_eq!(
            describe_on(&b, &v[0]).as_deref(),
            Some("R2 is 1.75 mm outside room dcdc")
        );
    }

    /// #3058: Run DRC's Messages line names the part and the room, never
    /// "net 0 / net 0", and says the same as the check report's message.
    #[test]
    fn the_messages_line_names_the_part_and_the_room() {
        let b = with_room(board(11.0), true);
        let v = found(&b);
        let names = std::collections::BTreeMap::new();
        let line = crate::findings::messages_line(&v[0], Some(&b), &names);
        assert_eq!(
            line,
            format!(
                "DRC: R2 is 1.75 mm outside room dcdc at ({}, {}) mm",
                mm(v[0].at.x),
                mm(v[0].at.y)
            )
        );
        assert!(!line.contains("net 0"), "{line}");
        let report = crate::findings::check_report(
            &check::run(&b, &DrcRules::new(200_000)),
            &b,
            &names,
            "board.snxpcb",
            Vec::new(),
            serde_json::Map::new(),
        );
        let messages: Vec<&str> = report
            .findings
            .iter()
            .filter(|f| f.rule == ViolationKind::PartOutsideRoom.rule_id())
            .map(|f| f.message.as_str())
            .collect();
        assert_eq!(messages, ["R2 is 1.75 mm outside room dcdc"]);
    }

    #[test]
    fn a_part_inside_the_margin_is_a_finding() {
        // The body reaches x = 9.75: inside the room, 0.25 mm into the margin.
        let v = found(&with_room(board(9.5), true));
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].measured_nm, 250_000);
    }

    /// R2 at `x` with a 2 mm square courtyard round its origin, drawn at
    /// `width`.
    fn with_courtyard(x: f64, width: f64) -> PcbBoard {
        let mut b = board(x);
        let courtyard: signex_types::pcb::FpGraphic = serde_json::from_value(json!({
            "graphic_type": "rect", "layer": "TopCourtyard", "width": width,
            "start": {"x": -1.0, "y": -1.0}, "end": {"x": 1.0, "y": 1.0}}))
        .expect("courtyard");
        b.footprints[0].graphics.push(courtyard);
        with_room(b, true)
    }

    fn overhangs(b: &PcbBoard) -> Vec<i64> {
        found(b).iter().map(|v| v.measured_nm).collect()
    }

    /// DRC never counts the courtyard stroke (Caner, 2026-11-24): the
    /// outline reaching 10 mm is 0.5 mm past the inside line at either width.
    #[test]
    fn the_room_overhang_is_the_same_at_stroke_0_and_0_05() {
        let thin = overhangs(&with_courtyard(9.0, 0.0));
        assert_eq!(thin, [500_000]);
        assert_eq!(overhangs(&with_courtyard(9.0, 0.05)), thin);
        // 0.01 mm inside; half a 0.05 mm stroke would take it past.
        assert!(overhangs(&with_courtyard(8.49, 0.0)).is_empty());
        assert!(overhangs(&with_courtyard(8.49, 0.05)).is_empty());
    }

    #[test]
    fn a_room_without_the_rule_is_not_checked() {
        assert!(found(&with_room(board(30.0), false)).is_empty());
    }

    #[test]
    fn a_board_without_rooms_gets_the_same_report() {
        let bare = board(30.0);
        let off = with_room(board(30.0), false);
        let rules = DrcRules::new(200_000);
        assert_eq!(check::run(&bare, &rules), check::run(&off, &rules));
        assert!(found(&bare).is_empty());
    }
}
