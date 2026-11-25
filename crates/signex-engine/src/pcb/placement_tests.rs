use std::collections::BTreeSet;

use signex_types::object_id::UuidV7Ids;
use signex_types::pcb::{PcbBoard, PcbSetup, Point};
use uuid::Uuid;

use super::*;
use crate::pcb::BoardEngine;

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

/// The corner a stretched track bends at with the router's default rule
/// (#2513).
fn corner() -> CornerRule {
    pcb_geom::stretch_rule(CornerRule::default())
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

/// With snapping off a group turns about the exact centre of its origins.
#[test]
fn a_group_with_snap_off_turns_about_its_exact_centre() {
    let mut b = board();
    b.footprints[0].position = Point::new(1.03, 2.0);
    b.footprints[1].position = Point::new(3.03, 2.0);
    let first = b.footprints[0].uuid;
    let group: BTreeSet<Uuid> = [first, b.footprints[1].uuid].into_iter().collect();
    let half_turn = PlacementTransform::Rotate { quarter_turns: 2 };
    let commands = transform_commands_with(&b, &group, half_turn, false).unwrap();
    let moved = commands.iter().find_map(|command| match command {
        BoardCommand::MoveFootprint { footprint, to } if *footprint == first => Some(*to),
        _ => None,
    });
    let to = moved.expect("the first part moves");
    assert!(
        (to.x - 3.03).abs() < 1e-9 && (to.y - 2.0).abs() < 1e-9,
        "{to:?}"
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
        move_commands(&b, &locked, Point::new(1.0, 0.0), corner(), &mut UuidV7Ids),
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
        move_commands(
            &b,
            &BTreeSet::new(),
            Point::new(1.0, 0.0),
            corner(),
            &mut UuidV7Ids
        ),
        Err(PlacementRefusal::NothingSelected)
    );
    assert_eq!(
        move_commands(
            &b,
            &ids(&[1]),
            Point::new(0.0, 0.0),
            corner(),
            &mut UuidV7Ids
        ),
        Ok(Vec::new())
    );
    assert_eq!(
        delete_commands(&b, Vec::new()),
        Err(PlacementRefusal::NothingSelected)
    );
}

/// #2379: a track end on a moved pad moves with it, in the same undo step;
/// its other end stays where it was.
#[test]
fn a_move_takes_the_track_ends_on_its_pads_along_in_one_step() {
    let mut b = board();
    // U2's pad sits at (21, 10); the track runs from (25, 10) onto it.
    b.segments = vec![
        serde_json::from_value(serde_json::json!({
            "uuid": id(900), "start": {"x": 25.0, "y": 10.0}, "end": {"x": 21.0, "y": 10.0},
            "width": 0.25, "layer": "TopCopper", "net": 0
        }))
        .unwrap(),
    ];
    let before = b.clone();
    let commands = move_commands(
        &b,
        &ids(&[2]),
        Point::new(2.0, 1.0),
        corner(),
        &mut UuidV7Ids,
    )
    .unwrap();
    // The move, the track and its new 45° leg (#2513, Caner 2026-11-25).
    assert_eq!(commands.len(), 3, "{commands:?}");
    let mut engine = BoardEngine::new(b, 1);
    engine.execute_batch(commands).expect("the batch runs");
    let track = &engine.board().segments[0];
    assert_eq!(track.start, Point::new(25.0, 10.0));
    // (25, 10) to the pad at (23, 11): straight to (24, 10), then 45°.
    assert_eq!(track.end, Point::new(24.0, 10.0));
    let leg = &engine.board().segments[1];
    assert_eq!(
        (leg.start, leg.end),
        (Point::new(24.0, 10.0), Point::new(23.0, 11.0))
    );
    assert_eq!(leg.net, track.net);
    assert_ne!(leg.uuid, track.uuid, "the leg is a new track");
    assert_eq!(fp(engine.board(), 2).position, Point::new(22.0, 11.0));
    engine.undo().expect("undo runs");
    assert_eq!(
        engine.board().segments,
        before.segments,
        "one undo puts back both"
    );
    assert_eq!(fp(engine.board(), 2).position, fp(&before, 2).position);
}

/// A Ctrl drag (Caner, 2026-11-21) moves the footprint alone: the track on
/// its pad keeps both ends.
#[test]
fn a_move_of_the_footprints_only_leaves_the_tracks_in_place() {
    let mut b = board();
    b.segments = vec![
        serde_json::from_value(serde_json::json!({
            "uuid": id(900), "start": {"x": 25.0, "y": 10.0}, "end": {"x": 21.0, "y": 10.0},
            "width": 0.25, "layer": "TopCopper", "net": 0
        }))
        .unwrap(),
    ];
    let commands = move_footprints_only(&b, &ids(&[2]), Point::new(2.0, 1.0)).unwrap();
    assert_eq!(commands.len(), 1, "{commands:?}");
    let moved = run(b.clone(), commands);
    assert_eq!(moved.segments, b.segments);
    assert_eq!(fp(&moved, 2).position, Point::new(22.0, 11.0));
}

/// A via at (20, 0) with an arc from it over (22, 2) to (24, 0).
fn board_with_via_arc() -> PcbBoard {
    let mut b = board();
    b.vias = vec![
        serde_json::from_value(serde_json::json!({
            "uuid": id(50), "position": {"x": 20.0, "y": 0.0}, "diameter": 0.8, "drill": 0.4,
            "layers": ["TopCopper", "BottomCopper"], "net": 1
        }))
        .unwrap(),
    ];
    b.free.arcs = vec![signex_types::pcb_arc::TrackArc {
        uuid: id(51),
        start: Point::new(20.0, 0.0),
        mid: Point::new(22.0, 2.0),
        end: Point::new(24.0, 0.0),
        width: 0.25,
        layer: "TopCopper".into(),
        net: 1,
    }];
    b
}

#[test]
fn a_via_move_refuses_an_arc_that_would_collapse_or_grow_too_much() {
    let b = board_with_via_arc();
    let via = ids(&[50]);
    for dx in [4.0, 4.0 - 0.8e-6] {
        assert_eq!(
            move_vias(&b, &via, Point::new(dx, 0.0), corner(), &mut UuidV7Ids),
            Err(PlacementRefusal::ArcCollapses(id(51))),
            "{dx}"
        );
    }
    // 3.1 times the 4 mm chord.
    assert_eq!(
        move_vias(&b, &via, Point::new(-8.4, 0.0), corner(), &mut UuidV7Ids),
        Err(PlacementRefusal::ArcGrows(id(51), 3.0))
    );
    // 2.9 times: allowed, and the far end stays.
    let commands =
        move_vias(&b, &via, Point::new(-7.6, 0.0), corner(), &mut UuidV7Ids).expect("allowed");
    let moved = run(b, commands);
    assert_eq!(moved.free.arcs[0].end, Point::new(24.0, 0.0));
    assert!((moved.free.arcs[0].start.x - 12.4).abs() < 1e-9);
}

/// #2410: a command names a component by its designator; a missing or
/// shared one is refused with the reason.
#[test]
fn a_component_is_found_by_its_designator_once() {
    let mut b = board();
    assert_eq!(by_designator(&b, " U2 "), Ok(id(2)));
    assert!(
        by_designator(&b, "U9")
            .unwrap_err()
            .contains("no component")
    );
    b.footprints[2].reference = "U2".into();
    assert!(by_designator(&b, "U2").unwrap_err().contains("several"));
}

/// #2326: a uuid names the board item of its kind; a pad's uuid is no board
/// item (its footprint is), nor is an unknown one.
#[test]
fn a_uuid_names_its_board_item_whatever_the_kind() {
    let b = board_with_via_arc();
    assert_eq!(
        BoardItemId::on_board(&b, id(2)),
        Some(BoardItemId::Footprint(id(2)))
    );
    assert_eq!(
        BoardItemId::on_board(&b, id(50)),
        Some(BoardItemId::Via(id(50)))
    );
    assert_eq!(
        BoardItemId::on_board(&b, id(51)),
        Some(BoardItemId::Arc(id(51)))
    );
    assert_eq!(BoardItemId::on_board(&b, id(2 * 16)), None);
    assert_eq!(BoardItemId::on_board(&b, id(99)), None);
}
