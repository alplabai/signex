//! #2941: Auto Place inside rooms. Every part lands in its room without
//! overlap, the same weights give the same placement, and each priority
//! pulls the result its way when its weight goes up.

use pcb_geom::RectNm;
use serde_json::{Value, json};
use signex_types::pcb::{PcbBoard, Point, Room, RoomOutline, RoomSource};
use signex_types::schematic::InstancePath;
use uuid::Uuid;

use super::*;
use crate::pcb::rooms::arrange::placed_box;
use crate::pcb::rooms::{BoxMm, inner_box};

const SHEET: u128 = 0xA;

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

/// A part with one 2 x 2 mm pad on `net` (0: none), linked to the sheet,
/// with the placement roles `roles`.
fn part(n: u128, reference: &str, at: (f64, f64), net: u32, roles: &str, locked: bool) -> Value {
    let pad_net = if net == 0 {
        Value::Null
    } else {
        json!({"number": net, "name": format!("N{net}")})
    };
    let properties = if roles.is_empty() {
        json!([])
    } else {
        json!([{"key": "Placement", "value": roles}])
    };
    json!({
        "uuid": id(n), "reference": reference,
        "position": {"x": at.0, "y": at.1}, "layer": "TopCopper", "locked": locked,
        "pads": [{"uuid": id(n + 500), "number": "1", "pad_type": "smd", "shape": "circle",
                  "position": {"x": 0.0, "y": 0.0}, "size": {"x": 2.0, "y": 2.0},
                  "drill": null, "layers": ["TopCopper"], "net": pad_net}],
        "properties": properties,
        "schematic_link": {"sheet": [id(SHEET)], "units": [id(n + 900)]},
    })
}

/// A 60 x 40 mm board with `parts` and one room over `(x, y, w, h)`.
fn board(room: (f64, f64, f64, f64), parts: Vec<Value>) -> PcbBoard {
    let mut board: PcbBoard = serde_json::from_value(json!({
        "uuid": id(1),
        "setup": {"grid_size": 0.5},
        "outline": [
            {"x": 0.0, "y": 0.0}, {"x": 60.0, "y": 0.0},
            {"x": 60.0, "y": 40.0}, {"x": 0.0, "y": 40.0}
        ],
        "footprints": parts,
    }))
    .expect("the fixture board");
    let (x, y, w, h) = room;
    let outline = RoomOutline::Rect {
        x_mm: x,
        y_mm: y,
        w_mm: w,
        h_mm: h,
    };
    let mut r = Room::drawn(id(100), "dcdc".into(), "#A058C8", outline);
    r.source = Some(RoomSource {
        sheet: InstancePath::from_chain(vec![id(SHEET)]),
        channel: None,
    });
    board.rooms.push(r);
    board
}

/// Only `priority` weighs, by `weight`.
fn only(priority: Priority, weight: f64) -> AutoPlaceOptions {
    let mut weights = Weights {
        connection: 0.0,
        thermal: 0.0,
        separation: 0.0,
        edges: 0.0,
    };
    match priority {
        Priority::Connection => weights.connection = weight,
        Priority::Thermal => weights.thermal = weight,
        Priority::Separation => weights.separation = weight,
        Priority::Edges => weights.edges = weight,
    }
    AutoPlaceOptions {
        weights,
        ..AutoPlaceOptions::default()
    }
}

/// The board after the plan's moves.
fn placed(board: &PcbBoard, options: &AutoPlaceOptions) -> (PcbBoard, AutoPlacePlan) {
    let plan = auto_place(board, options);
    let mut after = board.clone();
    for m in &plan.moves {
        let fp = after.footprints.iter_mut().find(|f| f.uuid == m.footprint);
        fp.expect("a moved part").position = m.to;
    }
    (after, plan)
}

fn box_of(board: &PcbBoard, n: u128) -> RectNm {
    let fp = board.footprints.iter().find(|f| f.uuid == id(n));
    placed_box(fp.expect("the part")).expect("a box")
}

fn holds(inner: &BoxMm, b: &RectNm) -> bool {
    inner.holds(&BoxMm {
        min: Point::new(pcb_geom::nm_to_mm(b.min.x), pcb_geom::nm_to_mm(b.min.y)),
        max: Point::new(pcb_geom::nm_to_mm(b.max.x), pcb_geom::nm_to_mm(b.max.y)),
    })
}

#[test]
fn every_part_lands_inside_its_room_without_overlap_and_a_rerun_agrees() {
    let parts = vec![
        part(1, "U1", (5.0, 35.0), 1, "", false),
        part(2, "U2", (8.0, 35.0), 1, "hot", false),
        part(3, "U3", (11.0, 35.0), 2, "noisy", false),
        part(4, "U4", (14.0, 35.0), 2, "sensitive", false),
        part(5, "U5", (15.0, 10.0), 0, "", true),
    ];
    let b = board((10.0, 5.0, 30.0, 20.0), parts);
    let options = AutoPlaceOptions::default();
    let (after, plan) = placed(&b, &options);
    assert!(plan.not_placed.is_empty(), "{:?}", plan.not_placed);
    let moved: Vec<Uuid> = plan.moves.iter().map(|m| m.footprint).collect();
    assert_eq!(moved.len(), 4);
    assert!(!moved.contains(&id(5)), "a locked part stays");
    let inner = inner_box(&after.rooms[0]).expect("an inner box");
    let boxes: Vec<RectNm> = (1..=5).map(|n| box_of(&after, n)).collect();
    for (k, b) in boxes.iter().enumerate().take(4) {
        assert!(holds(&inner, b), "U{} is inside its room", k + 1);
    }
    for (i, a) in boxes.iter().enumerate() {
        for b in boxes.iter().skip(i + 1) {
            assert!(!overlaps(a, b), "no two bodies overlap");
        }
    }
    assert_eq!(auto_place(&b, &options), plan, "the same placement");
    let unturned = |m: &PlacedPart| m.room == "dcdc" && m.rotation == 0.0 && !m.bottom;
    assert!(plan.moves.iter().all(unturned));
}

#[test]
fn a_heavier_connection_weight_pulls_a_part_towards_the_parts_it_shares_nets_with() {
    let parts = vec![
        part(1, "U1", (5.0, 35.0), 1, "", false),
        part(9, "U9", (45.0, 15.0), 1, "", true),
    ];
    let b = board((10.0, 10.0, 40.0, 10.0), parts);
    let distance = |options: AutoPlaceOptions| {
        let (after, _) = placed(&b, &options);
        score::centre_distance(&box_of(&after, 1), &box_of(&after, 9))
    };
    let off = distance(only(Priority::Connection, 0.0));
    let on = distance(only(Priority::Connection, 3.0));
    assert!(on < off, "connection pulls U1 to U9: {on} < {off}");
}

#[test]
fn a_heavier_thermal_weight_keeps_two_hot_parts_the_thermal_gap_apart() {
    let parts = vec![
        part(1, "U1", (5.0, 35.0), 0, "hot", false),
        part(9, "U9", (12.0, 15.0), 0, "hot", true),
    ];
    let b = board((10.0, 10.0, 40.0, 10.0), parts);
    let gap = |options: AutoPlaceOptions| {
        let (after, _) = placed(&b, &options);
        score::box_gap(&box_of(&after, 1), &box_of(&after, 9))
    };
    let off = gap(only(Priority::Thermal, 0.0));
    let on = gap(only(Priority::Thermal, 1.0));
    assert!(on > off, "thermal pushes the hot parts apart: {on} > {off}");
    assert!(on >= DEFAULT_THERMAL_GAP_MM - 1e-6, "{on}");
}

#[test]
fn a_heavier_separation_weight_keeps_a_sensitive_part_away_from_a_noisy_one() {
    let parts = vec![
        part(1, "U1", (5.0, 35.0), 0, "sensitive", false),
        part(9, "U9", (12.0, 15.0), 0, "noisy", true),
    ];
    let b = board((10.0, 10.0, 40.0, 10.0), parts);
    let gap = |options: AutoPlaceOptions| {
        let (after, _) = placed(&b, &options);
        score::box_gap(&box_of(&after, 1), &box_of(&after, 9))
    };
    let off = gap(only(Priority::Separation, 0.0));
    let on = gap(only(Priority::Separation, 2.0));
    assert!(on > off, "separation pushes them apart: {on} > {off}");
    assert!(on >= DEFAULT_SEPARATION_GAP_MM - 1e-6, "{on}");
}

#[test]
fn a_heavier_edges_weight_moves_a_connector_towards_the_board_edge() {
    let parts = vec![part(1, "J1", (5.0, 38.0), 0, "", false)];
    let b = board((20.0, 12.0, 20.0, 24.0), parts);
    let region = crate::pcb::rooms::plan::board_region(&b);
    let edge = |options: AutoPlaceOptions| {
        let (after, _) = placed(&b, &options);
        score::edge_distance(&box_of(&after, 1), &region)
    };
    let off = edge(only(Priority::Edges, 0.0));
    let on = edge(only(Priority::Edges, 5.0));
    assert!(on < off, "edges pulls the connector out: {on} < {off}");
}

#[test]
fn the_scores_before_and_after_are_reported_per_priority() {
    let parts = vec![
        part(1, "U1", (5.0, 35.0), 1, "", false),
        part(9, "U9", (45.0, 15.0), 1, "", true),
    ];
    let b = board((10.0, 10.0, 40.0, 10.0), parts);
    let plan = auto_place(&b, &only(Priority::Connection, 1.0));
    assert!(plan.after.connection < plan.before.connection);
    assert_eq!(plan.rooms, vec!["dcdc".to_string()]);
}

#[test]
fn a_part_with_no_free_place_is_reported_and_stays() {
    let parts = vec![
        part(1, "U1", (5.0, 35.0), 0, "", false),
        part(9, "U9", (12.0, 12.0), 0, "", true),
    ];
    // The room is just big enough for the locked part.
    let b = board((10.0, 10.0, 4.0, 4.0), parts);
    let plan = auto_place(&b, &AutoPlaceOptions::default());
    assert!(plan.moves.is_empty());
    assert_eq!(plan.not_placed.len(), 1);
    assert_eq!(plan.not_placed[0].footprint, id(1));
    assert_eq!(plan.not_placed[0].reason, NotPlacedReason::NoSpace);
}

#[test]
fn only_the_rooms_named_are_placed() {
    let parts = vec![part(1, "U1", (5.0, 35.0), 0, "", false)];
    let b = board((10.0, 10.0, 40.0, 10.0), parts);
    let options = AutoPlaceOptions {
        rooms: vec![id(999)],
        ..AutoPlaceOptions::default()
    };
    let plan = auto_place(&b, &options);
    assert!(plan.rooms.is_empty() && plan.moves.is_empty());
}

#[test]
fn weights_read_over_the_defaults_and_refuse_what_is_not_a_weight() {
    let w = Weights::parse("connection:3, edges:5").expect("weights");
    let read = (w.connection, w.thermal, w.separation, w.edges);
    assert_eq!(read, (3.0, 1.0, 1.0, 5.0));
    assert_eq!(Weights::parse("").expect("empty"), Weights::default());
    let bad_lists = [
        "speed:1",
        "thermal:-1",
        "thermal:x",
        "thermal",
        "edges:1,edges:2",
    ];
    for bad in bad_lists {
        assert!(Weights::parse(bad).is_err(), "{bad}");
    }
}

#[test]
fn roles_come_from_the_placement_property_and_connector_designators() {
    let b = board(
        (10.0, 10.0, 40.0, 10.0),
        vec![
            part(1, "U1", (0.0, 0.0), 0, "hot, Sensitive", false),
            part(2, "J3", (0.0, 0.0), 0, "", false),
            part(3, "JP1", (0.0, 0.0), 0, "", false),
        ],
    );
    let roles: Vec<PartRoles> = b.footprints.iter().map(PartRoles::of).collect();
    assert!(roles[0].hot && roles[0].sensitive && !roles[0].noisy);
    assert!(roles[1].connector);
    assert!(!roles[2].connector, "a jumper is no connector");
}

/// #2985: with `rooms=[A]`, a part of the nested room B (a child sheet of
/// A's) is B's, not A's, and B is not placed: it stays where it is.
#[test]
fn a_part_of_a_nested_room_not_named_stays() {
    const CHILD: u128 = 0xB;
    let mut child = part(2, "U2", (40.0, 15.0), 0, "", false);
    child["schematic_link"]["sheet"] = json!([id(SHEET), id(CHILD)]);
    let parts = vec![part(1, "U1", (5.0, 35.0), 0, "", false), child];
    let mut b = board((0.0, 0.0, 60.0, 40.0), parts);
    let outline = RoomOutline::Rect {
        x_mm: 35.0,
        y_mm: 10.0,
        w_mm: 10.0,
        h_mm: 10.0,
    };
    let mut nested = Room::drawn(id(101), "ldo".into(), "#58A0C8", outline);
    nested.source = Some(RoomSource {
        sheet: InstancePath::from_chain(vec![id(SHEET), id(CHILD)]),
        channel: None,
    });
    b.rooms.push(nested);
    let options = AutoPlaceOptions {
        rooms: vec![id(100)],
        ..AutoPlaceOptions::default()
    };
    let plan = auto_place(&b, &options);
    assert_eq!(plan.rooms, ["dcdc"]);
    assert!(
        plan.moves.iter().all(|m| m.footprint != id(2)),
        "U2 is the nested room's: {:?}",
        plan.moves
    );
}
