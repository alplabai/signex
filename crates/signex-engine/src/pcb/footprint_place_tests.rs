use signex_types::object_id::UuidV7Ids;
use signex_types::pcb::{PcbBoard, Point};
use uuid::Uuid;

use super::*;
use crate::pcb::BoardEngine;

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

fn corner() -> CornerRule {
    pcb_geom::stretch_rule(CornerRule::default())
}

/// U1 at (10, 10) and U2 at (20, 10), each with pad 1 one mm to the right of
/// its origin, joined by a track from U1's pad (11, 10) to U2's (21, 10);
/// U3 locked at (30, 30).
fn board() -> PcbBoard {
    let footprint = |n: u128, x: f64, y: f64, locked: bool| {
        serde_json::json!({
            "uuid": id(n), "reference": format!("U{n}"), "position": {"x": x, "y": y},
            "layer": "TopCopper", "locked": locked,
            "pads": [{
                "uuid": id(n * 16), "number": "1", "pad_type": "smd", "shape": "rect",
                "position": {"x": 1.0, "y": 0.0}, "size": {"x": 1.0, "y": 0.5},
                "drill": null, "layers": ["TopCopper"]
            }]
        })
    };
    serde_json::from_value(serde_json::json!({
        "uuid": id(1),
        "footprints": [
            footprint(1, 10.0, 10.0, false),
            footprint(2, 20.0, 10.0, false),
            footprint(3, 30.0, 30.0, true)
        ],
        "segments": [{
            "uuid": id(900), "start": {"x": 11.0, "y": 10.0}, "end": {"x": 21.0, "y": 10.0},
            "width": 0.25, "layer": "TopCopper", "net": 0
        }]
    }))
    .unwrap()
}

fn at(board: &PcbBoard, n: u128) -> Point {
    board
        .footprints
        .iter()
        .find(|f| f.uuid == id(n))
        .unwrap()
        .position
}

fn place(n: u128, x: f64, y: f64) -> FootprintPlace {
    FootprintPlace {
        footprint: id(n),
        to: Point::new(x, y),
    }
}

#[test]
fn a_footprint_goes_to_its_point_with_its_track_end_in_one_step() {
    let b = board();
    let before = b.clone();
    let commands =
        position_commands(&b, &[place(2, 20.0, 14.0)], corner(), &mut UuidV7Ids).unwrap();
    let mut engine = BoardEngine::new(b, 1);
    engine.execute_batch(commands).expect("the batch runs");
    assert_eq!(at(engine.board(), 2), Point::new(20.0, 14.0));
    let ends: Vec<Point> = engine.board().segments.iter().map(|s| s.end).collect();
    assert!(
        ends.contains(&Point::new(21.0, 14.0)),
        "the track follows the pad: {ends:?}"
    );
    engine.undo().expect("undo runs");
    assert_eq!(engine.board().segments, before.segments, "one step");
    assert_eq!(at(engine.board(), 2), Point::new(20.0, 10.0));
}

#[test]
fn a_list_moves_every_footprint_and_a_shared_track_follows_both() {
    let b = board();
    let places = [place(1, 10.0, 20.0), place(2, 20.0, 20.0)];
    let commands = position_commands(&b, &places, corner(), &mut UuidV7Ids).unwrap();
    let mut engine = BoardEngine::new(b, 1);
    engine.execute_batch(commands).expect("the batch runs");
    assert_eq!(at(engine.board(), 1), Point::new(10.0, 20.0));
    assert_eq!(at(engine.board(), 2), Point::new(20.0, 20.0));
    // The track (and any corner leg it took) reaches both moved pads.
    let ends: Vec<Point> = engine
        .board()
        .segments
        .iter()
        .flat_map(|s| [s.start, s.end])
        .collect();
    for pad in [Point::new(11.0, 20.0), Point::new(21.0, 20.0)] {
        assert!(ends.contains(&pad), "{pad:?} not reached: {ends:?}");
    }
    assert!(
        !ends.contains(&Point::new(11.0, 10.0)) && !ends.contains(&Point::new(21.0, 10.0)),
        "no end left on an old pad place: {ends:?}"
    );
}

#[test]
fn a_locked_missing_or_repeated_footprint_refuses_the_whole_list() {
    let b = board();
    let locked = position_commands(
        &b,
        &[place(1, 0.0, 0.0), place(3, 0.0, 0.0)],
        corner(),
        &mut UuidV7Ids,
    );
    assert!(
        matches!(&locked, Err(PlaceRefusal::Placement(PlacementRefusal::Locked(names))) if names == &["U3".to_string()]),
        "{locked:?}"
    );
    let missing = position_commands(&b, &[place(7, 0.0, 0.0)], corner(), &mut UuidV7Ids);
    assert!(missing.is_err());
    let twice = position_commands(
        &b,
        &[place(1, 0.0, 0.0), place(1, 1.0, 0.0)],
        corner(),
        &mut UuidV7Ids,
    );
    assert_eq!(twice, Err(PlaceRefusal::Repeated(id(1))));
}

#[test]
fn a_footprint_already_at_its_point_gets_no_command() {
    let b = board();
    let commands =
        position_commands(&b, &[place(1, 10.0, 10.0)], corner(), &mut UuidV7Ids).unwrap();
    assert!(commands.is_empty(), "{commands:?}");
}
