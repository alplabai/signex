//! Rubber-band rotate and mirror (#1115): the guarded pass of #1031 runs
//! after a turn too, so wires on the turned pins follow them.

use crate::rubber_band_tests::{
    Case, Lcg, add_resistor, add_wire, assert_clean, check_closure_oracle, describe, lib_pin,
    lib_symbol, moved_pin_items, net_shape, place, random_case, segments, series_fixture,
    symbol_item, warned_items,
};
use crate::test_support::test_sheet;
use crate::{Command, Engine, MirrorAxis, SelectionTransform};
use signex_types::schematic::{Point, SelectedItem, SelectedKind};
use uuid::Uuid;

fn rotate(engine: &mut Engine, items: Vec<SelectedItem>, keep: bool) {
    engine
        .execute(Command::RotateSelection {
            items,
            angle_degrees: 90.0,
            keep_wires: keep,
        })
        .expect("rotate");
}

fn mirror(engine: &mut Engine, items: Vec<SelectedItem>, axis: MirrorAxis, keep: bool) {
    engine
        .execute(Command::MirrorSelection {
            items,
            axis,
            keep_wires: keep,
        })
        .expect("mirror");
}

/// `T1` at (50, 50) with pin A at (45, 50) and pin B at (55, 55), each
/// wired to a stationary far end.
fn two_level_fixture() -> (Engine, Uuid) {
    let mut sheet = test_sheet();
    let mut pin_a = lib_pin("A", -5.0);
    let mut pin_b = lib_pin("B", 5.0);
    pin_a.pin.number = "A".to_string();
    pin_b.pin.number = "B".to_string();
    pin_b.pin.position = Point::new(5.0, -5.0);
    sheet
        .lib_symbols
        .insert("T".to_string(), lib_symbol("T", vec![pin_a, pin_b]));
    let t1 = place(&mut sheet, "T1", "T", Point::new(50.0, 50.0));
    add_wire(&mut sheet, (30.0, 50.0), (45.0, 50.0));
    add_wire(&mut sheet, (55.0, 55.0), (75.0, 55.0));
    (Engine::new(sheet).expect("engine"), t1)
}

#[test]
fn rotating_a_wired_symbol_keeps_its_nets() {
    let (mut engine, r1, _) = series_fixture();
    let before = net_shape(engine.document());
    rotate(&mut engine, vec![symbol_item(r1)], true);
    assert_clean(&mut engine, &before);
    assert!(segments(&engine).len() > 3, "wires bent to follow the pins");
}

#[test]
fn the_plain_rotate_breaks_the_nets() {
    let (mut engine, r1, _) = series_fixture();
    let before = net_shape(engine.document());
    rotate(&mut engine, vec![symbol_item(r1)], false);
    assert_ne!(net_shape(engine.document()), before);
}

#[test]
fn rotate_and_wire_edits_are_one_undo_step() {
    let (mut engine, r1, _) = series_fixture();
    let wires: Vec<_> = engine
        .document()
        .wires
        .iter()
        .map(|w| (w.uuid, w.start, w.end))
        .collect();
    let rotation = engine.document().symbols[0].rotation;
    rotate(&mut engine, vec![symbol_item(r1)], true);
    engine.undo().expect("undo");
    let after: Vec<_> = engine
        .document()
        .wires
        .iter()
        .map(|w| (w.uuid, w.start, w.end))
        .collect();
    assert_eq!(after, wires, "one undo restores every wire");
    assert_eq!(engine.document().symbols[0].rotation, rotation);
}

#[test]
fn mirroring_a_wired_symbol_keeps_its_nets() {
    let (mut engine, t1) = two_level_fixture();
    let before = net_shape(engine.document());
    mirror(
        &mut engine,
        vec![symbol_item(t1)],
        MirrorAxis::Vertical,
        true,
    );
    assert_clean(&mut engine, &before);
    engine.undo().expect("undo");
    assert_eq!(net_shape(engine.document()), before);
    assert_eq!(engine.document().wires.len(), 2, "one undo restores both");
}

#[test]
fn a_transform_of_the_selection_keeps_the_nets_to_stationary_parts() {
    let (mut engine, t1) = two_level_fixture();
    let before = net_shape(engine.document());
    engine
        .execute(Command::TransformSelection {
            items: vec![symbol_item(t1)],
            centre: Point::new(50.0, 50.0),
            transform: SelectionTransform::RotateClockwise,
            keep_wires: true,
        })
        .expect("transform");
    assert_clean(&mut engine, &before);
    engine.undo().expect("undo");
    assert_eq!(net_shape(engine.document()), before);
}

#[test]
fn a_mirror_that_would_cross_nets_falls_back_to_the_plain_turn_and_warns() {
    // Mirroring R1 swaps its pins across the two wires: no path keeps both
    // nets, so the guard leaves the plain mirror and says so.
    let (mut engine, r1, _) = series_fixture();
    let plain_wires = series_fixture().0.document().wires.len();
    mirror(
        &mut engine,
        vec![symbol_item(r1)],
        MirrorAxis::Horizontal,
        true,
    );
    let warnings = engine.take_warnings();
    assert!(
        warnings
            .iter()
            .any(|w| w.contains("could not keep") && w.contains("R1")),
        "{warnings:?}"
    );
    assert_eq!(engine.document().wires.len(), plain_wires);
}

#[test]
fn keep_wires_off_turns_alone() {
    let mut sheet = test_sheet();
    let r1 = add_resistor(&mut sheet, "R1", Point::new(50.0, 50.0));
    add_wire(&mut sheet, (30.0, 50.0), (45.0, 50.0));
    let mut engine = Engine::new(sheet).expect("engine");
    let before = segments(&engine);
    rotate(&mut engine, vec![symbol_item(r1)], false);
    assert_eq!(segments(&engine), before);
}

#[test]
fn random_turns_never_change_the_netlist_beyond_the_plain_turn() {
    let mut exercised = 0;
    let mut rerouted = 0;
    for seed in [0x5EED_1115, 0xFACE_FEED, 0x00DD_BA11] {
        let mut rng = Lcg(seed);
        for index in 0..800 {
            let Some(Case { sheet, moves, .. }) = random_case(&mut rng) else {
                continue;
            };
            let items: Vec<_> = moves
                .iter()
                .map(|(item, _, _)| *item)
                .filter(|i| i.kind == SelectedKind::Symbol)
                .collect();
            if items.is_empty() {
                continue;
            }
            let kind = rng.next(3);
            let run = |keep: bool| {
                let mut engine = Engine::new(sheet.clone()).expect("engine");
                let cmd = match kind {
                    0 => Command::RotateSelection {
                        items: items.clone(),
                        angle_degrees: 90.0,
                        keep_wires: keep,
                    },
                    1 => Command::MirrorSelection {
                        items: items.clone(),
                        axis: MirrorAxis::Vertical,
                        keep_wires: keep,
                    },
                    _ => Command::TransformSelection {
                        items: items.clone(),
                        centre: Point::new(50.0, 40.0),
                        transform: SelectionTransform::RotateCounterclockwise,
                        keep_wires: keep,
                    },
                };
                engine.execute(cmd).expect("turn");
                engine
            };
            let plain = run(false);
            let mut engine = run(true);
            let _warnings = engine.take_warnings();
            let unkept = engine.take_unkept_pins();
            let tips = crate::moved_pin_tips(&sheet, &items);
            exercised += 1;
            rerouted += usize::from(engine.document().wires.len() != sheet.wires.len());
            for w in &engine.document().wires {
                assert_ne!(
                    signex_net::pt_key(&w.start),
                    signex_net::pt_key(&w.end),
                    "seed {seed:#x} case {index}: zero-length wire\n{}",
                    describe(&sheet)
                );
            }
            if let Err(why) = check_closure_oracle(
                &sheet,
                plain.document(),
                engine.document(),
                &warned_items(&sheet, &unkept),
                &moved_pin_items(&tips),
            ) {
                panic!(
                    "seed {seed:#x} case {index} kind {kind}: {why}\nBEFORE {}\nAFTER {}",
                    describe(&sheet),
                    describe(engine.document())
                );
            }
        }
    }
    assert!(exercised > 500, "only {exercised} cases");
    assert!(rerouted > 50, "only {rerouted} cases rerouted wires");
}
