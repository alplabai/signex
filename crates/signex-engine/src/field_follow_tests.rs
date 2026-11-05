//! A designator or value text selected together with its symbol moves and
//! turns once, with the symbol, not a second time on its own (#2087, #2094).

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

/// Turning a part with its texts selected turns the texts once, through the
/// part, and leaves them to the autoplacer as a turn of the part alone does
/// (#2094).
#[test]
fn fields_selected_with_their_symbol_turn_once() {
    let mut alone = engine();
    alone
        .execute(Command::RotateSelection {
            items: vec![SelectedItem::new(Uuid::from_u128(1), SelectedKind::Symbol)],
            angle_degrees: 90.0,
            keep_wires: false,
        })
        .expect("rotate the part");
    let mut together = engine();
    together
        .execute(Command::RotateSelection {
            items: with_fields(),
            angle_degrees: 90.0,
            keep_wires: false,
        })
        .expect("rotate the part with its texts");
    let (a, b) = (
        &alone.document().symbols[0],
        &together.document().symbols[0],
    );
    assert_eq!(a.rotation, b.rotation);
    assert_eq!(a.ref_text, b.ref_text, "designator");
    assert_eq!(a.val_text, b.val_text, "value");
    assert_eq!(a.fields_user_placed, b.fields_user_placed);
    assert!(
        !b.fields_user_placed,
        "a part turn leaves the texts to autoplace"
    );
}

#[test]
fn a_field_turned_on_its_own_still_turns() {
    let mut engine = engine();
    engine
        .execute(Command::RotateSelection {
            items: vec![SelectedItem::new(
                Uuid::from_u128(1),
                SelectedKind::SymbolRefField,
            )],
            angle_degrees: 90.0,
            keep_wires: false,
        })
        .expect("rotate the designator");
    let symbol = &engine.document().symbols[0];
    assert_eq!(symbol.ref_text.as_ref().expect("ref text").rotation, 90.0);
    assert!(symbol.fields_user_placed);
}

/// A box around several parts selects each part and its texts; one move of
/// that selection moves every text once, with its own part (the report:
/// texts moved 2 mm when their parts moved 1 mm).
#[test]
fn a_box_selection_of_several_parts_moves_each_text_once() {
    let part = |n: u128, x: f64| Symbol {
        uuid: Uuid::from_u128(n),
        reference: format!("R{n}"),
        position: Point::new(x, 10.0),
        unit: 1,
        ref_text: Some(text_at(x - 5.0, 8.0)),
        val_text: Some(text_at(x - 5.0, 12.0)),
        ..Default::default()
    };
    let mut engine = Engine::new(SchematicSheet {
        symbols: vec![part(1, 10.0), part(2, 30.0)],
        ..Default::default()
    })
    .expect("engine");
    let items: Vec<SelectedItem> = [1u128, 2]
        .into_iter()
        .flat_map(|n| {
            let id = Uuid::from_u128(n);
            [
                SelectedItem::new(id, SelectedKind::SymbolRefField),
                SelectedItem::new(id, SelectedKind::Symbol),
                SelectedItem::new(id, SelectedKind::SymbolValField),
            ]
        })
        .collect();
    engine
        .execute(Command::MoveSelection {
            items,
            dx: 1.0,
            dy: 0.0,
            keep_wires: true,
        })
        .expect("move");
    for (symbol, x) in engine.document().symbols.iter().zip([10.0, 30.0]) {
        assert_near(symbol.position, (x + 1.0, 10.0), "part");
        assert_near(
            symbol.ref_text.as_ref().expect("ref").position,
            (x - 4.0, 8.0),
            "designator",
        );
        assert_near(
            symbol.val_text.as_ref().expect("value").position,
            (x - 4.0, 12.0),
            "value",
        );
    }
}
