//! Which body shapes take the default body fill.

use super::*;
use crate::schematic::{HAlign, LibGraphic, VAlign};

fn graphic(graphic: Graphic) -> LibGraphic {
    LibGraphic {
        unit: 0,
        body_style: 1,
        graphic,
    }
}

fn rect(fill: FillType) -> Graphic {
    Graphic::Rectangle {
        start: Point::new(0.0, 0.0),
        end: Point::new(10.0, 4.0),
        width: 0.25,
        fill,
    }
}

fn lib_of(graphics: Vec<Graphic>) -> LibSymbol {
    let mut lib: LibSymbol = serde_json::from_value(serde_json::json!({ "id": "L:S" })).unwrap();
    lib.graphics = graphics.into_iter().map(graphic).collect();
    lib
}

fn placed() -> Symbol {
    Symbol {
        lib_id: "L:S".into(),
        position: Point::new(100.0, 100.0),
        ..Symbol::default()
    }
}

#[test]
fn an_unfilled_body_rectangle_takes_the_default_fill_in_world_space() {
    let shapes = default_body_fill_shapes(Some(&lib_of(vec![rect(FillType::None)])), &placed());
    // Y-up library y 0..4 is world y 100..96 (Y-down).
    assert_eq!(
        shapes,
        vec![BodyFillShape::Polygon(vec![
            Point::new(100.0, 100.0),
            Point::new(110.0, 100.0),
            Point::new(110.0, 96.0),
            Point::new(100.0, 96.0),
        ])]
    );
}

#[test]
fn a_shape_with_its_own_fill_keeps_it() {
    let lib = lib_of(vec![rect(FillType::Background), rect(FillType::Outline)]);
    assert!(default_body_fill_shapes(Some(&lib), &placed()).is_empty());
}

#[test]
fn closed_polylines_and_circles_take_it_but_open_shapes_and_text_do_not() {
    let square = vec![
        Point::new(0.0, 0.0),
        Point::new(2.0, 0.0),
        Point::new(2.0, 2.0),
        Point::new(0.0, 0.0),
    ];
    let lib = lib_of(vec![
        Graphic::Polyline {
            points: square.clone(),
            width: 0.0,
            fill: FillType::None,
        },
        Graphic::Polyline {
            points: square[..3].to_vec(),
            width: 0.0,
            fill: FillType::None,
        },
        Graphic::Circle {
            center: Point::new(5.0, 5.0),
            radius: 1.0,
            width: 0.0,
            fill: FillType::None,
        },
        Graphic::Arc {
            start: Point::new(0.0, 0.0),
            mid: Point::new(1.0, 1.0),
            end: Point::new(2.0, 0.0),
            width: 0.0,
            fill: FillType::None,
        },
        Graphic::Text {
            text: "U".into(),
            position: Point::new(0.0, 0.0),
            rotation: 0.0,
            font_size: 1.27,
            justify_h: HAlign::Left,
            justify_v: VAlign::Bottom,
            bold: false,
            italic: false,
        },
    ]);
    let shapes = default_body_fill_shapes(Some(&lib), &placed());
    assert_eq!(shapes.len(), 2, "{shapes:?}");
    assert!(matches!(shapes[0], BodyFillShape::Polygon(ref p) if p.len() == 4));
    assert_eq!(
        shapes[1],
        BodyFillShape::Circle {
            center: Point::new(105.0, 95.0),
            radius: 1.0,
        }
    );
}

#[test]
fn a_power_port_or_a_missing_library_takes_no_fill() {
    let lib = lib_of(vec![rect(FillType::None)]);
    let port = Symbol {
        is_power: true,
        ..placed()
    };
    assert!(default_body_fill_shapes(Some(&lib), &port).is_empty());
    assert!(default_body_fill_shapes(None, &placed()).is_empty());
}
