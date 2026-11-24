//! #2870: a track, a pour and a drawing moved by one offset as one undo
//! step; a locked item refuses the whole move.

use std::collections::BTreeSet;

use signex_types::pcb::{PcbBoard, Point};
use uuid::Uuid;

use super::*;
use crate::pcb::BoardEngine;

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

/// A track, a pour with a hole and a line drawing.
fn board() -> PcbBoard {
    serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(1),
        "nets": [{"number": 3, "name": "GND"}],
        "segments": [{"uuid": Uuid::from_u128(2), "start": {"x": 0.0, "y": 0.0},
                      "end": {"x": 10.0, "y": 0.0}, "width": 0.5,
                      "layer": "TopCopper", "net": 3}],
        "zones": [{"uuid": Uuid::from_u128(3), "net": 3, "net_name": "GND",
                   "layer": "TopCopper", "fill_type": "solid",
                   "outline": [{"x": 0.0, "y": 0.0}, {"x": 20.0, "y": 0.0},
                               {"x": 20.0, "y": 20.0}, {"x": 0.0, "y": 20.0}],
                   "holes": [[{"x": 5.0, "y": 5.0}, {"x": 6.0, "y": 5.0},
                              {"x": 6.0, "y": 6.0}]]}],
        "graphics": [{"uuid": Uuid::from_u128(4), "graphic_type": "line",
                      "layer": "TopSilkscreen", "width": 0.1,
                      "start": {"x": 1.0, "y": 1.0}, "end": {"x": 2.0, "y": 1.0}}]
    }))
    .unwrap()
}

#[test]
fn items_move_by_one_offset_as_one_undo_step() {
    let before = board();
    let ids = BTreeSet::from([
        BoardItemId::Segment(Uuid::from_u128(2)),
        BoardItemId::Zone(Uuid::from_u128(3)),
        BoardItemId::Graphic(Uuid::from_u128(4)),
    ]);
    let commands = move_items(&before, &ids, p(1.0, 2.0)).unwrap();
    assert_eq!(commands.len(), 1, "one ReplaceItems");
    let mut engine = BoardEngine::new(before.clone(), 0);
    engine.execute_batch(commands).expect("the move applies");
    let after = engine.board();
    assert_eq!(after.segments[0].start, p(1.0, 2.0));
    assert_eq!(after.segments[0].end, p(11.0, 2.0));
    assert_eq!(after.zones[0].outline[2], p(21.0, 22.0));
    assert_eq!(after.zones[0].holes[0][0], p(6.0, 7.0));
    assert_eq!(after.graphics[0].end, Some(p(3.0, 3.0)));
    engine.undo().expect("undo");
    let json = |b: &PcbBoard| serde_json::to_value(b).unwrap();
    assert_eq!(json(engine.board()), json(&before), "one undo step");
    assert!(move_items(&before, &ids, p(0.0, 0.0)).unwrap().is_empty());
}

#[test]
fn a_locked_item_refuses_the_whole_move() {
    let mut b = board();
    b.segments[0].locked = true;
    let ids = BTreeSet::from([
        BoardItemId::Segment(Uuid::from_u128(2)),
        BoardItemId::Graphic(Uuid::from_u128(4)),
    ]);
    assert!(matches!(
        move_items(&b, &ids, p(1.0, 0.0)),
        Err(PlacementRefusal::Locked(_))
    ));
    assert!(matches!(
        move_items(&b, &BTreeSet::new(), p(1.0, 0.0)),
        Err(PlacementRefusal::NothingSelected)
    ));
}
