//! #2540: dragging a part whose pin sits on a junction of two or more wires
//! leaves those wires where they are and grows a stub to the pin.
//!
//! Fixture: the bundled sample sheet. C1 pin 1 is at (63.50, 99.06), where
//! the +5V wires (57.15, 99.06)-(63.50, 99.06) and (63.50, 99.06)-(66.04,
//! 99.06) end on a dot. Pin 2 is at (63.50, 106.68), with the GND wire down
//! to the dot at (63.50, 119.38). One step is 2.54 mm; down is +y.

use signex_net::{pt_key, uf::Key};
use signex_types::corner::DragWiringMode;
use signex_types::format::SnxSchematic;
use signex_types::schematic::{Point, SchematicSheet, SelectedItem};

use crate::rubber_band_tests::{
    add_dot, add_resistor, add_wire, has, net_shape, segments, symbol_item,
};
use crate::test_support::test_sheet;
use crate::{Command, Engine, MovePreview};

const SAMPLE: &str = include_str!("../../signex-app/assets/sample-project/linear-regulator.snxsch");
const STEP: f64 = 2.54;
const PIN_1: (f64, f64) = (63.5, 99.06);
const RAIL: [((f64, f64), (f64, f64)); 2] = [((57.15, 99.06), PIN_1), (PIN_1, (66.04, 99.06))];

type Span = (Key, Key);

fn span(a: (f64, f64), b: (f64, f64)) -> Span {
    let (ka, kb) = (pt_key(&Point::new(a.0, a.1)), pt_key(&Point::new(b.0, b.1)));
    if ka <= kb { (ka, kb) } else { (kb, ka) }
}

/// The wires as direction-free spans in the netlist's 1 um key space.
fn spans(sheet: &SchematicSheet) -> Vec<Span> {
    let mut spans: Vec<Span> = (sheet.wires.iter())
        .map(|w| span((w.start.x, w.start.y), (w.end.x, w.end.y)))
        .collect();
    spans.sort_unstable();
    spans
}

fn sample() -> (SchematicSheet, SelectedItem) {
    let sheet = SnxSchematic::parse(SAMPLE)
        .expect("the sample parses")
        .sheet;
    let c1 = (sheet.symbols.iter())
        .find(|s| s.reference == "C1")
        .expect("C1")
        .uuid;
    (sheet, symbol_item(c1))
}

/// C1 dragged `steps` down (negative: up) in Auto with keep wires on: the
/// rail stays, a vertical stub reaches pin 1, the nets are the sheet's own
/// and nothing falls back to a plain move.
fn assert_rail_kept_with_a_stub(steps: i32) {
    let dy = f64::from(steps) * STEP;
    let (sheet, c1) = sample();
    let mut engine = Engine::new(sheet.clone()).expect("engine");
    engine
        .execute(Command::MoveSelection {
            items: vec![c1],
            dx: 0.0,
            dy,
            keep_wires: true,
            wiring: DragWiringMode::Auto,
        })
        .expect("move");
    let warnings = engine.take_warnings();
    assert!(warnings.is_empty(), "{steps} steps: {warnings:?}");
    let after = engine.document();
    assert_eq!(net_shape(after), net_shape(&sheet), "{steps} steps: nets");
    let wires = spans(after);
    for (a, b) in RAIL {
        assert!(
            wires.contains(&span(a, b)),
            "{steps} steps: the +5V rail moved: {wires:?}"
        );
    }
    let stub = span(PIN_1, (PIN_1.0, PIN_1.1 + dy));
    assert!(
        wires.contains(&stub),
        "{steps} steps: no stub to pin 1: {wires:?}"
    );
    let dot = pt_key(&Point::new(PIN_1.0, PIN_1.1));
    assert!(
        after.junctions.iter().any(|j| pt_key(&j.position) == dot),
        "{steps} steps: the rail's dot moved"
    );
}

#[test]
fn c1_dragged_up_keeps_the_rail_and_grows_a_stub() {
    for steps in 1..=2 {
        assert_rail_kept_with_a_stub(-steps);
    }
}

/// From three steps the stub passes pin 2's old tip, where the GND wire still
/// ends until pin 2 is rerouted: the stub waits for it (no plain move).
#[test]
fn c1_dragged_down_keeps_the_rail_and_grows_a_stub_past_its_own_gnd_wire() {
    for steps in 1..=7 {
        assert_rail_kept_with_a_stub(steps);
    }
}

/// The ghost of the drag is the commit.
#[test]
fn the_ghost_of_a_junction_drag_is_its_commit() {
    for steps in [2, 4] {
        let dy = f64::from(steps) * STEP;
        let (sheet, c1) = sample();
        let ghost = MovePreview::new(&sheet)
            .moved_with_wiring(&[(c1.clone(), 0.0, dy)], true, DragWiringMode::Auto)
            .0;
        let mut engine = Engine::new(sheet).expect("engine");
        engine
            .execute(Command::MoveSelection {
                items: vec![c1],
                dx: 0.0,
                dy,
                keep_wires: true,
                wiring: DragWiringMode::Auto,
            })
            .expect("move");
        assert_eq!(spans(&ghost), spans(engine.document()), "{steps} steps");
    }
}

/// A pin on a four-way dot moved off both axes has no stub that misses the
/// four wires, so they follow it as before (the fallback).
#[test]
fn a_junction_pin_with_no_stub_that_fits_lets_its_wires_follow() {
    let mut sheet = test_sheet();
    let tp = add_resistor(&mut sheet, "TP", Point::new(55.0, 50.0)); // pin 1 at (50, 50)
    for (a, b) in [
        ((30.0, 50.0), (50.0, 50.0)),
        ((50.0, 50.0), (70.0, 50.0)),
        ((50.0, 30.0), (50.0, 50.0)),
        ((50.0, 50.0), (50.0, 70.0)),
    ] {
        add_wire(&mut sheet, a, b);
    }
    add_dot(&mut sheet, (50.0, 50.0), true);
    let before = net_shape(&sheet);
    let mut engine = Engine::new(sheet).expect("engine");
    engine
        .execute(Command::MoveSelection {
            items: vec![symbol_item(tp)],
            dx: 5.0,
            dy: 5.0,
            keep_wires: true,
            wiring: DragWiringMode::Auto,
        })
        .expect("move");
    assert_eq!(net_shape(engine.document()), before);
    let segs = segments(&engine);
    assert!(
        !has(&segs, (30.0, 50.0), (50.0, 50.0)),
        "the wires followed: {segs:?}"
    );
}
