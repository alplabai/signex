//! `Command::SetSymbolBodyStyle` (#1233): allowed when no wired pin is
//! disturbed (with undo / redo), refused when one would be, and the
//! `ReplaceLibSymbol` interplay for an instance showing the alternate body.

use crate::test_support::test_sheet;
use crate::{Command, Engine, EngineError};
use signex_types::schematic::{
    BodyStyle, LibPin, LibSymbol, Pin, PinDirection, PinShapeStyle, Point, Symbol, Wire,
};
use uuid::Uuid;

const ID: &str = "L:G";

fn lib_pin(body_style: u32, number: &str, x: f64) -> LibPin {
    LibPin {
        unit: 1,
        body_style,
        pin: Pin {
            direction: PinDirection::Passive,
            shape_style: PinShapeStyle::Plain,
            position: Point::new(x, 0.0),
            rotation: 0.0,
            length: 2.54,
            name: number.to_string(),
            number: number.to_string(),
            visible: true,
            name_visible: true,
            number_visible: true,
        },
    }
}

fn lib_symbol(pins: Vec<LibPin>) -> LibSymbol {
    LibSymbol {
        id: ID.to_string(),
        reference: "U".into(),
        value: String::new(),
        footprint: String::new(),
        datasheet: String::new(),
        description: String::new(),
        keywords: String::new(),
        fp_filters: String::new(),
        in_bom: true,
        on_board: true,
        in_pos_files: true,
        duplicate_pin_numbers_are_jumpers: false,
        graphics: Vec::new(),
        pins,
        show_pin_numbers: true,
        show_pin_names: true,
        pin_name_offset: 0.0,
    }
}

/// Symbol U1 at (10, 10) of `pins`, with a wire ending at world (12.54, 10),
/// i.e. on a pin at local x = 2.54.
fn engine(pins: Vec<LibPin>, body_style: BodyStyle) -> (Engine, Uuid) {
    let (sheet, uuid) = sheet(pins, body_style);
    (Engine::new(sheet).unwrap(), uuid)
}

fn sheet(
    pins: Vec<LibPin>,
    body_style: BodyStyle,
) -> (signex_types::schematic::SchematicSheet, Uuid) {
    let mut sheet = test_sheet();
    sheet.lib_symbols.insert(ID.into(), lib_symbol(pins));
    let uuid = Uuid::new_v4();
    sheet.symbols.push(Symbol {
        uuid,
        lib_id: ID.into(),
        reference: "U1".into(),
        position: Point::new(10.0, 10.0),
        body_style,
        ..Default::default()
    });
    sheet.wires.push(Wire {
        start: Point::new(12.54, 10.0),
        end: Point::new(20.0, 10.0),
        ..Default::default()
    });
    (sheet, uuid)
}

fn body_of(engine: &Engine) -> BodyStyle {
    engine.document().symbols[0].body_style
}

fn switch(
    engine: &mut Engine,
    symbol_id: Uuid,
    body_style: BodyStyle,
) -> Result<bool, EngineError> {
    engine
        .execute(Command::SetSymbolBodyStyle {
            symbol_id,
            body_style,
        })
        .map(|r| r.changed)
}

#[test]
fn switch_is_allowed_when_only_unwired_pins_move_and_undoes_and_redoes() {
    // Wired pin "1" sits at 2.54 in both bodies; pin "2" is unwired and moves.
    let (mut e, id) = engine(
        vec![
            lib_pin(0, "1", 2.54),
            lib_pin(1, "2", -2.54),
            lib_pin(2, "2", -5.08),
        ],
        BodyStyle::Normal,
    );
    assert!(switch(&mut e, id, BodyStyle::Alternate).unwrap());
    assert_eq!(body_of(&e), BodyStyle::Alternate);
    e.undo().unwrap();
    assert_eq!(body_of(&e), BodyStyle::Normal);
    e.redo().unwrap();
    assert_eq!(body_of(&e), BodyStyle::Alternate);
}

#[test]
fn switch_is_refused_when_a_wired_pin_would_move() {
    let (mut e, id) = engine(
        vec![lib_pin(1, "1", 2.54), lib_pin(2, "1", -2.54)],
        BodyStyle::Normal,
    );
    match switch(&mut e, id, BodyStyle::Alternate).unwrap_err() {
        EngineError::SymbolBodyPinsConnected { reference, pins } => {
            assert_eq!(reference, "U1");
            assert_eq!(pins, vec!["U1 pin 1".to_string()]);
        }
        other => panic!("unexpected error: {other:?}"),
    }
    assert_eq!(body_of(&e), BodyStyle::Normal);
    assert!(!e.can_undo());
}

#[test]
fn switch_is_refused_when_a_pin_would_land_on_a_connection() {
    // Normal body has no pin at the wire end; the alternate pin lands on it.
    let (mut e, id) = engine(
        vec![lib_pin(1, "1", -2.54), lib_pin(2, "2", 2.54)],
        BodyStyle::Normal,
    );
    assert!(matches!(
        switch(&mut e, id, BodyStyle::Alternate),
        Err(EngineError::SymbolBodyPinsConnected { .. })
    ));
}

#[test]
fn switch_to_an_alternate_the_library_lacks_changes_nothing_on_screen_and_is_allowed() {
    let (mut e, id) = engine(vec![lib_pin(1, "1", 2.54)], BodyStyle::Normal);
    assert!(switch(&mut e, id, BodyStyle::Alternate).unwrap());
    assert_eq!(body_of(&e), BodyStyle::Alternate);
}

#[test]
fn switching_to_the_current_body_records_nothing() {
    let (mut e, id) = engine(vec![lib_pin(1, "1", 2.54)], BodyStyle::Normal);
    assert!(!switch(&mut e, id, BodyStyle::Normal).unwrap());
    assert!(!e.can_undo());
}

#[test]
fn replacing_a_lib_symbol_that_loses_the_wired_alternate_pin_is_refused() {
    let (mut e, _) = engine(
        vec![lib_pin(1, "1", -2.54), lib_pin(2, "1", 2.54)],
        BodyStyle::Alternate,
    );
    let fresh = lib_symbol(vec![lib_pin(1, "1", -2.54)]);
    let err = e
        .execute(Command::ReplaceLibSymbol {
            lib_symbol: fresh,
            library_versions: vec![],
        })
        .unwrap_err();
    assert!(matches!(err, EngineError::LibSymbolPinsConnected { .. }));
    assert_eq!(body_of(&e), BodyStyle::Alternate);
}

#[test]
fn replacing_a_lib_symbol_that_loses_an_unwired_alternate_keeps_the_stored_body() {
    let (mut e, _) = engine(
        vec![
            lib_pin(0, "1", 2.54),
            lib_pin(1, "2", -2.54),
            lib_pin(2, "2", -5.08),
        ],
        BodyStyle::Alternate,
    );
    let fresh = lib_symbol(vec![lib_pin(0, "1", 2.54), lib_pin(1, "2", -2.54)]);
    assert!(
        e.execute(Command::ReplaceLibSymbol {
            lib_symbol: fresh,
            library_versions: vec![],
        })
        .unwrap()
        .changed
    );
    // The choice is not dropped; the symbol draws as Normal until the library
    // gains an alternate again.
    assert_eq!(body_of(&e), BodyStyle::Alternate);
}

/// A `body_style` value this build cannot read is kept on the symbol until
/// the user picks a body; picking the body already shown counts.
#[test]
fn picking_normal_clears_a_kept_unreadable_value() {
    let (mut doc, id) = sheet(vec![lib_pin(0, "1", 2.54)], BodyStyle::Normal);
    // Same shape a load of `body_style = 3` leaves behind.
    doc.symbols[0].unknown =
        serde_json::from_value(serde_json::json!({ "body_style": 3 })).unwrap();
    let mut e = Engine::new(doc).unwrap();
    assert!(e.document().symbols[0].unknown.contains_key("body_style"));
    assert!(switch(&mut e, id, BodyStyle::Normal).unwrap());
    assert!(!e.document().symbols[0].unknown.contains_key("body_style"));
    e.undo().unwrap();
    assert!(e.document().symbols[0].unknown.contains_key("body_style"));
}
