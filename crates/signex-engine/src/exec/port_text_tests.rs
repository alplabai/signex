//! A power port's text is fixed to the port (maintainer decision 2026-11-07,
//! `docs/DECISIONS.md`): its field items act on the whole port, once.

use crate::test_support::test_sheet;
use crate::{Command, Engine, MirrorAxis};
use signex_types::schematic::{Point, SelectedItem, SelectedKind, Symbol, power_port_value_field};
use uuid::Uuid;

const AT: (f64, f64) = (50.8, 25.4);

fn engine_with(is_power: bool) -> (Engine, Uuid) {
    let mut sheet = test_sheet();
    let id = Uuid::now_v7();
    let at = Point::new(AT.0, AT.1);
    sheet.symbols.push(Symbol {
        uuid: id,
        lib_id: "power:VCC".into(),
        reference: "#PWR01".into(),
        value: "VCC".into(),
        position: at,
        is_power,
        val_text: Some(power_port_value_field("power:VCC", at)),
        ..Default::default()
    });
    (Engine::new(sheet).unwrap(), id)
}

fn port(engine: &Engine) -> &Symbol {
    &engine.document().symbols[0]
}

fn value_field(id: Uuid) -> SelectedItem {
    SelectedItem::new(id, SelectedKind::SymbolValField)
}

#[test]
fn moving_a_power_ports_text_moves_the_port() {
    let (mut engine, id) = engine_with(true);
    let text_before = port(&engine).val_text.clone().unwrap().position;
    engine
        .execute(Command::MoveSelection {
            items: vec![value_field(id)],
            dx: 2.54,
            dy: 5.08,
            keep_wires: false,
        })
        .unwrap();
    let moved = port(&engine);
    assert_eq!(moved.position, Point::new(AT.0 + 2.54, AT.1 + 5.08));
    let text = moved.val_text.as_ref().unwrap().position;
    assert_eq!(text, Point::new(text_before.x + 2.54, text_before.y + 5.08));
}

#[test]
fn a_port_and_its_text_selected_together_move_once() {
    let (mut engine, id) = engine_with(true);
    engine
        .execute(Command::MoveSelection {
            items: vec![
                SelectedItem::new(id, SelectedKind::Symbol),
                value_field(id),
                SelectedItem::new(id, SelectedKind::SymbolRefField),
            ],
            dx: 2.54,
            dy: 0.0,
            keep_wires: false,
        })
        .unwrap();
    assert_eq!(port(&engine).position, Point::new(AT.0 + 2.54, AT.1));
}

#[test]
fn turning_or_mirroring_a_power_ports_text_turns_the_port() {
    let (mut engine, id) = engine_with(true);
    engine
        .execute(Command::RotateSelection {
            items: vec![value_field(id)],
            angle_degrees: 90.0,
            keep_wires: false,
        })
        .unwrap();
    assert_eq!(port(&engine).rotation, 90.0);
    engine
        .execute(Command::MirrorSelection {
            items: vec![value_field(id)],
            axis: MirrorAxis::Vertical,
            keep_wires: false,
        })
        .unwrap();
    assert!(port(&engine).mirror_x);
}

#[test]
fn a_parts_value_text_still_moves_on_its_own() {
    let (mut engine, id) = engine_with(false);
    let text_before = port(&engine).val_text.clone().unwrap().position;
    engine
        .execute(Command::MoveSelection {
            items: vec![value_field(id)],
            dx: 2.54,
            dy: 0.0,
            keep_wires: false,
        })
        .unwrap();
    let part = port(&engine);
    assert_eq!(part.position, Point::new(AT.0, AT.1), "the body stays");
    assert_eq!(
        part.val_text.as_ref().unwrap().position,
        Point::new(text_before.x + 2.54, text_before.y)
    );
}
