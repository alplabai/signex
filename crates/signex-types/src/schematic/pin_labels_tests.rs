use super::*;
use crate::schematic::{BodyStyle, LibPin};

const LEN: f64 = 2.54;
const ORIGIN: (f64, f64) = (10.0, 20.0);

fn pin_json(rotation: f64, name: &str) -> serde_json::Value {
    serde_json::json!({
        "direction": "passive",
        "shape_style": "plain",
        "position": { "x": 0.0, "y": 0.0 },
        "rotation": rotation,
        "length": LEN,
        "name": name,
        "number": "3",
    })
}

fn lib_with_pin(rotation: f64, name: &str) -> LibSymbol {
    let mut lib: LibSymbol =
        serde_json::from_value(serde_json::json!({ "id": "L:P" })).expect("lib symbol");
    lib.pins.push(LibPin {
        unit: 0,
        body_style: crate::schematic::BODY_NORMAL,
        pin: serde_json::from_value(pin_json(rotation, name)).expect("pin"),
    });
    lib
}

fn placed(rotation: f64, mirror_x: bool, mirror_y: bool) -> Symbol {
    Symbol {
        unit: 1,
        body_style: BodyStyle::Normal,
        position: Point::new(ORIGIN.0, ORIGIN.1),
        rotation,
        mirror_x,
        mirror_y,
        ..Symbol::default()
    }
}

fn near(a: Point, x: f64, y: f64) -> bool {
    (a.x - x).abs() < 1e-9 && (a.y - y).abs() < 1e-9
}

/// `(pin rotation, symbol rotation, mirror_x, mirror_y)` -> the world
/// direction tip -> body end (Y-down), the text angle in degrees, the name's
/// alignment and the screen direction the text's top faces (number side).
type Case = ((f64, f64, bool, bool), (f64, f64), f64, HAlign, (f64, f64));

const UP: (f64, f64) = (0.0, -1.0);
const LEFT: (f64, f64) = (-1.0, 0.0);
const RIGHT: (f64, f64) = (1.0, 0.0);

fn cases() -> Vec<Case> {
    vec![
        // Every pin direction on an unturned symbol.
        ((0.0, 0.0, false, false), (1.0, 0.0), 0.0, HAlign::Left, UP),
        (
            (90.0, 0.0, false, false),
            (0.0, -1.0),
            -90.0,
            HAlign::Left,
            LEFT,
        ),
        (
            (180.0, 0.0, false, false),
            (-1.0, 0.0),
            0.0,
            HAlign::Right,
            UP,
        ),
        (
            (270.0, 0.0, false, false),
            (0.0, 1.0),
            90.0,
            HAlign::Left,
            RIGHT,
        ),
        // Symbol turns, pin pointing right in the library.
        (
            (0.0, 90.0, false, false),
            (0.0, -1.0),
            -90.0,
            HAlign::Left,
            LEFT,
        ),
        (
            (0.0, 180.0, false, false),
            (-1.0, 0.0),
            0.0,
            HAlign::Right,
            UP,
        ),
        (
            (0.0, 270.0, false, false),
            (0.0, 1.0),
            90.0,
            HAlign::Left,
            RIGHT,
        ),
        // Mirrors flip the direction, never the glyphs.
        ((0.0, 0.0, false, true), (-1.0, 0.0), 0.0, HAlign::Right, UP),
        (
            (90.0, 0.0, true, false),
            (0.0, 1.0),
            90.0,
            HAlign::Left,
            RIGHT,
        ),
        (
            (90.0, 0.0, false, true),
            (0.0, -1.0),
            -90.0,
            HAlign::Left,
            LEFT,
        ),
    ]
}

#[test]
fn text_follows_the_pins_world_direction() {
    for ((pin_rot, sym_rot, mx, my), dir, rot_deg, name_align, up) in cases() {
        let lib = lib_with_pin(pin_rot, "OUT");
        let labels = lower_pin_labels(&lib, &placed(sym_rot, mx, my));
        let case = format!("pin {pin_rot} sym {sym_rot} mx {mx} my {my}");
        assert_eq!(labels.len(), 2, "{case}");
        let name = labels
            .iter()
            .find(|l| l.kind == PinLabelKind::Name)
            .unwrap();
        let number = labels
            .iter()
            .find(|l| l.kind == PinLabelKind::Number)
            .unwrap();
        let reach = LEN + lib.name_offset_mm();
        assert!(
            near(
                name.position,
                ORIGIN.0 + dir.0 * reach,
                ORIGIN.1 + dir.1 * reach
            ),
            "{case}: name at {:?}",
            name.position
        );
        assert_eq!(name.h_align, name_align, "{case}");
        assert_eq!(name.v_align, VAlign::Center, "{case}");
        let along = LEN * PIN_NUMBER_ALONG_RATIO;
        assert!(
            near(
                number.position,
                ORIGIN.0 + dir.0 * along + up.0 * PIN_NUMBER_OFFSET_MM,
                ORIGIN.1 + dir.1 * along + up.1 * PIN_NUMBER_OFFSET_MM
            ),
            "{case}: number at {:?}",
            number.position
        );
        assert_eq!(number.h_align, HAlign::Center, "{case}");
        assert_eq!(number.v_align, VAlign::Bottom, "{case}");
        for label in [name, number] {
            let got = label.rotation_rad.to_degrees();
            assert!((got - rot_deg).abs() < 1e-9, "{case}: rotation {got}");
        }
    }
}

#[test]
fn text_is_never_upside_down() {
    for pin_rot in [0.0, 45.0, 90.0, 135.0, 180.0, 225.0, 270.0, 315.0] {
        for sym_rot in [0.0, 90.0, 180.0, 270.0] {
            let labels =
                lower_pin_labels(&lib_with_pin(pin_rot, "A"), &placed(sym_rot, false, false));
            for label in labels {
                let r = label.rotation_rad;
                assert!(
                    (-FRAC_PI_2 - 1e-9..=FRAC_PI_2 + 1e-9).contains(&r),
                    "pin {pin_rot} sym {sym_rot}: {r}"
                );
            }
        }
    }
}

#[test]
fn a_tilde_name_draws_only_the_number() {
    let labels = lower_pin_labels(&lib_with_pin(0.0, "~"), &placed(0.0, false, false));
    assert_eq!(labels.len(), 1);
    assert_eq!(labels[0].kind, PinLabelKind::Number);
}

#[test]
fn a_hidden_pin_draws_nothing() {
    let mut lib = lib_with_pin(0.0, "OUT");
    lib.pins[0].pin.visible = false;
    assert!(lower_pin_labels(&lib, &placed(0.0, false, false)).is_empty());
}

#[test]
fn per_pin_flags_hide_one_text_each() {
    let mut lib = lib_with_pin(0.0, "OUT");
    lib.pins[0].pin.name_visible = false;
    let labels = lower_pin_labels(&lib, &placed(0.0, false, false));
    assert!(labels.iter().all(|l| l.kind == PinLabelKind::Number));
    assert_eq!(labels.len(), 1);

    lib.pins[0].pin.name_visible = true;
    lib.pins[0].pin.number_visible = false;
    let labels = lower_pin_labels(&lib, &placed(0.0, false, false));
    assert!(labels.iter().all(|l| l.kind == PinLabelKind::Name));
    assert_eq!(labels.len(), 1);
}

#[test]
fn show_pin_names_off_keeps_the_numbers() {
    let mut lib = lib_with_pin(0.0, "OUT");
    lib.show_pin_names = false;
    let labels = lower_pin_labels(&lib, &placed(0.0, false, false));
    assert_eq!(labels.len(), 1);
    assert_eq!(labels[0].kind, PinLabelKind::Number);

    lib.show_pin_names = true;
    lib.show_pin_numbers = false;
    let labels = lower_pin_labels(&lib, &placed(0.0, false, false));
    assert_eq!(labels.len(), 1);
    assert_eq!(labels[0].kind, PinLabelKind::Name);
}

#[test]
fn a_power_port_has_no_pin_text() {
    let lib = lib_with_pin(0.0, "OUT");
    let mut sym = placed(0.0, false, false);
    sym.is_power = true;
    assert!(lower_pin_labels(&lib, &sym).is_empty());
}
