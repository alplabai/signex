//! #2216 — Slice Pour: one batch, the largest piece keeps the pour, the rest
//! are new pours with its settings, undo restores the board. #2871 — Pour
//! Cutout places a cutout region and leaves the pour as it was.

use signex_types::pcb::{PcbBoard, Point};
use signex_types::pcb_layer::CopperLayer;
use signex_types::zone_pour::POUR_FILL;
use uuid::Uuid;

use super::*;
use crate::pcb::tests::{ZONE_1, board, id};
use crate::pcb::{BoardEngine, draw};

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

/// #2871 (DECISIONS 2026-11-24, replacing the #2216 cut): a cutout across,
/// inside or over the whole pour is placed as a region of its own in one
/// step, and the pour is left as it was. Before, the cut rewrote the pour
/// into pieces, a hole, or nothing.
#[test]
fn a_cutout_is_a_region_of_its_own_and_the_pour_is_left_as_it_was() {
    let b = with_pour();
    for (n, cut) in [
        (1, rect(3.0, -1.0, 5.0, 11.0)),
        (2, rect(4.0, 4.0, 6.0, 6.0)),
        (3, rect(-1.0, -1.0, 11.0, 11.0)),
    ] {
        let region =
            draw::new_pour_cutout(Uuid::from_u128(0xC00 + n), cut.clone(), CopperLayer::Bottom)
                .expect("an area");
        let mut engine = BoardEngine::new(b.clone(), 1);
        engine.execute(draw::place_zone(region)).expect("applies");
        let zones = &engine.board().zones;
        assert_eq!(zones.len(), 2, "the pour and the cutout");
        assert_eq!(zones[0], b.zones[0], "the pour's outline is not edited");
        let cutout = &zones[1];
        assert!(cutout.is_pour_cutout() && !cutout.is_pour());
        assert_eq!((cutout.net, cutout.layer.as_str()), (0, "BottomCopper"));
        assert_eq!(cutout.outline, cut);
        engine.undo().expect("undo").expect("a step");
        assert_eq!(engine.board().zones, b.zones, "one step undoes it");
    }
}

/// #2871: a cutout with no area is not placed.
#[test]
fn a_cutout_with_no_area_is_not_placed() {
    let flat = vec![
        Point::new(0.0, 0.0),
        Point::new(4.0, 0.0),
        Point::new(8.0, 0.0),
    ];
    assert!(draw::new_pour_cutout(Uuid::from_u128(1), flat, CopperLayer::Top).is_none());
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

/// #2523: Shelve all and Unshelve all set every pour in one step, leave
/// other zones as they are, and change nothing when already done.
#[test]
fn shelve_all_and_unshelve_all_are_one_step_each() {
    let mut b = with_pour();
    let mut second = b.zones[0].clone();
    second.uuid = Uuid::from_u128(77);
    b.zones.push(second);
    let mut solid = b.zones[0].clone();
    solid.uuid = Uuid::from_u128(78);
    solid.fill_type = "solid".into();
    b.zones.push(solid);

    let mut engine = BoardEngine::new(b.clone(), 1);
    let shelve = shelve_all_pours(engine.board(), true).expect("two pours to shelve");
    engine.execute(shelve).expect("applies");
    let shelved: Vec<bool> = engine.board().zones.iter().map(|z| z.shelved).collect();
    assert_eq!(shelved, vec![true, true, false], "only the pours");
    assert!(
        shelve_all_pours(engine.board(), true).is_none(),
        "already shelved"
    );

    let unshelve = shelve_all_pours(engine.board(), false).expect("two to unshelve");
    engine.execute(unshelve).expect("applies");
    assert_eq!(engine.board().zones, b.zones);
    engine.undo().expect("undo").expect("a step");
    assert!(engine.board().zones[..2].iter().all(|z| z.shelved));
    engine.undo().expect("undo").expect("a step");
    assert_eq!(engine.board().zones, b.zones, "each was a single step");
}
