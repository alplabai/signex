//! #2216 — Pour Cutout and Slice Pour: one batch, the largest piece keeps the
//! pour, the rest are new pours with its settings, undo restores the board.

use signex_types::pcb::{PcbBoard, Point};
use signex_types::pcb_layer::CopperLayer;
use signex_types::zone_pour::POUR_FILL;
use uuid::Uuid;

use super::*;
use crate::pcb::BoardEngine;
use crate::pcb::tests::{ZONE_1, board, id};

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Point> {
    vec![
        Point::new(x0, y0),
        Point::new(x1, y0),
        Point::new(x1, y1),
        Point::new(x0, y1),
    ]
}

/// The test board with its zone a net-0 10 x 10 mm pour on the bottom
/// copper, priority 2, clearance 0.3.
fn with_pour() -> PcbBoard {
    let mut b = board();
    let zone = &mut b.zones[0];
    zone.fill_type = POUR_FILL.into();
    zone.outline = rect(0.0, 0.0, 10.0, 10.0);
    zone.priority = 2;
    zone.clearance = 0.3;
    b
}

fn ids() -> impl FnMut() -> Uuid {
    let mut next = 900_u128;
    move || {
        next += 1;
        Uuid::from_u128(next)
    }
}

#[test]
fn a_cutout_across_a_pour_leaves_two_pours_in_one_step() {
    let b = with_pour();
    let commands = cut_pours(&b, CopperLayer::Bottom, &rect(3.0, -1.0, 5.0, 11.0), ids());
    assert_eq!(commands.len(), 2, "a replace and an add");
    let mut engine = BoardEngine::new(b.clone(), 1);
    engine.execute_batch(commands).expect("applies");
    let zones = &engine.board().zones;
    assert_eq!(zones.len(), 2);
    assert_eq!(zones[0].uuid, id(ZONE_1), "the larger piece keeps the pour");
    assert_eq!(zones[1].uuid, Uuid::from_u128(901));
    for zone in zones {
        assert!(zone.is_pour());
        assert_eq!((zone.priority, zone.clearance), (2, 0.3), "settings kept");
        assert_eq!(zone.layer, "BottomCopper");
    }
    engine.undo().expect("undo").expect("a step");
    assert_eq!(engine.board().zones, b.zones, "one step undoes it all");
}

#[test]
fn a_cutout_on_another_layer_or_off_the_pour_changes_nothing() {
    let b = with_pour();
    let cut = rect(3.0, 3.0, 5.0, 5.0);
    assert!(cut_pours(&b, CopperLayer::Top, &cut, ids()).is_empty());
    let away = rect(30.0, 30.0, 35.0, 35.0);
    assert!(cut_pours(&b, CopperLayer::Bottom, &away, ids()).is_empty());
}

#[test]
fn a_cutout_over_the_whole_pour_removes_it() {
    let b = with_pour();
    let commands = cut_pours(
        &b,
        CopperLayer::Bottom,
        &rect(-1.0, -1.0, 11.0, 11.0),
        ids(),
    );
    let mut engine = BoardEngine::new(b, 1);
    engine.execute_batch(commands).expect("applies");
    assert!(engine.board().zones.is_empty());
}

#[test]
fn a_cutout_inside_a_pour_is_a_hole_in_it() {
    let b = with_pour();
    let commands = cut_pours(&b, CopperLayer::Bottom, &rect(4.0, 4.0, 6.0, 6.0), ids());
    let mut engine = BoardEngine::new(b, 1);
    engine.execute_batch(commands).expect("applies");
    let zones = &engine.board().zones;
    assert_eq!(zones.len(), 1);
    assert_eq!(zones[0].holes.len(), 1);
    assert_eq!(zones[0].usable_holes(), vec![0], "the fill can use it");
}

#[test]
fn a_slice_splits_the_pour_it_crosses() {
    let b = with_pour();
    let commands = slice_pours(
        &b,
        CopperLayer::Bottom,
        Point::new(6.0, -2.0),
        Point::new(6.0, 12.0),
        ids(),
    );
    let mut engine = BoardEngine::new(b, 1);
    engine.execute_batch(commands).expect("applies");
    let zones = &engine.board().zones;
    assert_eq!(zones.len(), 2);
    assert_eq!(zones[0].uuid, id(ZONE_1));
    let xs = |i: usize| {
        zones[i]
            .outline
            .iter()
            .map(|p| p.x)
            .fold(f64::NAN, f64::max)
    };
    assert!(
        (xs(0) - 6.0).abs() < 1e-6,
        "the larger piece is left of x = 6"
    );
    assert!((xs(1) - 10.0).abs() < 1e-6);
    assert!(
        slice_pours(
            engine.board(),
            CopperLayer::Bottom,
            Point::new(20.0, 0.0),
            Point::new(20.0, 5.0),
            ids()
        )
        .is_empty(),
        "a line off every pour slices nothing"
    );
}
