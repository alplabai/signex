//! A pin end inside a wire joins it through a dot (2026-11-18): the edit that
//! makes the contact mints the dot in its own undo step, a dragged wire then
//! carries the pin with a stub, and a sheet that already had the bare contact
//! is reported and repaired in one step, never changed on open.

use crate::rubber_band_tests::{add_wire, has, lib_pin, lib_symbol, mv, net_shape, place, segments};
use crate::test_support::test_sheet;
use crate::{Command, Engine, bare_pin_taps, pin_junction_repairs};
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

/// Place another one-pin part through the engine, its pin tip at `at`.
fn place_part(engine: &mut Engine, reference: &str, at: (f64, f64)) -> Uuid {
    let mut part = engine.document().symbols[0].clone();
    part.uuid = Uuid::now_v7();
    part.reference = reference.to_string();
    part.position = Point::new(at.0, at.1);
    let id = part.uuid;
    engine
        .execute(Command::PlaceSymbol { symbol: part })
        .unwrap();
    id
}

fn dots(engine: &Engine) -> Vec<Point> {
    let sheet = engine.document();
    sheet.junctions.iter().map(|j| j.position).collect()
}

/// Whether one net holds every part of `ids` (pin 1 each).
fn one_net(sheet: &SchematicSheet, ids: &[Uuid]) -> bool {
    net_shape(sheet).into_iter().any(|(_, pins)| {
        ids.iter().all(|id| pins.contains(&(*id, "1".to_string())))
    })
}

/// J1's pin at the left end of a wire (20,40)-(80,40).
fn wire_from_j1() -> (Engine, Uuid, Uuid) {
    let mut sheet = test_sheet();
    let j1 = add_pin(&mut sheet, "J1", (20.0, 40.0));
    let wire = add_wire(&mut sheet, (20.0, 40.0), (80.0, 40.0));
    (Engine::new(sheet).unwrap(), j1, wire)
}

#[test]
fn placing_a_part_with_a_pin_end_inside_a_wire_joins_it_through_a_dot() {
    let (mut engine, j1, _) = wire_from_j1();
    let tp1 = place_part(&mut engine, "TP1", (50.0, 40.0));
    assert_eq!(dots(&engine), vec![Point::new(50.0, 40.0)]);
    let dot = &engine.document().junctions[0];
    assert!(dot.minted, "an automatic dot");
    assert!(one_net(engine.document(), &[j1, tp1]));

    // The dot is part of the placement's undo step.
    engine.undo().unwrap();
    assert!(dots(&engine).is_empty());
    assert_eq!(engine.document().symbols.len(), 1);
}

#[test]
fn a_part_on_a_wire_end_needs_no_dot() {
    let (mut engine, _, _) = wire_from_j1();
    place_part(&mut engine, "TP1", (80.0, 40.0));
    assert!(dots(&engine).is_empty());
}

#[test]
fn drawing_a_wire_through_a_pin_end_joins_it_and_moving_the_part_off_drops_the_dot() {
    let mut sheet = test_sheet();
    let tp1 = add_pin(&mut sheet, "TP1", (50.0, 40.0));
    let mut engine = Engine::new(sheet).unwrap();
    let wire = signex_types::schematic::Wire {
        uuid: Uuid::now_v7(),
        start: Point::new(20.0, 40.0),
        end: Point::new(80.0, 40.0),
        ..Default::default()
    };
    engine
        .execute(Command::PlaceWireSegmentAsIs { wire })
        .unwrap();
    assert_eq!(dots(&engine), vec![Point::new(50.0, 40.0)]);

    let part = SelectedItem::new(tp1, SelectedKind::Symbol);
    mv(&mut engine, vec![part], 0.0, -20.0, false);
    assert!(dots(&engine).is_empty(), "the pin left: its dot goes");
}

/// The FB1 / F1 case: parts placed on a wire mid-span get their dots, so a
/// drag of the wire leaves a stub to each, and every pin stays on the net.
#[test]
fn a_dragged_wire_keeps_the_parts_placed_on_it() {
    let (mut engine, j1, segment) = wire_from_j1();
    let mut parts = vec![j1];
    for (name, x) in [("FB1", 35.0), ("F1", 50.0), ("L", 65.0)] {
        parts.push(place_part(&mut engine, name, (x, 40.0)));
    }
    let before = net_shape(engine.document());
    assert!(one_net(engine.document(), &parts));

    let item = SelectedItem::new(segment, SelectedKind::Wire);
    mv(&mut engine, vec![item], 0.0, 10.0, true);
    let segs = segments(&engine);
    for x in [20.0, 35.0, 50.0, 65.0] {
        assert!(has(&segs, (x, 40.0), (x, 50.0)), "{x}: {segs:?}");
    }
    assert_eq!(net_shape(engine.document()), before, "every pin stays");
}

#[test]
fn an_old_bare_contact_is_reported_untouched_and_repaired_in_one_step() {
    let mut sheet = test_sheet();
    let j1 = add_pin(&mut sheet, "J1", (20.0, 40.0));
    let tp1 = add_pin(&mut sheet, "TP1", (50.0, 40.0));
    add_wire(&mut sheet, (20.0, 40.0), (80.0, 40.0));

    // Opening reads the sheet as it is: the contact is reported, not fixed.
    let mut engine = Engine::new(sheet).unwrap();
    let taps = bare_pin_taps(engine.document());
    assert_eq!(taps, vec![Point::new(50.0, 40.0)]);
    assert!(dots(&engine).is_empty());
    assert!(!one_net(engine.document(), &[j1, tp1]));

    let repairs = pin_junction_repairs(engine.document());
    engine.execute_batch(repairs).unwrap();
    assert_eq!(dots(&engine), vec![Point::new(50.0, 40.0)]);
    assert!(one_net(engine.document(), &[j1, tp1]));
    assert!(bare_pin_taps(engine.document()).is_empty());

    engine.undo().unwrap();
    assert!(dots(&engine).is_empty(), "one undo step");
}
