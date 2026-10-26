use super::*;
use signex_types::schematic::{Graphic, HAlign, LibGraphic, VAlign};

fn text_graphic(text: &str, unit_hint: f64) -> Graphic {
    Graphic::Text {
        text: text.into(),
        position: Point::new(0.0, 0.0),
        rotation: unit_hint,
        font_size: 1.27,
        justify_h: HAlign::Left,
        justify_v: VAlign::Top,
        bold: false,
        italic: false,
    }
}

fn lib_text() -> LibSymbol {
    let mut lib: LibSymbol =
        serde_json::from_value(serde_json::json!({ "id": "L:S" })).expect("lib symbol");
    lib.graphics.push(LibGraphic {
        unit: 0,
        body_style: 1,
        graphic: text_graphic("+", 0.0),
    });
    lib
}

fn emit(lib: &LibSymbol, sym: &Symbol) -> Vec<SvgElement> {
    let xform = PageTransform {
        mm_to_unit: 1.0,
        translate_x: 0.0,
        translate_y: 0.0,
    };
    let mut out = Vec::new();
    push_symbol_lib_graphics(&mut out, sym, lib, &xform, &SchematicPalette::classic());
    out
}

fn text_of(elements: &[SvgElement]) -> (f32, String, String) {
    elements
        .iter()
        .find_map(|e| match e {
            SvgElement::Text {
                rotation_deg,
                align,
                v_align,
                ..
            } => Some((*rotation_deg, format!("{align:?}"), format!("{v_align:?}"))),
            _ => None,
        })
        .expect("a text element")
}

/// The SVG draws library text exactly as the shared lowering (and so the
/// canvas) does: folded to a readable angle with the justification swapped.
#[test]
fn library_text_matches_the_shared_lowering_under_every_turn() {
    for rotation in [0.0, 90.0, 180.0, 270.0] {
        for (mirror_x, mirror_y) in [(false, false), (true, false), (false, true)] {
            let sym = Symbol {
                rotation,
                mirror_x,
                mirror_y,
                ..Symbol::default()
            };
            let lib = lib_text();
            let want = lower_symbol(&lib, &sym)
                .into_iter()
                .find_map(|p| match p {
                    SymbolPrimitive::Text {
                        rotation_deg,
                        h_align,
                        v_align,
                        ..
                    } => Some((
                        rotation_deg as f32,
                        format!("{:?}", halign_to_svg(h_align)),
                        format!("{:?}", valign_to_svg(v_align)),
                    )),
                    _ => None,
                })
                .expect("lowered text");
            assert_eq!(
                text_of(&emit(&lib, &sym)),
                want,
                "rot {rotation} mx {mirror_x} my {mirror_y}"
            );
        }
    }
}

#[test]
fn library_text_on_a_quarter_turned_symbol_is_drawn_turned() {
    let sym = Symbol {
        rotation: 90.0,
        ..Symbol::default()
    };
    let (rotation, ..) = text_of(&emit(&lib_text(), &sym));
    assert!((rotation - 90.0).abs() < 1e-3, "got {rotation}");
}

#[test]
fn library_text_on_a_half_turned_symbol_is_flipped_like_a_field() {
    let sym = Symbol {
        rotation: 180.0,
        ..Symbol::default()
    };
    let (rotation, align, _) = text_of(&emit(&lib_text(), &sym));
    assert!(rotation.abs() < 1e-3, "got {rotation}");
    assert_eq!(align, format!("{:?}", halign_to_svg(HAlign::Right)));
}

#[test]
fn other_units_are_left_out_like_on_the_canvas() {
    let mut lib = lib_text();
    lib.graphics.push(LibGraphic {
        unit: 2,
        body_style: 1,
        graphic: text_graphic("u2", 0.0),
    });
    let sym = Symbol {
        unit: 1,
        ..Symbol::default()
    };
    assert_eq!(emit(&lib, &sym).len(), 1);
}

/// An alternate-body symbol draws the alternate graphics and not the normal
/// ones (#1233), exactly as the shared lowering does.
#[test]
fn the_alternate_body_draws_the_alternate_graphics() {
    let mut lib = lib_text();
    lib.graphics[0].body_style = 1;
    lib.graphics.push(LibGraphic {
        unit: 0,
        body_style: 2,
        graphic: text_graphic("&", 0.0),
    });
    let texts = |sym: &Symbol| -> Vec<String> {
        emit(&lib, sym)
            .into_iter()
            .filter_map(|e| match e {
                SvgElement::Text { text, .. } => Some(text),
                _ => None,
            })
            .collect()
    };
    let normal = Symbol::default();
    let alt = Symbol {
        body_style: signex_types::schematic::BodyStyle::Alternate,
        ..Symbol::default()
    };
    assert_eq!(texts(&normal), vec!["+".to_string()]);
    assert_eq!(texts(&alt), vec!["&".to_string()]);
}
