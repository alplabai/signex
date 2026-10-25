use super::*;
use crate::schematic::{LibGraphic, Pin, PinDirection, PinShapeStyle};

fn lib(graphics: Vec<Graphic>, pins: Vec<Pin>) -> LibSymbol {
    let mut lib: LibSymbol = serde_json::from_value(serde_json::json!({ "id": "L:S" })).unwrap();
    lib.graphics = graphics
        .into_iter()
        .map(|graphic| LibGraphic {
            unit: 0,
            body_style: 1,
            graphic,
        })
        .collect();
    lib.pins = pins
        .into_iter()
        .map(|pin| LibPin {
            unit: 0,
            body_style: 1,
            pin,
        })
        .collect();
    lib
}

fn rect(w: f64, h: f64) -> Graphic {
    Graphic::Rectangle {
        start: Point::new(-w / 2.0, -h / 2.0),
        end: Point::new(w / 2.0, h / 2.0),
        width: 0.0,
        fill: FillType::None,
    }
}

fn pin(x: f64, y: f64, rotation: f64, length: f64) -> Pin {
    Pin {
        direction: PinDirection::Passive,
        shape_style: PinShapeStyle::Plain,
        position: Point::new(x, y),
        rotation,
        length,
        name: String::new(),
        number: "1".into(),
        visible: true,
        name_visible: true,
        number_visible: true,
    }
}

fn placed(x: f64, y: f64, rotation: f64) -> Symbol {
    Symbol {
        position: Point::new(x, y),
        rotation,
        ..Symbol::default()
    }
}

fn bounds(lib: &LibSymbol, sym: &Symbol) -> Aabb {
    primitives_bounds(&lower_symbol(lib, sym)).expect("drawable")
}

fn assert_box(b: Aabb, min_x: f64, min_y: f64, max_x: f64, max_y: f64) {
    let ok = [
        (b.min_x, min_x),
        (b.min_y, min_y),
        (b.max_x, max_x),
        (b.max_y, max_y),
    ]
    .iter()
    .all(|(got, want)| (got - want).abs() < 1e-9);
    assert!(
        ok,
        "expected ({min_x}, {min_y})..({max_x}, {max_y}), got {b:?}"
    );
}

#[test]
fn large_and_small_symbols_get_their_own_boxes() {
    let ic = lib(vec![rect(20.0, 40.0)], vec![]);
    let passive = lib(vec![rect(2.0, 1.0)], vec![]);
    let sym = placed(100.0, 50.0, 0.0);
    assert_box(bounds(&ic, &sym), 90.0, 30.0, 110.0, 70.0);
    assert_box(bounds(&passive, &sym), 99.0, 49.5, 101.0, 50.5);
}

#[test]
fn pins_extend_the_box_by_their_length() {
    // Pin tip at x = -10 pointing right (into the body) for 5 mm.
    let lib = lib(vec![rect(10.0, 4.0)], vec![pin(-10.0, 0.0, 0.0, 5.0)]);
    assert_box(bounds(&lib, &placed(0.0, 0.0, 0.0)), -10.0, -2.0, 5.0, 2.0);
}

#[test]
fn rotation_turns_the_box() {
    let tall = lib(vec![rect(4.0, 20.0)], vec![]);
    assert_box(
        bounds(&tall, &placed(0.0, 0.0, 90.0)),
        -10.0,
        -2.0,
        10.0,
        2.0,
    );
}

#[test]
fn other_units_and_de_morgan_graphics_are_skipped() {
    let mut l = lib(vec![rect(2.0, 2.0)], vec![]);
    l.graphics.push(LibGraphic {
        unit: 2,
        body_style: 1,
        graphic: rect(50.0, 50.0),
    });
    l.graphics.push(LibGraphic {
        unit: 0,
        body_style: 2,
        graphic: rect(60.0, 60.0),
    });
    assert_box(bounds(&l, &placed(0.0, 0.0, 0.0)), -1.0, -1.0, 1.0, 1.0);
}

#[test]
fn arc_box_reaches_the_bulge() {
    // Upper half circle of radius 5 through (0, 5): Y-up library space, so
    // the bulge lands at world y = -5.
    let arc = Graphic::Arc {
        start: Point::new(-5.0, 0.0),
        mid: Point::new(0.0, 5.0),
        end: Point::new(5.0, 0.0),
        width: 0.0,
        fill: FillType::None,
    };
    assert_box(
        bounds(&lib(vec![arc], vec![]), &placed(0.0, 0.0, 0.0)),
        -5.0,
        -5.0,
        5.0,
        0.0,
    );
}

#[test]
fn empty_symbol_has_no_bounds() {
    let l = lib(vec![], vec![]);
    assert!(primitives_bounds(&lower_symbol(&l, &placed(0.0, 0.0, 0.0))).is_none());
}
