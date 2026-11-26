//! `Command::ChangeComponent` with a pin mapping given by hand (#2954): the
//! swap follows it, carries connections to the new pins, is one undo step,
//! and refuses a mapping it cannot follow. The fixture is
//! `change_component_tests::engine_with_wired_pin_2`: U1 at (10, 10), old
//! pin "1" at (7.46, 10), old pin "2" at (12.54, 10) with a wire to (20, 10).

use super::change_component::change_component_preview;
use super::change_component_pins::{PinChoice, PinMapRow, pin_choices, pin_table};
use super::change_component_tests::{OLD, engine_with_wired_pin_2, lib_pin, replacement};
use crate::{Command, DocumentPatch, Engine, EngineError};
use signex_types::schematic::{Label, LabelType, PinMapping, PinMatchedBy, PinPair, Point};
use uuid::Uuid;

fn pair(old: &str, new: &str) -> PinPair {
    PinPair {
        old: old.into(),
        new: new.into(),
        by: PinMatchedBy::Chosen,
    }
}

fn hand(pairs: Vec<PinPair>, unmatched: &[&str]) -> Option<PinMapping> {
    Some(PinMapping {
        pairs,
        unmatched: unmatched.iter().map(|k| k.to_string()).collect(),
    })
}

fn near(a: Point, b: (f64, f64)) -> bool {
    (a.x - b.0).abs() < 1e-6 && (a.y - b.1).abs() < 1e-6
}

/// The fixture with its wire turned down from pin "2": (12.54, 10) to
/// (12.54, 20), so a carry sideways crosses no other pin.
fn engine_with_a_wire_down_from_pin_2() -> (Engine, Uuid) {
    let (mut engine, id) = engine_with_wired_pin_2();
    let mut doc = engine.document().clone();
    doc.wires[0].end = Point::new(12.54, 20.0);
    engine.set_document(doc);
    (engine, id)
}

#[test]
fn a_hand_mapping_carries_the_wire_to_the_new_pin_in_one_undo_step() {
    let (mut engine, id) = engine_with_wired_pin_2();
    // The new pin "2" sits 5.08 mm further out, at (17.62, 10).
    let mut r = replacement(vec![lib_pin("1", "IN", -2.54), lib_pin("2", "OUT", 7.62)]);
    r.pin_map = hand(vec![pair("1", "1"), pair("2", "2")], &[]);

    let preview = change_component_preview(engine.document(), id, &r);
    assert_eq!(preview.carried, vec!["2".to_string()]);
    assert!(preview.moved_connected.is_empty() && preview.refused.is_none());
    assert!(preview.is_clean());

    let result = engine
        .execute(Command::ChangeComponent {
            symbol_id: id,
            replacement: r,
        })
        .unwrap();
    let patch = result.patch_pair.expect("a change").document;
    assert!(patch.contains(DocumentPatch::WIRES | DocumentPatch::SYMBOLS));
    let wire = &engine.document().wires[0];
    assert!(near(wire.start, (17.62, 10.0)), "{wire:?}");
    assert!(near(wire.end, (20.0, 10.0)));

    engine.undo().unwrap();
    let doc = engine.document();
    assert_eq!(doc.symbols[0].lib_id, OLD);
    assert!(near(doc.wires[0].start, (12.54, 10.0)));
    assert!(!engine.can_undo(), "one undo undid the swap and the carry");
}

#[test]
fn a_remapped_pin_takes_over_the_old_pins_connection_and_uuid() {
    let (mut engine, id) = engine_with_a_wire_down_from_pin_2();
    let uuid = Uuid::from_u128(0xB1);
    let mut doc = engine.document().clone();
    doc.symbols[0].pin_uuids.insert("2".into(), uuid);
    engine.set_document(doc);
    let mut r = replacement(vec![lib_pin("1", "IN", -2.54), lib_pin("2", "OUT", 2.54)]);
    // Old "2" (wired) goes to new "1"; old "1" (unwired) to new "2".
    r.pin_map = hand(vec![pair("1", "2"), pair("2", "1")], &[]);

    engine
        .execute(Command::ChangeComponent {
            symbol_id: id,
            replacement: r,
        })
        .unwrap();
    let doc = engine.document();
    assert!(near(doc.wires[0].start, (7.46, 10.0)), "{:?}", doc.wires[0]);
    assert!(near(doc.wires[0].end, (12.54, 20.0)));
    assert_eq!(doc.symbols[0].pin_uuids.get("1"), Some(&uuid));
}

#[test]
fn a_carry_that_would_join_another_net_is_refused_and_changes_nothing() {
    // The wire runs on to (20, 10): carried to new pin "1" at (7.46, 10) it
    // would run through new pin "2" at (12.54, 10).
    let (mut engine, id) = engine_with_wired_pin_2();
    let before = engine.document().clone();
    let mut r = replacement(vec![lib_pin("1", "IN", -2.54), lib_pin("2", "OUT", 2.54)]);
    r.pin_map = hand(vec![pair("1", "2"), pair("2", "1")], &[]);

    let preview = change_component_preview(engine.document(), id, &r);
    assert!(preview.refused.is_some(), "{preview:?}");
    let refused = engine.execute(Command::ChangeComponent {
        symbol_id: id,
        replacement: r,
    });
    assert!(matches!(
        refused,
        Err(EngineError::ChangeComponentPins { .. })
    ));
    assert_eq!(engine.document().symbols, before.symbols);
    assert_eq!(engine.document().wires, before.wires);
    assert!(!engine.can_undo());
}

#[test]
fn a_pin_mapped_to_no_pin_dangles() {
    let (mut engine, id) = engine_with_wired_pin_2();
    let mut r = replacement(vec![lib_pin("1", "IN", -2.54), lib_pin("2", "OUT", 2.54)]);
    r.pin_map = hand(vec![pair("1", "1")], &["2"]);

    let preview = change_component_preview(engine.document(), id, &r);
    assert_eq!(preview.unmatched_connected, vec!["2".to_string()]);
    assert_eq!(preview.mapping.unmatched, vec!["2".to_string()]);
    // New pin "2" sits on the wire it is not mapped to.
    assert_eq!(preview.gained, vec!["2".to_string()]);
    engine
        .execute(Command::ChangeComponent {
            symbol_id: id,
            replacement: r,
        })
        .unwrap();
    assert!(near(engine.document().wires[0].start, (12.54, 10.0)));
}

#[test]
fn a_new_pin_on_the_tip_a_carry_empties_gains_nothing() {
    let (engine, id) = engine_with_a_wire_down_from_pin_2();
    let mut r = replacement(vec![lib_pin("1", "IN", -2.54), lib_pin("2", "OUT", 2.54)]);
    r.pin_map = hand(vec![pair("1", "2"), pair("2", "1")], &[]);
    let preview = change_component_preview(engine.document(), id, &r);
    assert_eq!(preview.carried, vec!["2".to_string()]);
    assert!(preview.gained.is_empty(), "{preview:?}");
    assert!(preview.refused.is_none());
}

#[test]
fn a_hand_mapping_the_part_cannot_follow_is_refused() {
    let (mut engine, id) = engine_with_wired_pin_2();
    let pins = || vec![lib_pin("1", "IN", -2.54), lib_pin("2", "OUT", 2.54)];
    for (pairs, why) in [
        (vec![pair("2", "7")], "no pin 7 on the new part"),
        (vec![pair("5", "1")], "no pin 5 on the part"),
        (
            vec![pair("1", "1"), pair("2", "1")],
            "new pin 1 taken twice",
        ),
    ] {
        let mut r = replacement(pins());
        r.pin_map = hand(pairs, &[]);
        let refused = engine.execute(Command::ChangeComponent {
            symbol_id: id,
            replacement: r,
        });
        assert!(
            matches!(refused, Err(EngineError::ChangeComponentPins { .. })),
            "{why}: {refused:?}"
        );
    }
    assert!(!engine.can_undo());
}

#[test]
fn without_a_hand_mapping_a_moved_pin_is_still_left_where_it_was() {
    let (mut engine, id) = engine_with_wired_pin_2();
    let r = replacement(vec![lib_pin("1", "IN", -2.54), lib_pin("2", "OUT", 7.62)]);
    let preview = change_component_preview(engine.document(), id, &r);
    assert_eq!(preview.moved_connected, vec!["2".to_string()]);
    assert!(preview.carried.is_empty());
    engine
        .execute(Command::ChangeComponent {
            symbol_id: id,
            replacement: r,
        })
        .unwrap();
    assert!(near(engine.document().wires[0].start, (12.54, 10.0)));
}

#[test]
fn the_pin_table_lists_each_old_pin_with_its_new_pin_and_net() {
    let (mut engine, id) = engine_with_wired_pin_2();
    let mut doc = engine.document().clone();
    doc.labels
        .push(Label::new("FB", Point::new(20.0, 10.0), LabelType::Net));
    engine.set_document(doc);
    let mut r = replacement(vec![lib_pin("1", "IN", -2.54), lib_pin("9", "OUT", 2.54)]);
    let rows = pin_table(engine.document(), id, &r);
    assert_eq!(
        rows,
        vec![
            PinMapRow {
                old: "1".into(),
                old_name: "IN".into(),
                new: Some(("1".into(), "IN".into())),
                net: None,
            },
            PinMapRow {
                old: "2".into(),
                old_name: "OUT".into(),
                new: Some(("9".into(), "OUT".into())),
                net: Some("FB".into()),
            },
        ]
    );
    r.pin_map = hand(vec![pair("2", "1")], &["1"]);
    let rows = pin_table(engine.document(), id, &r);
    assert_eq!(rows[0].new, None);
    assert_eq!(rows[1].new, Some(("1".into(), "IN".into())));
    assert_eq!(
        pin_choices(engine.document(), id, &r),
        vec![
            PinChoice {
                key: "1".into(),
                name: "IN".into()
            },
            PinChoice {
                key: "9".into(),
                name: "OUT".into()
            },
        ]
    );
}
