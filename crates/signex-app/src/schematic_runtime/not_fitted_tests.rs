//! #1294 — a not-fitted part on the canvas: greyed, under a cross over its
//! body, in the same rule the PDF and SVG export draw with.

use std::collections::HashSet;

use iced::Rectangle;
use signex_types::schematic::{FillType, Graphic, LibGraphic, LibSymbol, Point, SchematicSheet};
use signex_types::theme::{ThemeId, canvas_colors};

use super::*;

/// One part whose body is a 10 x 4 mm rectangle with no pins.
fn body_sheet() -> (SchematicSheet, uuid::Uuid) {
    let mut lib: LibSymbol =
        serde_json::from_value(serde_json::json!({ "id": "L:Body" })).expect("lib symbol");
    lib.graphics.push(LibGraphic {
        unit: 0,
        body_style: 1,
        graphic: Graphic::Rectangle {
            start: Point::new(0.0, 0.0),
            end: Point::new(10.0, 4.0),
            width: 0.0,
            fill: FillType::None,
        },
    });
    let id = uuid::Uuid::now_v7();
    let mut sheet = SchematicSheet::default();
    sheet.symbols.push(Symbol {
        uuid: id,
        lib_id: lib.id.clone(),
        reference: "U1".into(),
        position: Point::new(20.0, 20.0),
        ..Symbol::default()
    });
    sheet.lib_symbols.insert(lib.id.clone(), lib);
    (sheet, id)
}

fn drawn(sheet: &SchematicSheet, not_fitted: Option<&HashSet<uuid::Uuid>>) -> RendererSnapshot {
    let transform = ScreenTransform {
        offset_x: 0.0,
        offset_y: 0.0,
        scale: 4.0,
        device_px_per_logical_px: signex_types::schematic::UNSCALED_DEVICE_PX_PER_LOGICAL_PX,
    };
    build_renderer_snapshot(
        sheet,
        &transform,
        &canvas_colors(ThemeId::GraphiteLight),
        Rectangle::new(iced::Point::ORIGIN, iced::Size::new(2000.0, 2000.0)),
        None,
        SceneOverrides {
            not_fitted,
            ..SceneOverrides::default()
        },
    )
}

/// The stroke colour of the body outline.
fn outline(snap: &RendererSnapshot) -> [f32; 4] {
    snap.polygons
        .iter()
        .find_map(|p| p.stroke_color)
        .expect("the body outline")
}

#[test]
fn a_not_fitted_part_draws_greyed_under_a_cross_and_others_as_before() {
    let (sheet, id) = body_sheet();
    let colors = canvas_colors(ThemeId::GraphiteLight);
    let ink = to_rgba(to_iced(&colors.body));

    let plain = drawn(&sheet, Some(&HashSet::new()));
    assert_eq!(outline(&plain), ink);
    assert!(plain.wires.is_empty(), "no pins, no cross");

    let crossed = drawn(&sheet, Some(&HashSet::from([id])));
    let grey = to_rgba(symbol::not_fitted_ink(
        to_iced(&colors.body),
        to_iced(&colors.paper),
    ));
    assert_eq!(outline(&crossed), grey);
    let lib = &sheet.lib_symbols["L:Body"];
    let [a, b] = signex_types::schematic::not_fitted_cross(lib, &sheet.symbols[0]).expect("a body");
    let strokes: Vec<([f32; 2], [f32; 2])> = crossed
        .wires
        .iter()
        .inspect(|w| assert_eq!(w.explicit_color, Some(ink), "the cross is not greyed"))
        .map(|w| (w.p0, w.p1))
        .collect();
    let point = |p: Point| [p.x as f32, p.y as f32];
    assert_eq!(
        strokes,
        vec![(point(a.0), point(a.1)), (point(b.0), point(b.1))]
    );
}

#[test]
fn no_set_draws_every_part_as_fitted() {
    let (sheet, _) = body_sheet();
    let snap = drawn(&sheet, None);
    assert!(snap.wires.is_empty());
}
