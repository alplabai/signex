//! A designator or value text selected together with its symbol moves once,
//! with the symbol, not a second time on its own (#2087).

use crate::*;
use signex_types::schematic::{Point, SelectedKind, Symbol, TextProp};
use uuid::Uuid;

fn text_at(x: f64, y: f64) -> TextProp {
    TextProp {
        position: Point::new(x, y),
        rotation: 0.0,
        font_size: 1.27,
        font_family: None,
        justify_h: Default::default(),
        justify_v: Default::default(),
        hidden: false,
    }
}

fn engine() -> Engine {
    let sheet = SchematicSheet {
        symbols: vec![Symbol {
            uuid: Uuid::from_u128(1),
            reference: "R1".into(),
            position: Point::new(10.0, 10.0),
            unit: 1,
            ref_text: Some(text_at(5.0, 8.0)),
            val_text: Some(text_at(5.0, 12.0)),
            ..Default::default()
        }],
        ..Default::default()
    };
    Engine::new(sheet).expect("engine")
}

fn texts(engine: &Engine) -> (Point, Point, Point) {
    let symbol = &engine.document().symbols[0];
    (
        symbol.position,
        symbol.ref_text.as_ref().expect("ref text").position,
        symbol.val_text.as_ref().expect("value text").position,
    )
}

fn assert_near(got: Point, want: (f64, f64), what: &str) {
    assert!(
        (got.x - want.0).abs() < 1e-9 && (got.y - want.1).abs() < 1e-9,
        "{what}: got ({}, {}), want {want:?}",
        got.x,
        got.y
    );
}

fn assert_texts(engine: &Engine, want: [(f64, f64); 3], case: &str) {
    let (symbol, reference, value) = texts(engine);
    assert_near(symbol, want[0], &format!("{case}: symbol"));
    assert_near(reference, want[1], &format!("{case}: designator"));
    assert_near(value, want[2], &format!("{case}: value"));
}

fn with_fields() -> Vec<SelectedItem> {
    let uuid = Uuid::from_u128(1);
    vec![
        SelectedItem::new(uuid, SelectedKind::Symbol),
        SelectedItem::new(uuid, SelectedKind::SymbolRefField),
        SelectedItem::new(uuid, SelectedKind::SymbolValField),
    ]
}

#[test]
fn fields_selected_with_their_symbol_move_once() {
    for keep_wires in [false, true] {
        let mut engine = engine();
        engine
            .execute(Command::MoveSelection {
                items: with_fields(),
                dx: 2.54,
                dy: 5.08,
                keep_wires,
            })
            .expect("move");
        assert_texts(
            &engine,
            [(12.54, 15.08), (7.54, 13.08), (7.54, 17.08)],
            &format!("keep_wires = {keep_wires}"),
        );
    }
}

#[test]
fn the_drag_ghost_moves_fields_once_too() {
    let engine = engine();
    let preview = MovePreview::new(engine.document());
    let moves: Vec<_> = with_fields()
        .into_iter()
        .map(|item| (item, 2.54, 5.08))
        .collect();
    let ghost = preview.moved(&moves, true);
    let symbol = &ghost.symbols[0];
    assert_near(
        symbol.ref_text.as_ref().expect("ref text").position,
        (7.54, 13.08),
        "ghost designator",
    );
}

#[test]
fn a_field_moved_on_its_own_still_moves() {
    let mut engine = engine();
    engine
        .execute(Command::MoveSelection {
            items: vec![SelectedItem::new(
                Uuid::from_u128(1),
                SelectedKind::SymbolRefField,
            )],
            dx: 1.27,
            dy: 0.0,
            keep_wires: false,
        })
        .expect("move");
    assert_texts(
        &engine,
        [(10.0, 10.0), (6.27, 8.0), (5.0, 12.0)],
        "field alone",
    );
}
