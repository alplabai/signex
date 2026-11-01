use super::*;
use signex_types::schematic::{Graphic, HAlign, LibGraphic, VAlign, lower_symbol};

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
    push_symbol_lib_graphics(
        &mut out,
        sym,
        Some(lib),
        &xform,
        &SchematicPalette::classic(),
    );
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

#[test]
fn a_power_port_without_a_library_entry_is_drawn_from_its_built_in_body() {
    let sym = Symbol {
        lib_id: "power:GND".into(),
        is_power: true,
        ..Symbol::default()
    };
    let xform = PageTransform {
        mm_to_unit: 1.0,
        translate_x: 0.0,
        translate_y: 0.0,
    };
    let mut out = Vec::new();
    push_symbol_lib_graphics(&mut out, &sym, None, &xform, &SchematicPalette::classic());
    assert!(!out.is_empty());
    let mut plain = Vec::new();
    let not_power = Symbol {
        is_power: false,
        ..sym
    };
    push_symbol_lib_graphics(
        &mut plain,
        &not_power,
        None,
        &xform,
        &SchematicPalette::classic(),
    );
    assert!(plain.is_empty());
}

/// A power port's library entry (#1408) draws through its pin, and the SVG
/// shows the same elements as the entry-less built-in body.
#[test]
fn power_port_library_entry_draws_its_pin_like_the_built_in_body() {
    for lib_id in ["power:GND", "power:+5V", "power:arrow"] {
        let sym = Symbol {
            lib_id: lib_id.to_string(),
            is_power: true,
            unit: 1,
            ..Symbol::default()
        };
        let lib = signex_types::schematic::power_port_lib_symbol(lib_id);
        let mut with_entry: Vec<String> =
            emit(&lib, &sym).iter().map(|e| format!("{e:?}")).collect();
        let mut xform_out = Vec::new();
        push_symbol_lib_graphics(
            &mut xform_out,
            &sym,
            None,
            &PageTransform {
                mm_to_unit: 1.0,
                translate_x: 0.0,
                translate_y: 0.0,
            },
            &SchematicPalette::classic(),
        );
        let mut built_in: Vec<String> = xform_out.iter().map(|e| format!("{e:?}")).collect();
        with_entry.sort();
        built_in.sort();
        assert_eq!(with_entry, built_in, "{lib_id}");
    }
}

/// A pin name sits inside the body, past the pin's body end, even when the
/// library stores no offset: centred on the connection point it would straddle
/// the wire (#1798).
#[test]
fn a_pin_name_is_inside_the_body_past_the_pin_end() {
    use signex_types::schematic::{LibPin, PIN_NAME_OFFSET_MM, PinDirection, PinShapeStyle};
    let pin = |x: f64, y: f64, rotation: f64, name: &str| LibPin {
        unit: 0,
        body_style: 1,
        pin: Pin {
            direction: PinDirection::Passive,
            shape_style: PinShapeStyle::Plain,
            position: Point::new(x, y),
            rotation,
            length: 2.54,
            name: name.into(),
            number: "1".into(),
            visible: true,
            name_visible: true,
            number_visible: false,
        },
    };
    let mut lib = lib_text();
    lib.show_pin_names = true;
    lib.pin_name_offset = 0.0;
    // Left pin: tip at x = -5.08, body edge at x = -2.54. Top pin: tip at
    // y = 5.08 (library Y is up), body edge 2.54 below it on screen.
    lib.pins.push(pin(-5.08, 0.0, 0.0, "IN"));
    lib.pins.push(pin(0.0, 5.08, 270.0, "TOP"));
    let sym = Symbol::default();
    let xform = PageTransform {
        mm_to_unit: 1.0,
        translate_x: 0.0,
        translate_y: 0.0,
    };
    let mut out = Vec::new();
    push_symbol_pins(
        &mut out,
        &sym,
        &lib,
        &xform,
        &signex_types::markup::ExpressionEvalContext::default(),
        None,
        &SchematicPalette::classic(),
    );
    let names: Vec<_> = out
        .iter()
        .filter_map(|e| match e {
            SvgElement::Text {
                x,
                y,
                align,
                v_align,
                text,
                ..
            } if text == "IN" || text == "TOP" => Some((
                text.clone(),
                *x,
                *y,
                format!("{align:?}"),
                format!("{v_align:?}"),
            )),
            _ => None,
        })
        .collect();
    let off = PIN_NAME_OFFSET_MM as f32;
    let (_, x, y, align, _) = names.iter().find(|n| n.0 == "IN").expect("IN");
    assert!((x - (-2.54 + off)).abs() < 1e-4, "IN starts inside: {x}");
    assert!(y.abs() < 1e-4, "IN is on the pin line: {y}");
    assert_eq!(align, "Left");
    let (_, x, y, align, v_align) = names.iter().find(|n| n.0 == "TOP").expect("TOP");
    assert!(x.abs() < 1e-4, "TOP is centred on the pin: {x}");
    // Screen Y is down: the body end is at y = 2.54 - 5.08 below the tip.
    assert!(
        (y - (-5.08 + 2.54 + off)).abs() < 1e-4,
        "TOP hangs inside: {y}"
    );
    assert_eq!((align.as_str(), v_align.as_str()), ("Center", "Top"));
}

/// #1858: output pins stroke at the symbol's library width, like the body
/// and like the canvas, in mm with no screen floor.
#[test]
fn pins_stroke_at_the_symbols_library_width() {
    let mut lib: LibSymbol =
        serde_json::from_value(serde_json::json!({ "id": "L:S" })).expect("lib symbol");
    lib.graphics.push(LibGraphic {
        unit: 0,
        body_style: 1,
        graphic: Graphic::Rectangle {
            start: Point::new(-2.54, -2.54),
            end: Point::new(2.54, 2.54),
            width: 0.254,
            fill: FillType::None,
        },
    });
    lib.pins.push(signex_types::schematic::LibPin {
        unit: 0,
        body_style: 1,
        pin: Pin {
            direction: signex_types::schematic::PinDirection::Passive,
            shape_style: signex_types::schematic::PinShapeStyle::Plain,
            position: Point::new(-5.08, 0.0),
            rotation: 0.0,
            length: 2.54,
            name: String::new(),
            number: "1".into(),
            visible: true,
            name_visible: true,
            number_visible: true,
        },
    });
    // The pin stub is the one open two-point path; the body outline closes.
    let widths: Vec<(bool, f32)> = emit(&lib, &Symbol::default())
        .iter()
        .filter_map(|e| match e {
            SvgElement::Path { commands, style } => Some((
                !commands.iter().any(|c| matches!(c, SvgPathCommand::Close)),
                style.stroke_width,
            )),
            _ => None,
        })
        .collect();
    let pin = widths.iter().find(|(open, _)| *open).expect("a pin stub");
    let body = widths
        .iter()
        .find(|(open, _)| !*open)
        .expect("a body outline");
    assert_eq!(pin.1, 0.254);
    assert_eq!(pin.1, body.1);
}
