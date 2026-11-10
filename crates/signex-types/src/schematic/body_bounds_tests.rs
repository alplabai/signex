//! #2199 — the body box a headless wire must not cross, and the crossing test.

use super::*;
use crate::schematic::{LibGraphic, Pin, PinDirection, PinShapeStyle};

/// A body x 0..10, y 0..4 in Y-up library space, with a pin and a text that
/// are not part of the body.
fn body_lib() -> LibSymbol {
    let mut lib: LibSymbol = serde_json::from_value(serde_json::json!({ "id": "L:S" })).unwrap();
    lib.graphics = vec![
        LibGraphic {
            unit: 0,
            body_style: 1,
            graphic: Graphic::Rectangle {
                start: Point::new(0.0, 0.0),
                end: Point::new(10.0, 4.0),
                width: 0.5,
                fill: FillType::None,
            },
        },
        LibGraphic {
            unit: 0,
            body_style: 1,
            graphic: Graphic::Text {
                text: "far away".into(),
                position: Point::new(100.0, 100.0),
                rotation: 0.0,
                font_size: 1.27,
                justify_h: HAlign::Left,
                justify_v: VAlign::Bottom,
                bold: false,
                italic: false,
            },
        },
    ];
    lib.pins = vec![LibPin {
        unit: 0,
        body_style: 1,
        pin: Pin {
            direction: PinDirection::Passive,
            shape_style: PinShapeStyle::Plain,
            position: Point::new(-50.0, 0.0),
            rotation: 0.0,
            length: 2.54,
            name: String::new(),
            number: "1".into(),
            visible: true,
            name_visible: true,
            number_visible: true,
        },
    }];
    lib
}

fn placed(rotation: f64, mirror_x: bool, mirror_y: bool) -> Symbol {
    Symbol {
        position: Point::new(100.0, 100.0),
        rotation,
        mirror_x,
        mirror_y,
        ..Symbol::default()
    }
}

fn near(a: &Aabb, x0: f64, y0: f64, x1: f64, y1: f64) -> bool {
    let close = |p: f64, q: f64| (p - q).abs() < 1e-9;
    close(a.min_x, x0) && close(a.min_y, y0) && close(a.max_x, x1) && close(a.max_y, y1)
}

#[test]
fn the_body_box_has_no_pins_text_or_stroke() {
    // Y-up library y 0..4 is world y 0..-4 below the origin (Y-down).
    let b = body_bounds(&body_lib(), &placed(0.0, false, false)).expect("a body");
    assert!(near(&b, 100.0, 96.0, 110.0, 100.0), "{b:?}");
}

#[test]
fn the_body_box_follows_rotation_and_mirror() {
    let lib = body_lib();
    let turned = body_bounds(&lib, &placed(90.0, false, false)).expect("a body");
    assert!(near(&turned, 96.0, 90.0, 100.0, 100.0), "{turned:?}");
    let mx = body_bounds(&lib, &placed(0.0, true, false)).expect("a body");
    assert!(near(&mx, 100.0, 100.0, 110.0, 104.0), "{mx:?}");
    let my = body_bounds(&lib, &placed(0.0, false, true)).expect("a body");
    assert!(near(&my, 90.0, 96.0, 100.0, 100.0), "{my:?}");
}

#[test]
fn a_symbol_without_body_graphics_has_no_box() {
    let mut lib = body_lib();
    lib.graphics.clear();
    assert!(body_bounds(&lib, &placed(0.0, false, false)).is_none());
}

#[test]
fn only_the_open_interior_is_crossed() {
    let b = Aabb::new(0.0, 0.0, 10.0, 4.0);
    let p = Point::new;
    // Through, ending inside, and wholly inside cross.
    assert!(segment_crosses_interior(&b, p(-5.0, 2.0), p(15.0, 2.0)));
    assert!(segment_crosses_interior(&b, p(5.0, -5.0), p(5.0, 2.0)));
    assert!(segment_crosses_interior(&b, p(2.0, 2.0), p(3.0, 2.0)));
    // Along an edge, ending on an edge or a corner, or outside do not.
    assert!(!segment_crosses_interior(&b, p(-5.0, 0.0), p(15.0, 0.0)));
    assert!(!segment_crosses_interior(&b, p(10.0, -5.0), p(10.0, 9.0)));
    assert!(!segment_crosses_interior(&b, p(-5.0, 2.0), p(0.0, 2.0)));
    assert!(!segment_crosses_interior(&b, p(5.0, -5.0), p(5.0, 0.0)));
    assert!(!segment_crosses_interior(&b, p(-5.0, -5.0), p(0.0, 0.0)));
    assert!(!segment_crosses_interior(&b, p(-5.0, 9.0), p(15.0, 9.0)));
}

#[test]
fn a_wire_across_a_rotated_body_crosses_it_and_one_past_its_edge_does_not() {
    let mut sheet = crate::schematic::SchematicSheet::default();
    let lib = body_lib();
    sheet.lib_symbols.insert("L:S".into(), lib);
    let mut sym = placed(90.0, false, false);
    sym.lib_id = "L:S".into();
    sheet.symbols.push(sym);
    // Turned 90 degrees the body is x 96..100, y 90..100.
    let p = Point::new;
    assert!(symbol_crossed_by(&sheet, p(90.0, 95.0), p(110.0, 95.0)).is_some());
    assert!(symbol_crossed_by(&sheet, p(100.0, 85.0), p(100.0, 105.0)).is_none());
    assert!(symbol_crossed_by(&sheet, p(94.0, 85.0), p(94.0, 105.0)).is_none());
}
