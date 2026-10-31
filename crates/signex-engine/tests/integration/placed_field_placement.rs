//! A part placed from a library starts with its reference and value clear of
//! its body (#1798). The fixed offset it used to get from the origin landed
//! inside any body taller than the offset.

use signex_engine::place_fields_for_new_symbol;
use signex_types::schematic::{
    FillType, Graphic, HAlign, LibGraphic, LibSymbol, Point, SCHEMATIC_TEXT_MM, Symbol, TextProp,
    VAlign,
};

fn block(half_w: f64, half_h: f64) -> LibSymbol {
    let mut lib: LibSymbol =
        serde_json::from_value(serde_json::json!({ "id": "Test:Block" })).expect("lib symbol");
    lib.graphics.push(LibGraphic {
        unit: 0,
        body_style: 1,
        graphic: Graphic::Rectangle {
            start: Point::new(-half_w, -half_h),
            end: Point::new(half_w, half_h),
            width: 0.0,
            fill: FillType::None,
        },
    });
    lib
}

fn field(dy: f64) -> TextProp {
    TextProp {
        position: Point::new(0.0, dy),
        rotation: 0.0,
        font_size: SCHEMATIC_TEXT_MM,
        justify_h: HAlign::Center,
        justify_v: VAlign::default(),
        hidden: false,
        font_family: None,
    }
}

#[test]
fn fields_of_a_placed_part_sit_outside_a_tall_body() {
    let lib = block(7.62, 5.08);
    let mut sym = Symbol {
        lib_id: lib.id.clone(),
        unit: 1,
        ref_text: Some(field(-2.54)),
        val_text: Some(field(2.54)),
        fields_autoplaced: true,
        ..Default::default()
    };
    place_fields_for_new_symbol(&mut sym, &lib);
    for (name, prop) in [("ref", &sym.ref_text), ("value", &sym.val_text)] {
        let at = prop.as_ref().expect(name).position;
        assert!(
            at.x.abs() > 7.62 || at.y.abs() > 5.08,
            "{name} anchor {at:?} is inside the body"
        );
    }
}

#[test]
fn a_part_the_user_placed_fields_of_keeps_them() {
    let lib = block(7.62, 5.08);
    let mut sym = Symbol {
        lib_id: lib.id.clone(),
        unit: 1,
        ref_text: Some(field(-2.54)),
        val_text: Some(field(2.54)),
        fields_user_placed: true,
        ..Default::default()
    };
    place_fields_for_new_symbol(&mut sym, &lib);
    assert_eq!(sym.ref_text.expect("ref").position, Point::new(0.0, -2.54));
}
