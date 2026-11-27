//! The drag wiring mode (#1669): how a wire that follows a dragged pin bends.
//!
//! Fixture: resistor `R1` at (50, 50), pin 1 at (45, 50), a vertical wire
//! (45, 30)-(45, 50) into pin 1. R1 moves by (10, 10), so pin 1 lands on
//! (55, 60). With a net label on (45, 50) the tip is anchored and a new path
//! is added; without it the tip is free and the wire end follows.

use crate::rubber_band_tests::{
    NetShape, add_label, add_resistor, add_wire, has, net_shape, segments, symbol_item,
};
use crate::test_support::test_sheet;
use crate::{Command, Engine};
use signex_types::corner::DragWiringMode;
use signex_types::schematic::Point;

type Segment = ((f64, f64), (f64, f64));

/// The fixture with foreign `blockers` wires, dragged; the engine and the
/// nets before the move.
fn drag_over(
    blockers: &[Segment],
    is_anchored: bool,
    wiring: DragWiringMode,
) -> (Engine, NetShape) {
    let mut sheet = test_sheet();
    let r1 = add_resistor(&mut sheet, "R1", Point::new(50.0, 50.0));
    add_wire(&mut sheet, (45.0, 30.0), (45.0, 50.0));
    for (a, b) in blockers {
        add_wire(&mut sheet, *a, *b);
    }
    if is_anchored {
        add_label(&mut sheet, "SIG", (45.0, 50.0));
    }
    let mut engine = Engine::new(sheet).expect("engine");
    let before = net_shape(engine.document());
    engine
        .execute(Command::MoveSelection {
            items: vec![symbol_item(r1)],
            dx: 10.0,
            dy: 10.0,
            keep_wires: true,
            wiring,
        })
        .expect("move");
    (engine, before)
}

fn drag(is_anchored: bool, wiring: DragWiringMode) -> Engine {
    let (mut engine, before) = drag_over(&[], is_anchored, wiring);
    assert!(engine.take_warnings().is_empty(), "{wiring:?}");
    assert_eq!(net_shape(engine.document()), before, "{wiring:?}: nets");
    engine
}

fn assert_wires(engine: &Engine, expected: &[Segment]) {
    let segs = segments(engine);
    assert_eq!(segs.len(), expected.len(), "{segs:?}");
    for (a, b) in expected {
        assert!(has(&segs, *a, *b), "missing {a:?}-{b:?} in {segs:?}");
    }
}

#[test]
fn a_free_tip_bends_as_the_mode_says() {
    // Auto keeps the vertical wire's column: the corner is (45, 60).
    assert_wires(
        &drag(false, DragWiringMode::Auto),
        &[((45.0, 30.0), (45.0, 60.0)), ((45.0, 60.0), (55.0, 60.0))],
    );
    assert_wires(
        &drag(false, DragWiringMode::HorizontalFirst),
        &[((45.0, 30.0), (55.0, 30.0)), ((55.0, 30.0), (55.0, 60.0))],
    );
    assert_wires(
        &drag(false, DragWiringMode::VerticalFirst),
        &[((45.0, 30.0), (45.0, 60.0)), ((45.0, 60.0), (55.0, 60.0))],
    );
    assert_wires(
        &drag(false, DragWiringMode::Straight),
        &[((45.0, 30.0), (55.0, 60.0))],
    );
}

#[test]
fn an_anchored_tip_gets_a_new_path_bent_as_the_mode_says() {
    // The wire and the label stay; the new path starts at the old tip.
    let kept = ((45.0, 30.0), (45.0, 50.0));
    assert_wires(
        &drag(true, DragWiringMode::HorizontalFirst),
        &[
            kept,
            ((45.0, 50.0), (55.0, 50.0)),
            ((55.0, 50.0), (55.0, 60.0)),
        ],
    );
    assert_wires(
        &drag(true, DragWiringMode::Straight),
        &[kept, ((45.0, 50.0), (55.0, 60.0))],
    );
    // Vertical first continues the wire's own column, as Auto does here.
    let auto = segments(&drag(true, DragWiringMode::Auto));
    let vertical = segments(&drag(true, DragWiringMode::VerticalFirst));
    assert_eq!(vertical, auto);
    assert!(has(&vertical, (45.0, 60.0), (55.0, 60.0)), "{vertical:?}");
}

#[test]
fn an_anchored_tip_turns_vertical_first_where_auto_turns_horizontal() {
    // A horizontal wire (30, 50)-(45, 50): Auto leaves the old tip along it.
    let run = |wiring| {
        let mut sheet = test_sheet();
        let r1 = add_resistor(&mut sheet, "R1", Point::new(50.0, 50.0));
        add_wire(&mut sheet, (30.0, 50.0), (45.0, 50.0));
        add_label(&mut sheet, "SIG", (45.0, 50.0));
        let mut engine = Engine::new(sheet).expect("engine");
        engine
            .execute(Command::MoveSelection {
                items: vec![symbol_item(r1)],
                dx: 10.0,
                dy: 10.0,
                keep_wires: true,
                wiring,
            })
            .expect("move");
        segments(&engine)
    };
    let auto = run(DragWiringMode::Auto);
    assert!(has(&auto, (55.0, 50.0), (55.0, 60.0)), "{auto:?}");
    let vertical = run(DragWiringMode::VerticalFirst);
    assert!(has(&vertical, (45.0, 50.0), (45.0, 60.0)), "{vertical:?}");
    assert!(has(&vertical, (45.0, 60.0), (55.0, 60.0)), "{vertical:?}");
}

/// The approved fallback: a foreign wire ending on the chosen L (a crossing
/// alone joins nothing) makes the free tip take the other L, and the pin
/// stays on its net.
#[test]
fn a_blocked_l_falls_back_to_the_other_l() {
    let horizontal = [((45.0, 30.0), (55.0, 30.0)), ((55.0, 30.0), (55.0, 60.0))];
    let vertical = [((45.0, 30.0), (45.0, 60.0)), ((45.0, 60.0), (55.0, 60.0))];
    // Each blocker ends on the first leg of the chosen L only.
    let cases = [
        (
            DragWiringMode::HorizontalFirst,
            ((50.0, 20.0), (50.0, 30.0)),
            vertical,
        ),
        (
            DragWiringMode::VerticalFirst,
            ((35.0, 55.0), (45.0, 55.0)),
            horizontal,
        ),
    ];
    for (wiring, blocker, other_l) in cases {
        let (mut engine, before) = drag_over(&[blocker], false, wiring);
        assert!(engine.take_warnings().is_empty(), "{wiring:?}");
        assert_eq!(net_shape(engine.document()), before, "{wiring:?}: nets");
        let mut expected = other_l.to_vec();
        expected.push(blocker);
        assert_wires(&engine, &expected);
    }
}

/// The fixture moved by (`dx`, `dy`) with `blockers`: the move must keep the
/// nets with no warning.
fn drag_by(
    blockers: &[Segment],
    is_anchored: bool,
    wiring: DragWiringMode,
    (dx, dy): (f64, f64),
) -> Engine {
    let mut sheet = test_sheet();
    let r1 = add_resistor(&mut sheet, "R1", Point::new(50.0, 50.0));
    add_wire(&mut sheet, (45.0, 30.0), (45.0, 50.0));
    for (a, b) in blockers {
        add_wire(&mut sheet, *a, *b);
    }
    if is_anchored {
        add_label(&mut sheet, "SIG", (45.0, 50.0));
    }
    let mut engine = Engine::new(sheet).expect("engine");
    let before = net_shape(engine.document());
    engine
        .execute(Command::MoveSelection {
            items: vec![symbol_item(r1)],
            dx,
            dy,
            keep_wires: true,
            wiring,
        })
        .expect("move");
    assert!(engine.take_warnings().is_empty(), "{wiring:?}");
    assert_eq!(net_shape(engine.document()), before, "{wiring:?}: nets");
    engine
}

/// #2516: a free tip's wire leaves its fixed end (45, 30) on a 45° leg and
/// turns onto the pin's column, or the other way round. Pin 1 lands on
/// (55, 54).
#[test]
fn a_free_tip_bends_at_45_degrees_as_the_mode_says() {
    assert_wires(
        &drag_by(&[], false, DragWiringMode::Angle45Start, (10.0, 4.0)),
        &[((45.0, 30.0), (55.0, 40.0)), ((55.0, 40.0), (55.0, 54.0))],
    );
    assert_wires(
        &drag_by(&[], false, DragWiringMode::Angle45End, (10.0, 4.0)),
        &[((45.0, 30.0), (45.0, 44.0)), ((45.0, 44.0), (55.0, 54.0))],
    );
}

/// #2516: an anchored tip's new path leaves the old tip (45, 50) the same
/// way; the wire and the label stay.
#[test]
fn an_anchored_tip_gets_a_45_degree_new_path() {
    let kept = ((45.0, 30.0), (45.0, 50.0));
    assert_wires(
        &drag_by(&[], true, DragWiringMode::Angle45Start, (10.0, 4.0)),
        &[
            kept,
            ((45.0, 50.0), (49.0, 54.0)),
            ((49.0, 54.0), (55.0, 54.0)),
        ],
    );
    assert_wires(
        &drag_by(&[], true, DragWiringMode::Angle45End, (10.0, 4.0)),
        &[
            kept,
            ((45.0, 50.0), (51.0, 50.0)),
            ((51.0, 50.0), (55.0, 54.0)),
        ],
    );
}

/// #2516: a foreign wire ending on the 45° leg of the chosen route makes the
/// tip take the 45° leg at the other end, for a free and an anchored tip.
#[test]
fn a_blocked_45_degree_route_falls_back_to_the_other_end() {
    // (50, 35) is on (45, 30)-(55, 40), the free tip's first leg.
    let blocker = ((50.0, 35.0), (60.0, 35.0));
    assert_wires(
        &drag_by(&[blocker], false, DragWiringMode::Angle45Start, (10.0, 4.0)),
        &[
            ((45.0, 30.0), (45.0, 44.0)),
            ((45.0, 44.0), (55.0, 54.0)),
            blocker,
        ],
    );
    // (47, 52) is on (45, 50)-(49, 54), the anchored tip's first leg.
    let blocker = ((47.0, 52.0), (47.0, 58.0));
    assert_wires(
        &drag_by(&[blocker], true, DragWiringMode::Angle45Start, (10.0, 4.0)),
        &[
            ((45.0, 30.0), (45.0, 50.0)),
            ((45.0, 50.0), (51.0, 50.0)),
            ((51.0, 50.0), (55.0, 54.0)),
            blocker,
        ],
    );
}

/// Straight has no other route: foreign wire ends on the leg and on the new
/// path an anchored tip would get leave the pin as a plain move, with the
/// guard's warning.
#[test]
fn a_blocked_straight_leg_is_a_plain_move_with_a_warning() {
    // (50, 45) is on (45, 30)-(55, 60); (50, 55) on (45, 50)-(55, 60).
    let blockers = [((40.0, 45.0), (50.0, 45.0)), ((60.0, 55.0), (50.0, 55.0))];
    let (mut engine, _) = drag_over(&blockers, false, DragWiringMode::Straight);
    let warnings = engine.take_warnings();
    assert!(
        warnings.iter().any(|w| w.pins_not_kept() == ["R1 pin 1"]),
        "{warnings:?}"
    );
    assert_wires(
        &engine,
        &[((45.0, 30.0), (45.0, 50.0)), blockers[0], blockers[1]],
    );
}
