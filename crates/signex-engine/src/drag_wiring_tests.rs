//! The drag wiring mode (#1669): how a wire that follows a dragged pin bends.
//!
//! Fixture: resistor `R1` at (50, 50), pin 1 at (45, 50), a vertical wire
//! (45, 30)-(45, 50) into pin 1. R1 moves by (10, 10), so pin 1 lands on
//! (55, 60). With a net label on (45, 50) the tip is anchored and a new path
//! is added; without it the tip is free and the wire end follows.

use crate::rubber_band_tests::{
    add_label, add_resistor, add_wire, has, net_shape, segments, symbol_item,
};
use crate::test_support::test_sheet;
use crate::{Command, Engine};
use signex_types::corner::DragWiringMode;
use signex_types::schematic::Point;

fn drag(is_anchored: bool, wiring: DragWiringMode) -> Engine {
    let mut sheet = test_sheet();
    let r1 = add_resistor(&mut sheet, "R1", Point::new(50.0, 50.0));
    add_wire(&mut sheet, (45.0, 30.0), (45.0, 50.0));
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
    assert!(engine.take_warnings().is_empty(), "{wiring:?}");
    assert_eq!(net_shape(engine.document()), before, "{wiring:?}: nets");
    engine
}

type Segment = ((f64, f64), (f64, f64));

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
