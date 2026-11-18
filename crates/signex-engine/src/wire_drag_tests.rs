//! A dragged wire keeps its connections (`transform::rubber_band::wire_contacts`):
//! the wires on its ends stretch, shrink or fold, pins it joined get a new
//! stub, and a grip drag of one end brings the wires attached there.

use crate::rubber_band_tests::{
    add_dot, add_wire, has, lib_pin, lib_symbol, mv, net_shape, place, segments,
};
use crate::test_support::test_sheet;
use crate::{Command, Engine, MovePreview};
use signex_types::schematic::{Point, SchematicSheet, SelectedItem, SelectedKind};
use uuid::Uuid;

/// A one-pin part whose pin tip is at `at`.
fn add_pin(sheet: &mut SchematicSheet, reference: &str, at: (f64, f64)) -> Uuid {
    sheet
        .lib_symbols
        .entry("TP".to_string())
        .or_insert_with(|| lib_symbol("TP", vec![lib_pin("1", 0.0)]));
    place(sheet, reference, "TP", Point::new(at.0, at.1))
}

fn wire_item(uuid: Uuid) -> SelectedItem {
    SelectedItem::new(uuid, SelectedKind::Wire)
}

/// Whether a wire of `segs` ends at `p`.
fn ends_at(segs: &[((f64, f64), (f64, f64))], p: (f64, f64)) -> bool {
    segs.iter().any(|s| s.0 == p || s.1 == p)
}

/// Every wire as `(uuid, start, end)`, for an exact undo comparison.
fn wires(sheet: &SchematicSheet) -> Vec<(Uuid, Point, Point)> {
    sheet
        .wires
        .iter()
        .map(|w| (w.uuid, w.start, w.end))
        .collect()
}

/// The maintainer's sheet: U1's pin at (50,50), a stub up to (50,40), the
/// segment across to (80,40), and a vertical down to U2's pin at (80,70).
fn stub_segment_vertical() -> (SchematicSheet, Uuid) {
    let mut sheet = test_sheet();
    add_pin(&mut sheet, "U1", (50.0, 50.0));
    add_pin(&mut sheet, "U2", (80.0, 70.0));
    add_wire(&mut sheet, (50.0, 50.0), (50.0, 40.0));
    let segment = add_wire(&mut sheet, (50.0, 40.0), (80.0, 40.0));
    add_wire(&mut sheet, (80.0, 40.0), (80.0, 70.0));
    (sheet, segment)
}

#[test]
fn a_dragged_segment_stretches_its_neighbours_and_one_undo_restores_them() {
    let mut sheet = test_sheet();
    add_pin(&mut sheet, "U1", (50.0, 60.0));
    add_pin(&mut sheet, "U2", (80.0, 70.0));
    add_wire(&mut sheet, (50.0, 60.0), (50.0, 40.0));
    let segment = add_wire(&mut sheet, (50.0, 40.0), (80.0, 40.0));
    add_wire(&mut sheet, (80.0, 40.0), (80.0, 70.0));
    let before = net_shape(&sheet);
    let before_wires = wires(&sheet);
    let mut engine = Engine::new(sheet).unwrap();
    mv(&mut engine, vec![wire_item(segment)], 0.0, 10.0, true);
    let segs = segments(&engine);
    assert_eq!(segs.len(), 3, "{segs:?}");
    assert!(has(&segs, (50.0, 50.0), (80.0, 50.0)), "{segs:?}");
    assert!(has(&segs, (50.0, 60.0), (50.0, 50.0)), "{segs:?}");
    assert!(has(&segs, (80.0, 50.0), (80.0, 70.0)), "{segs:?}");
    assert_eq!(net_shape(engine.document()), before, "netlist changed");
    assert!(engine.take_warnings().is_empty());

    engine.undo().unwrap();
    assert_eq!(wires(engine.document()), before_wires);
}

#[test]
fn a_segment_dragged_onto_the_pin_row_folds_the_stub_away() {
    let (sheet, segment) = stub_segment_vertical();
    let before = net_shape(&sheet);
    let before_wires = wires(&sheet);
    let mut engine = Engine::new(sheet).unwrap();
    mv(&mut engine, vec![wire_item(segment)], 0.0, 10.0, true);
    let segs = segments(&engine);
    assert_eq!(segs.len(), 2, "the zero-length stub is gone: {segs:?}");
    assert!(has(&segs, (50.0, 50.0), (80.0, 50.0)), "{segs:?}");
    assert!(has(&segs, (80.0, 50.0), (80.0, 70.0)), "{segs:?}");
    assert_eq!(net_shape(engine.document()), before, "U1 and U2 joined");

    engine.undo().unwrap();
    assert_eq!(wires(engine.document()), before_wires);
}

#[test]
fn a_sideways_drag_bends_the_neighbours_instead_of_tilting_them() {
    let (sheet, segment) = stub_segment_vertical();
    let before = net_shape(&sheet);
    let mut engine = Engine::new(sheet).unwrap();
    mv(&mut engine, vec![wire_item(segment)], 5.0, -10.0, true);
    let segs = segments(&engine);
    // The left bend's short run may fold into the segment.
    let covers = segs
        .iter()
        .any(|s| s.0.1 == 30.0 && s.1.1 == 30.0 && s.0.0 <= 55.0 && s.1.0 >= 85.0);
    assert!(covers, "{segs:?}");
    assert!(
        segs.iter().all(|s| s.0.0 == s.1.0 || s.0.1 == s.1.1),
        "diagonal wire: {segs:?}"
    );
    assert_eq!(net_shape(engine.document()), before, "netlist changed");
}

#[test]
fn keep_wires_off_still_moves_only_the_segment() {
    let (sheet, segment) = stub_segment_vertical();
    let mut engine = Engine::new(sheet).unwrap();
    mv(&mut engine, vec![wire_item(segment)], 0.0, 10.0, false);
    let segs = segments(&engine);
    assert!(has(&segs, (50.0, 50.0), (50.0, 40.0)), "{segs:?}");
    assert!(has(&segs, (80.0, 40.0), (80.0, 70.0)), "{segs:?}");
}

/// A segment along the bottom pins of four parts: FB1, F1 and L touch it
/// mid-span (each with a dot), D1 sits on its right end, and a vertical runs
/// down from its left end to J1. Another part's pin touches it bare (no dot),
/// so the netlist never had it on the net.
#[test]
fn a_segment_dragged_off_its_pins_leaves_a_stub_to_each_of_them() {
    let mut sheet = test_sheet();
    let mut parts = Vec::new();
    for (name, x) in [("FB1", 35.0), ("F1", 50.0), ("L", 65.0), ("D1", 80.0)] {
        parts.push((add_pin(&mut sheet, name, (x, 40.0)), x));
    }
    let bare = add_pin(&mut sheet, "NC1", (72.5, 40.0));
    add_pin(&mut sheet, "J1", (20.0, 70.0));
    let segment = add_wire(&mut sheet, (20.0, 40.0), (80.0, 40.0));
    add_wire(&mut sheet, (20.0, 40.0), (20.0, 70.0));
    for x in [35.0, 50.0, 65.0] {
        add_dot(&mut sheet, (x, 40.0), false);
    }
    let before = net_shape(&sheet);
    let before_wires = wires(&sheet);
    let mut engine = Engine::new(sheet).unwrap();
    mv(&mut engine, vec![wire_item(segment)], 0.0, 10.0, true);
    let segs = segments(&engine);
    assert!(has(&segs, (20.0, 50.0), (80.0, 50.0)), "{segs:?}");
    assert!(has(&segs, (20.0, 50.0), (20.0, 70.0)), "{segs:?}");
    for (_, x) in &parts {
        assert!(has(&segs, (*x, 40.0), (*x, 50.0)), "{x}: {segs:?}");
    }
    assert!(
        !ends_at(&segs, (72.5, 40.0)),
        "a bare touch was never a connection: {segs:?}"
    );
    let after = net_shape(engine.document());
    assert_eq!(after, before, "every pin stays on its net");
    let joined = after.iter().find(|(_, t)| t.len() == 5).expect("one net");
    for (part, _) in &parts {
        assert!(joined.1.iter().any(|(id, _)| id == part), "{after:?}");
    }
    assert!(!joined.1.iter().any(|(id, _)| *id == bare));

    engine.undo().unwrap();
    assert_eq!(wires(engine.document()), before_wires);
}

#[test]
fn the_drag_preview_is_the_commit() {
    let (sheet, segment) = stub_segment_vertical();
    let moves = vec![(wire_item(segment), 0.0, 10.0)];
    let ghost = MovePreview::new(&sheet).moved(&moves, true);
    let mut engine = Engine::new(sheet).unwrap();
    engine
        .execute(Command::MoveEach {
            moves,
            keep_wires: true,
        })
        .unwrap();
    let shape = |s: &SchematicSheet| {
        let mut w: Vec<_> = s
            .wires
            .iter()
            .map(|w| (w.start.x, w.start.y, w.end.x, w.end.y))
            .collect();
        w.sort_by(|a, b| a.partial_cmp(b).unwrap());
        w
    };
    assert_eq!(shape(&ghost), shape(engine.document()));
}

fn grip(engine: &mut Engine, wire: Uuid, start: bool, to: (f64, f64)) {
    engine
        .execute(Command::MoveWireEnd {
            wire,
            start,
            to: Point::new(to.0, to.1),
            keep_wires: true,
        })
        .unwrap();
}

/// A segment (20,40)-(80,40) with a leg down from its right end to U2's pin
/// at (80,70), and U1's pin on its left end.
fn segment_with_leg() -> (SchematicSheet, Uuid) {
    let mut sheet = test_sheet();
    add_pin(&mut sheet, "U1", (20.0, 40.0));
    add_pin(&mut sheet, "U2", (80.0, 70.0));
    let segment = add_wire(&mut sheet, (20.0, 40.0), (80.0, 40.0));
    add_wire(&mut sheet, (80.0, 40.0), (80.0, 70.0));
    (sheet, segment)
}

#[test]
fn an_end_grip_shortens_the_segment_and_the_leg_follows() {
    let (sheet, segment) = segment_with_leg();
    let before = net_shape(&sheet);
    let before_wires = wires(&sheet);
    let mut engine = Engine::new(sheet).unwrap();
    // Off the line on purpose: a straight wire keeps its line.
    grip(&mut engine, segment, false, (60.0, 43.0));
    let segs = segments(&engine);
    assert!(has(&segs, (20.0, 40.0), (60.0, 40.0)), "{segs:?}");
    assert!(has(&segs, (60.0, 40.0), (60.0, 70.0)), "{segs:?}");
    assert!(has(&segs, (60.0, 70.0), (80.0, 70.0)), "{segs:?}");
    assert_eq!(net_shape(engine.document()), before, "netlist changed");

    engine.undo().unwrap();
    assert_eq!(wires(engine.document()), before_wires, "one undo step");
}

#[test]
fn an_end_grip_lengthens_the_segment_and_a_collinear_neighbour_follows() {
    let mut sheet = test_sheet();
    let segment = add_wire(&mut sheet, (20.0, 40.0), (50.0, 40.0));
    add_wire(&mut sheet, (50.0, 40.0), (50.0, 60.0));
    add_wire(&mut sheet, (20.0, 40.0), (0.0, 40.0));
    let mut engine = Engine::new(sheet).unwrap();
    grip(&mut engine, segment, true, (10.0, 40.0));
    let segs = segments(&engine);
    // The collinear neighbour on the dragged end shrinks to meet it; the
    // fold may then join the two into one run.
    assert!(
        has(&segs, (0.0, 40.0), (10.0, 40.0)) || has(&segs, (0.0, 40.0), (50.0, 40.0)),
        "{segs:?}"
    );
    assert!(has(&segs, (50.0, 40.0), (50.0, 60.0)), "{segs:?}");
}

#[test]
fn an_end_grip_takes_the_end_off_its_pin() {
    let (sheet, segment) = segment_with_leg();
    let mut engine = Engine::new(sheet).unwrap();
    grip(&mut engine, segment, true, (30.0, 40.0));
    let segs = segments(&engine);
    assert!(has(&segs, (30.0, 40.0), (80.0, 40.0)), "{segs:?}");
    assert!(!ends_at(&segs, (20.0, 40.0)), "no stub to U1: {segs:?}");
}

#[test]
fn an_end_grip_onto_the_other_end_changes_nothing() {
    let (sheet, segment) = segment_with_leg();
    let before_wires = wires(&sheet);
    let mut engine = Engine::new(sheet).unwrap();
    let done = engine
        .execute(Command::MoveWireEnd {
            wire: segment,
            start: false,
            to: Point::new(20.0, 40.0),
            keep_wires: true,
        })
        .unwrap();
    assert!(!done.changed);
    assert_eq!(wires(engine.document()), before_wires);
}
