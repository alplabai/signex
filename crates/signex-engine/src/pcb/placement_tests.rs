use std::collections::BTreeSet;

use signex_types::pcb::{PcbBoard, PcbSetup, Point};
use uuid::Uuid;

use super::*;
use crate::pcb::BoardEngine;

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

fn board() -> PcbBoard {
    let footprint = |n: u128, x: f64, y: f64, rotation: f64, locked: bool| {
        serde_json::json!({
            "uuid": id(n), "reference": format!("U{n}"), "position": {"x": x, "y": y},
            "rotation": rotation, "layer": "TopCopper", "locked": locked,
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
            footprint(1, 10.0, 10.0, 30.0, false),
            footprint(2, 20.0, 10.0, 0.0, false),
            footprint(3, 30.0, 30.0, 0.0, true)
        ]
    }))
    .unwrap()
}

fn ids(list: &[u128]) -> BTreeSet<Uuid> {
    list.iter().map(|&n| id(n)).collect()
}

fn fp(board: &PcbBoard, n: u128) -> &signex_types::pcb::Footprint {
    board.footprints.iter().find(|f| f.uuid == id(n)).unwrap()
}

fn run(board: PcbBoard, commands: Vec<BoardCommand>) -> PcbBoard {
    let mut engine = BoardEngine::new(board, 1);
    engine.execute_batch(commands).expect("the batch runs");
    engine.board().clone()
}

#[test]
fn the_grid_is_the_setup_step_or_the_default() {
    let mut b = board();
    assert_eq!(grid_step_mm(&b), PCB_GRID_MM);
    b.setup = Some(PcbSetup {
        grid_size: 0.5,
        trace_width: 0.0,
        via_diameter: 0.0,
        via_drill: 0.0,
        clearance: 0.0,
        track_min_width: 0.0,
        via_min_diameter: 0.0,
        via_min_drill: 0.0,
    });
    assert_eq!(grid_step_mm(&b), 0.5);
    b.setup.as_mut().unwrap().grid_size = f64::NAN;
    assert_eq!(grid_step_mm(&b), PCB_GRID_MM);
}

#[test]
fn a_move_snaps_the_anchor_to_the_grid() {
    let delta = snap_delta(Point::new(10.0, 10.0), Point::new(0.7, -0.2), 0.5);
    assert_eq!(delta, Point::new(0.5, 0.0));
    let raw = Point::new(0.3, 0.3);
    assert_eq!(snap_delta(Point::new(0.0, 0.0), raw, 0.0), raw);
}

#[test]
fn a_lone_footprint_turns_in_place() {
    let b = board();
    let commands = transform_commands(
        &b,
        &ids(&[1]),
        PlacementTransform::Rotate { quarter_turns: -1 },
    )
    .unwrap();
    assert_eq!(
        commands,
        vec![BoardCommand::Rotate {
            footprint: id(1),
            degrees: 300.0
        }]
    );
}

#[test]
fn a_group_turns_about_its_grid_centre() {
    let b = board();
    let commands = transform_commands(
        &b,
        &ids(&[1, 2]),
        PlacementTransform::Rotate { quarter_turns: 1 },
    )
    .unwrap();
    let after = run(b, commands);
    // Centre (15, 10) on the 1.27 grid is (15.24, 10.16).
    let (u1, u2) = (fp(&after, 1), fp(&after, 2));
    assert_eq!(u1.rotation, 120.0);
    assert_eq!(u2.rotation, 90.0);
    assert!((u1.position.x - 15.4).abs() < 1e-9, "{:?}", u1.position);
    assert!((u2.position.x - 15.4).abs() < 1e-9, "{:?}", u2.position);
    assert!((u2.position.y - u1.position.y - 10.0).abs() < 1e-9);
}

#[test]
fn a_flip_left_right_is_one_flip_and_top_bottom_adds_a_half_turn() {
    let b = board();
    let lr = transform_commands(&b, &ids(&[1]), PlacementTransform::FlipLeftRight).unwrap();
    assert_eq!(lr, vec![BoardCommand::Flip { footprint: id(1) }]);
    let tb = transform_commands(&b, &ids(&[1]), PlacementTransform::FlipTopBottom).unwrap();
    let after = run(b, tb);
    assert_eq!(fp(&after, 1).rotation, 150.0);
    assert_eq!(fp(&after, 1).layer, "BottomCopper");
}

#[test]
fn a_locked_footprint_refuses_every_edit() {
    let b = board();
    let locked = ids(&[2, 3]);
    let refusal = PlacementRefusal::Locked(vec!["U3".into()]);
    assert_eq!(
        transform_commands(&b, &locked, PlacementTransform::FlipLeftRight),
        Err(refusal.clone())
    );
    assert_eq!(
        move_commands(&b, &locked, Point::new(1.0, 0.0)),
        Err(refusal.clone())
    );
    assert_eq!(
        delete_commands(&b, vec![BoardItemId::Footprint(id(3))]),
        Err(refusal)
    );
    assert_eq!(refusal_text(), "U3 is locked");
}

fn refusal_text() -> String {
    PlacementRefusal::Locked(vec!["U3".into()]).to_string()
}

#[test]
fn nothing_selected_and_a_zero_move_do_nothing() {
    let b = board();
    assert_eq!(
        move_commands(&b, &BTreeSet::new(), Point::new(1.0, 0.0)),
        Err(PlacementRefusal::NothingSelected)
    );
    assert_eq!(
        move_commands(&b, &ids(&[1]), Point::new(0.0, 0.0)),
        Ok(Vec::new())
    );
    assert_eq!(
        delete_commands(&b, Vec::new()),
        Err(PlacementRefusal::NothingSelected)
    );
}
