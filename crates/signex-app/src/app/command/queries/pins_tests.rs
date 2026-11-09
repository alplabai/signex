//! #2199 — `query_pins` lists a part's pins with the tips the canvas hit-test
//! uses, per placed unit, and refuses what it cannot name.

use serde_json::Value;
use signex_types::schematic::{
    LibPin, LibSymbol, Pin, PinDirection, PinShapeStyle, Point, SchematicSheet, Symbol,
};
use uuid::Uuid;

use super::tests::{answer, app_with, path};
use crate::app::Signex;
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::handlers::editing_commands::net_color_driver as fixture;
use crate::keymap::AppCommandId;

fn lib_pin(number: &str, unit: u32, x: f64, visible: bool) -> LibPin {
    LibPin {
        unit,
        body_style: 1,
        pin: Pin {
            direction: PinDirection::Passive,
            shape_style: PinShapeStyle::Plain,
            position: Point::new(x, 0.0),
            rotation: 0.0,
            length: 2.54,
            name: format!("N{number}"),
            number: number.into(),
            visible,
            name_visible: true,
            number_visible: true,
        },
    }
}

fn lib(id: &str, pins: Vec<LibPin>) -> LibSymbol {
    let mut lib: LibSymbol = serde_json::from_value(serde_json::json!({ "id": id })).unwrap();
    lib.pins = pins;
    lib
}

fn part(reference: &str, lib_id: &str, unit: u32, at: (f64, f64)) -> Symbol {
    Symbol {
        uuid: Uuid::now_v7(),
        lib_id: lib_id.into(),
        reference: reference.into(),
        position: Point::new(at.0, at.1),
        unit,
        ..Symbol::default()
    }
}

/// R1 turned 90 degrees, R2 mirrored in Y, a three-unit U1 with units 1 and 3
/// placed (pin 8 is hidden and shared by every unit), a second part also
/// called U2 and an unannotated R?.
pub(in crate::app) fn pin_sheet() -> SchematicSheet {
    let mut sheet = SchematicSheet::default();
    for (id, pins) in [
        (
            "R",
            vec![lib_pin("1", 0, -2.54, true), lib_pin("2", 0, 2.54, true)],
        ),
        (
            "OPAMP",
            vec![
                lib_pin("1", 1, 3.0, true),
                lib_pin("2", 2, 3.0, true),
                lib_pin("3", 3, 3.0, true),
                lib_pin("8", 0, 0.0, false),
            ],
        ),
    ] {
        sheet.lib_symbols.insert(id.into(), lib(id, pins));
    }
    let mut r1 = part("R1", "R", 1, (50.0, 50.0));
    r1.rotation = 90.0;
    let mut r2 = part("R2", "R", 1, (10.0, 10.0));
    r2.mirror_y = true;
    sheet.symbols = vec![
        r1,
        r2,
        part("U1", "OPAMP", 1, (100.0, 100.0)),
        part("U1", "OPAMP", 3, (120.0, 100.0)),
        part("U2", "R", 1, (0.0, 0.0)),
        part("U2", "OPAMP", 2, (5.0, 5.0)),
        part("U3", "R", 1, (0.0, 40.0)),
        part("U3", "R", 1, (0.0, 45.0)),
        part("R?", "R", 1, (0.0, 20.0)),
    ];
    sheet
}

fn app() -> Signex {
    let mut app = app_with(&[]);
    fixture::add_sheet_with(&mut app, &path("pins.snxsch"), pin_sheet());
    app.document_state.mark_document_clean(&path("pins.snxsch"));
    app
}

fn pins_of(app: &mut Signex, designator: &str) -> Vec<Value> {
    let document = format!("document={}", path("pins.snxsch").display());
    let designator = format!("designator={designator}");
    let answered = answer(app, "query_pins", &[&document, &designator]);
    answered["pins"].as_array().expect("pins").clone()
}

fn refusal(app: &mut Signex, operands: &[&str]) -> String {
    let command = AppCommandId::new("query_pins").unwrap();
    let args = CommandArgs::parse_operands(&command, operands).expect("operands parse");
    match app.run_query(&command, &args).expect("a query").0 {
        CommandOutcome::Disabled { reason } => reason,
        other => panic!("was not refused: {other:?}"),
    }
}

fn tip(pin: &Value) -> (f64, f64) {
    let at = pin["position"].as_array().expect("position");
    (at[0].as_f64().unwrap(), at[1].as_f64().unwrap())
}

fn assert_tip(pin: &Value, x: f64, y: f64) {
    let (got_x, got_y) = tip(pin);
    assert!(
        (got_x - x).abs() < 1e-9 && (got_y - y).abs() < 1e-9,
        "expected ({x}, {y}), got ({got_x}, {got_y}) for {pin}"
    );
}

#[test]
fn a_rotated_resistor_lists_its_tips_where_the_canvas_hit_test_finds_them() {
    let mut app = app();
    let pins = pins_of(&mut app, "R1");
    assert_eq!(pins.len(), 2);
    assert_eq!(pins[0]["number"], "1");
    assert_eq!(pins[0]["name"], "N1");
    assert_eq!(pins[0]["electrical_type"], "passive");
    assert_eq!(pins[0]["hidden"], false);
    assert_eq!(pins[0]["unit"], 1);
    assert_tip(&pins[0], 50.0, 52.54);
    assert_tip(&pins[1], 50.0, 47.46);
}

#[test]
fn a_mirrored_resistor_lists_mirrored_tips() {
    let mut app = app();
    let pins = pins_of(&mut app, "r2");
    assert_tip(&pins[0], 12.54, 10.0);
    assert_tip(&pins[1], 7.46, 10.0);
}

#[test]
fn a_multi_unit_part_lists_the_placed_units_only_with_the_shared_pin_once() {
    let mut app = app();
    let pins = pins_of(&mut app, "U1");
    let listed: Vec<(String, u64)> = pins
        .iter()
        .map(|p| {
            (
                p["number"].as_str().unwrap().to_string(),
                p["unit"].as_u64().unwrap(),
            )
        })
        .collect();
    // Unit 2 is not placed: its pin is not listed.
    assert_eq!(listed, [("1".into(), 1), ("8".into(), 1), ("3".into(), 3)]);
    let hidden = pins.iter().find(|p| p["number"] == "8").unwrap();
    assert_eq!(hidden["hidden"], true);
    assert_tip(&pins[2], 123.0, 100.0);
}

#[test]
fn what_cannot_be_named_is_refused_with_the_reason() {
    let mut app = app();
    let document = format!("document={}", path("pins.snxsch").display());
    let on = |designator: &str| [document.clone(), format!("designator={designator}")];
    for (designator, fragment) in [
        ("C9", "no part `C9`"),
        ("U2", "ambiguous"),
        ("U3", "ambiguous"),
        ("R?", "not annotated"),
    ] {
        let operands = on(designator);
        let reason = refusal(&mut app, &[&operands[0], &operands[1]]);
        assert!(reason.contains(fragment), "{designator}: {reason}");
    }
    let missing = format!("document={}", path("nope.snxsch").display());
    let reason = refusal(&mut app, &[&missing, "designator=R1"]);
    assert!(reason.contains("not an open or referenced"), "{reason}");
}
