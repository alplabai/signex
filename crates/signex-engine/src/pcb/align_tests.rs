use std::collections::BTreeSet;

use signex_types::pcb::{PcbBoard, Point};
use uuid::Uuid;

use super::*;
use crate::pcb::BoardEngine;

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

/// U1 at (1, 1), U2 at (4, 5), U3 at (10, 2); U4 at (0, 0) is locked.
fn board() -> PcbBoard {
    let fp = |n: u128, x: f64, y: f64, locked: bool| {
        serde_json::json!({"uuid": id(n), "reference": format!("U{n}"),
                           "position": {"x": x, "y": y}, "layer": "TopCopper",
                           "locked": locked})
    };
    serde_json::from_value(serde_json::json!({
        "uuid": id(100),
        "setup": {"grid_size": 1.0},
        "footprints": [fp(1, 1.0, 1.0, false), fp(2, 4.0, 5.0, false),
                       fp(3, 10.0, 2.0, false), fp(4, 0.0, 0.0, true)]
    }))
    .unwrap()
}

fn aligned(to: AlignTo, ids: &[u128]) -> Vec<Point> {
    let mut engine = BoardEngine::new(board(), 1);
    let set: BTreeSet<Uuid> = ids.iter().map(|&n| id(n)).collect();
    let commands = align_commands(engine.board(), &set, to).unwrap();
    engine.execute_batch(commands).unwrap();
    ids.iter()
        .map(|&n| {
            engine
                .board()
                .footprints
                .iter()
                .find(|f| f.uuid == id(n))
                .unwrap()
                .position
        })
        .collect()
}

#[test]
fn footprints_line_up_by_their_origins() {
    let p = Point::new;
    let three = [1, 2, 3];
    assert_eq!(
        aligned(AlignTo::Left, &three),
        [p(1.0, 1.0), p(1.0, 5.0), p(1.0, 2.0)]
    );
    assert_eq!(aligned(AlignTo::Right, &three)[0], p(10.0, 1.0));
    assert_eq!(aligned(AlignTo::Top, &three)[1], p(4.0, 1.0));
    assert_eq!(aligned(AlignTo::Bottom, &three)[2], p(10.0, 5.0));
    assert_eq!(aligned(AlignTo::HorizontalCenters, &three)[0], p(5.5, 1.0));
    assert_eq!(aligned(AlignTo::VerticalCenters, &three)[2], p(10.0, 3.0));
    assert_eq!(
        aligned(AlignTo::DistributeHorizontally, &three),
        [p(1.0, 1.0), p(5.5, 5.0), p(10.0, 2.0)]
    );
}

#[test]
fn align_needs_two_and_refuses_a_locked_footprint() {
    let engine = BoardEngine::new(board(), 1);
    let one: BTreeSet<Uuid> = [id(1)].into();
    assert!(
        align_commands(engine.board(), &one, AlignTo::Left)
            .unwrap()
            .is_empty()
    );
    let locked: BTreeSet<Uuid> = [id(1), id(4)].into();
    assert!(align_commands(engine.board(), &locked, AlignTo::Left).is_err());
    let mut board = board();
    board.footprints[0].position = Point::new(1.4, 0.6);
    let commands = align_commands(&board, &one, AlignTo::Grid).unwrap();
    assert_eq!(
        commands,
        [BoardCommand::MoveFootprint {
            footprint: id(1),
            to: Point::new(1.0, 1.0)
        }]
    );
}
