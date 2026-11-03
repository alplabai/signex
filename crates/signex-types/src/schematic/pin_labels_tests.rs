use super::*;
use crate::schematic::{LibPin, Pin, PinDirection, PinShapeStyle};

fn lib_with_pins(pins: Vec<LibPin>) -> LibSymbol {
    let mut lib: LibSymbol =
        serde_json::from_value(serde_json::json!({ "id": "L:Ic" })).expect("lib symbol");
    lib.pins = pins;
    lib
}

fn pin(number: &str, name: &str, x: f64, y: f64, rotation: f64) -> LibPin {
    LibPin {
        unit: 0,
        body_style: 1,
        pin: Pin {
            direction: PinDirection::Passive,
            shape_style: PinShapeStyle::Plain,
            position: Point::new(x, y),
            rotation,
            length: 2.54,
            name: name.to_owned(),
            number: number.to_owned(),
            visible: true,
            name_visible: true,
            number_visible: true,
        },
    }
}

fn placed() -> Symbol {
    Symbol {
        lib_id: "L:Ic".to_owned(),
        position: Point::new(100.0, 100.0),
        ..Symbol::default()
    }
}

fn texts(labels: &[PinLabel]) -> Vec<(&str, f64, f64, HAlign)> {
    labels
        .iter()
        .map(|l| (l.text.as_str(), l.position.x, l.position.y, l.h_align))
        .collect()
}

#[test]
fn left_pin_shows_its_number_over_the_line_and_its_name_inside_the_body() {
    let lib = lib_with_pins(vec![pin("1", "IN", -7.62, 0.0, 0.0)]);
    let labels = lower_pin_labels(&lib, &placed());
    let got = texts(&labels);
    assert_eq!(got.len(), 2, "{got:?}");
    let number = got.iter().find(|t| t.0 == "1").expect("number");
    let name = got.iter().find(|t| t.0 == "IN").expect("name");
    // Pin spans x 92.38..94.92 on the sheet; the number is centred over it.
    assert!(
        (number.1 - 93.65).abs() < 1e-6 && number.2 < 100.0,
        "{number:?}"
    );
    // The name starts past the body end, reading rightwards into the body.
    assert!(name.1 > 94.92 && name.3 == HAlign::Left, "{name:?}");
}

#[test]
fn right_pin_name_reads_leftwards_into_the_body() {
    let lib = lib_with_pins(vec![pin("8", "OUT", 7.62, 0.0, 180.0)]);
    let labels = lower_pin_labels(&lib, &placed());
    let got = texts(&labels);
    let name = got.iter().find(|t| t.0 == "OUT").expect("name");
    assert!(name.1 < 105.08 && name.3 == HAlign::Right, "{name:?}");
}

#[test]
fn symbol_and_pin_flags_hide_the_text() {
    let mut lib = lib_with_pins(vec![pin("1", "IN", -7.62, 0.0, 0.0)]);
    lib.show_pin_names = false;
    let got = texts(&lower_pin_labels(&lib, &placed())).len();
    assert_eq!(got, 1, "only the number remains");
    lib.show_pin_numbers = false;
    assert!(lower_pin_labels(&lib, &placed()).is_empty());

    let mut lib = lib_with_pins(vec![pin("1", "IN", -7.62, 0.0, 0.0)]);
    lib.pins[0].pin.number_visible = false;
    lib.pins[0].pin.name_visible = false;
    assert!(lower_pin_labels(&lib, &placed()).is_empty());
    lib.pins[0].pin.name_visible = true;
    lib.pins[0].pin.name = "~".to_owned();
    assert!(lower_pin_labels(&lib, &placed()).is_empty());
}

#[test]
fn hidden_pin_has_no_text() {
    let mut lib = lib_with_pins(vec![pin("1", "IN", -7.62, 0.0, 0.0)]);
    lib.pins[0].pin.visible = false;
    assert!(lower_pin_labels(&lib, &placed()).is_empty());
}

#[test]
fn mirrored_symbol_flips_the_name_side() {
    let lib = lib_with_pins(vec![pin("1", "IN", -7.62, 0.0, 0.0)]);
    let mut sym = placed();
    sym.mirror_y = true;
    let labels = lower_pin_labels(&lib, &sym);
    let got = texts(&labels);
    let name = got.iter().find(|t| t.0 == "IN").expect("name");
    assert_eq!(name.3, HAlign::Right, "{name:?}");
}

#[test]
fn rotated_symbol_turns_text_vertical_with_number_left_of_the_line() {
    // Pin at the symbol's left, body end 2.54 mm to its right; rotating the
    // symbol 90 degrees stands it on end.
    let lib = lib_with_pins(vec![pin("1", "IN", -7.62, 0.0, 0.0)]);
    let mut sym = placed();
    sym.rotation = 90.0;
    let labels = lower_pin_labels(&lib, &sym);
    let number = labels.iter().find(|l| l.text == "1").expect("number");
    let name = labels.iter().find(|l| l.text == "IN").expect("name");
    let line_x = 100.0;
    assert!(
        number.position.x < line_x && number.rotation_deg == 90.0,
        "{number:?}"
    );
    assert_eq!(number.v_align, VAlign::Bottom, "{number:?}");
    // The name runs along the vertical pin axis, past the body end.
    assert!((name.position.x - line_x).abs() < 1e-6, "{name:?}");
    assert_eq!(name.rotation_deg, 90.0, "{name:?}");
    // Tip at y 107.62, body end at 105.08: the name starts just past the
    // body end and reads upward into the body.
    assert!(
        name.position.y < 105.08 && name.h_align == HAlign::Left,
        "{name:?}"
    );
    assert!((number.position.y - 106.35).abs() < 1e-6, "{number:?}");
}

#[test]
fn horizontal_pin_text_stays_upright() {
    let lib = lib_with_pins(vec![pin("1", "IN", -7.62, 0.0, 0.0)]);
    assert!(
        lower_pin_labels(&lib, &placed())
            .iter()
            .all(|l| l.rotation_deg == 0.0)
    );
}
