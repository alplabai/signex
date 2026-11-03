use super::*;
use crate::schematic::LibPin;

fn lib_with_pin(rotation: f64) -> LibSymbol {
    let mut lib: LibSymbol =
        serde_json::from_value(serde_json::json!({ "id": "L:P" })).expect("lib symbol");
    lib.pins.push(LibPin {
        unit: 0,
        body_style: crate::schematic::BODY_NORMAL,
        pin: serde_json::from_value(serde_json::json!({
            "direction": "passive",
            "shape_style": "plain",
            "position": { "x": -5.0, "y": 0.0 },
            "rotation": rotation,
            "length": 2.54,
            "name": "OUT",
            "number": "3",
        }))
        .expect("pin"),
    });
    lib
}

fn placed() -> Symbol {
    Symbol {
        unit: 1,
        body_style: crate::schematic::BodyStyle::Normal,
        ..Symbol::default()
    }
}

#[test]
fn a_pin_has_its_number_beside_the_line_and_its_name_inside() {
    let labels = lower_pin_labels(&lib_with_pin(0.0), &placed());
    let name = labels
        .iter()
        .find(|l| l.kind == PinLabelKind::Name)
        .unwrap();
    let number = labels
        .iter()
        .find(|l| l.kind == PinLabelKind::Number)
        .unwrap();
    // The pin runs from x = -5 to the body end at x = -2.46.
    assert!(name.position.x > -2.46, "name is past the body end");
    assert_eq!(name.h_align, HAlign::Left);
    assert!((number.position.x - -3.73).abs() < 1e-9, "mid of the line");
    assert!(number.position.y < 0.0, "number sits above the line");
}

#[test]
fn per_pin_and_symbol_flags_hide_the_text() {
    let mut lib = lib_with_pin(0.0);
    lib.pins[0].pin.name_visible = false;
    let labels = lower_pin_labels(&lib, &placed());
    assert!(labels.iter().all(|l| l.kind == PinLabelKind::Number));

    lib.show_pin_numbers = false;
    assert!(lower_pin_labels(&lib, &placed()).is_empty());
}
