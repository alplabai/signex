use super::*;
use crate::schematic::{BodyStyle, LibGraphic, Pin, PinDirection, PinShapeStyle};

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
    primitives_bounds(&lower_symbol(
        lib,
        sym,
        &crate::schematic::SchematicDefaults::BUILT_IN,
    ))
    .expect("drawable")
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
    assert!(
        primitives_bounds(&lower_symbol(
            &l,
            &placed(0.0, 0.0, 0.0),
            &crate::schematic::SchematicDefaults::BUILT_IN
        ))
        .is_none()
    );
}

fn mirrored(mirror_x: bool, mirror_y: bool) -> Symbol {
    Symbol {
        mirror_x,
        mirror_y,
        ..Symbol::default()
    }
}

/// A body that is not centred on its origin: x 0..10, y 0..4 in Y-up space.
fn offset_body() -> LibSymbol {
    lib(
        vec![Graphic::Rectangle {
            start: Point::new(0.0, 0.0),
            end: Point::new(10.0, 4.0),
            width: 0.0,
            fill: FillType::None,
        }],
        vec![],
    )
}

#[test]
fn mirror_y_flips_the_box_about_the_origin_x() {
    let l = offset_body();
    assert_box(bounds(&l, &mirrored(false, false)), 0.0, -4.0, 10.0, 0.0);
    assert_box(bounds(&l, &mirrored(false, true)), -10.0, -4.0, 0.0, 0.0);
}

#[test]
fn mirror_x_flips_the_box_about_the_origin_y() {
    assert_box(
        bounds(&offset_body(), &mirrored(true, false)),
        0.0,
        0.0,
        10.0,
        4.0,
    );
}

fn lib_text(rotation: f64) -> Graphic {
    Graphic::Text {
        text: "+".into(),
        position: Point::new(0.0, 0.0),
        rotation,
        font_size: 1.27,
        bold: false,
        italic: false,
        justify_h: HAlign::Left,
        justify_v: VAlign::Top,
    }
}

fn first_text(l: &LibSymbol, sym: &Symbol) -> (f64, HAlign, VAlign) {
    lower_symbol(l, sym, &crate::schematic::SchematicDefaults::BUILT_IN)
        .into_iter()
        .find_map(|p| match p {
            SymbolPrimitive::Text {
                rotation_deg,
                h_align,
                v_align,
                ..
            } => Some((rotation_deg, h_align, v_align)),
            _ => None,
        })
        .expect("a text primitive")
}

#[test]
fn library_text_on_a_half_turned_symbol_is_readable_and_flipped() {
    let l = lib(vec![lib_text(0.0)], vec![]);
    assert_eq!(
        first_text(&l, &placed(0.0, 0.0, 180.0)),
        (0.0, HAlign::Right, VAlign::Top)
    );
    assert_eq!(
        first_text(&l, &placed(0.0, 0.0, 270.0)),
        (90.0, HAlign::Left, VAlign::Bottom)
    );
}

#[test]
fn library_text_on_a_quarter_turned_symbol_stays_turned() {
    let l = lib(vec![lib_text(0.0)], vec![]);
    assert_eq!(
        first_text(&l, &placed(0.0, 0.0, 90.0)),
        (90.0, HAlign::Left, VAlign::Top)
    );
}

#[test]
fn library_text_swaps_justification_on_a_mirror() {
    let l = lib(vec![lib_text(0.0)], vec![]);
    assert_eq!(
        first_text(&l, &mirrored(false, true)),
        (0.0, HAlign::Right, VAlign::Top)
    );
    assert_eq!(
        first_text(&l, &mirrored(true, false)),
        (0.0, HAlign::Left, VAlign::Bottom)
    );
}

#[test]
fn text_box_text_sits_in_the_frames_top_left_corner() {
    // Frame x 0..10, y 0..4 in Y-up space: its top-left is (0, 4), which is
    // world (0, -4). The old anchor was the frame's bottom-left, (0, 0).
    let l = lib(
        vec![Graphic::TextBox {
            text: "T".into(),
            position: Point::new(0.0, 0.0),
            rotation: 0.0,
            size: Point::new(10.0, 4.0),
            font_size: 1.27,
            bold: false,
            italic: false,
            width: 0.0,
            fill: FillType::None,
        }],
        vec![],
    );
    let at = lower_symbol(
        &l,
        &placed(0.0, 0.0, 0.0),
        &crate::schematic::SchematicDefaults::BUILT_IN,
    )
    .into_iter()
    .find_map(|p| match p {
        SymbolPrimitive::Text { position, .. } => Some(position),
        _ => None,
    })
    .expect("a text primitive");
    assert!(
        (at.x - 0.0).abs() < 1e-9 && (at.y + 4.0).abs() < 1e-9,
        "{at:?}"
    );
}

#[test]
fn filled_arc_keeps_its_fill() {
    let arc = Graphic::Arc {
        start: Point::new(-5.0, 0.0),
        mid: Point::new(0.0, 5.0),
        end: Point::new(5.0, 0.0),
        width: 0.0,
        fill: FillType::Background,
    };
    let prims = lower_symbol(
        &lib(vec![arc], vec![]),
        &placed(0.0, 0.0, 0.0),
        &crate::schematic::SchematicDefaults::BUILT_IN,
    );
    assert!(matches!(
        prims[0],
        SymbolPrimitive::Arc {
            fill: FillType::Background,
            ..
        }
    ));
}

#[test]
fn arc_polyline_runs_start_to_end_through_the_bulge() {
    let pts = arc_polyline(
        Point::new(-5.0, 0.0),
        Point::new(0.0, 5.0),
        Point::new(5.0, 0.0),
    );
    let (first, last) = (pts[0], pts[pts.len() - 1]);
    assert!((first.x + 5.0).abs() < 1e-9 && first.y.abs() < 1e-9);
    assert!((last.x - 5.0).abs() < 1e-9 && last.y.abs() < 1e-9);
    assert!(pts.iter().any(|p| (p.y - 5.0).abs() < 1e-9));
    assert!(pts.iter().all(|p| p.y >= -1e-9));
}

#[test]
fn caller_width_replaces_the_monospace_estimate() {
    let (anchor, size) = ((10.0, 5.0), 2.0);
    let estimate = text_aabb("abcd", size, anchor, 0.0, HAlign::Left, VAlign::Top, None);
    let same = text_aabb_with_width(
        crate::font::family_width_mm("abcd", None, text_height_mm(size)),
        size,
        anchor,
        0.0,
        HAlign::Left,
        VAlign::Top,
        None,
    );
    assert_box(estimate, same.min_x, same.min_y, same.max_x, same.max_y);
    let wide = text_aabb_with_width(10.0, size, anchor, 0.0, HAlign::Right, VAlign::Top, None);
    assert_box(
        wide,
        0.0,
        5.0,
        10.0,
        5.0 + crate::font::line_box_mm(text_height_mm(size), None),
    );
}

/// A symbol with a 2x2 normal body and a 6x6 alternate body (#1233).
fn two_bodies() -> LibSymbol {
    let mut lib = lib(vec![rect(2.0, 2.0)], vec![]);
    lib.graphics.push(LibGraphic {
        unit: 0,
        body_style: 2,
        graphic: rect(6.0, 6.0),
    });
    lib
}

fn alternate() -> Symbol {
    Symbol {
        body_style: BodyStyle::Alternate,
        ..Symbol::default()
    }
}

#[test]
fn bounds_follow_the_selected_body() {
    let lib = two_bodies();
    let normal = bounds(&lib, &Symbol::default());
    let alt = bounds(&lib, &alternate());
    assert!((normal.max_x - normal.min_x - 2.0).abs() < 1e-9);
    assert!((alt.max_x - alt.min_x - 6.0).abs() < 1e-9);
}

#[test]
fn an_alternate_request_falls_back_to_normal_when_the_unit_has_no_alternate() {
    let lib = lib(vec![rect(2.0, 2.0)], vec![pin(0.0, 0.0, 0.0, 1.0)]);
    assert!(!lib.has_alternate_body(1));
    assert_eq!(
        lib.effective_body(1, BodyStyle::Alternate),
        BodyStyle::Normal
    );
    assert_eq!(
        lower_symbol(
            &lib,
            &alternate(),
            &crate::schematic::SchematicDefaults::BUILT_IN
        ),
        lower_symbol(
            &lib,
            &Symbol::default(),
            &crate::schematic::SchematicDefaults::BUILT_IN
        )
    );
}

#[test]
fn alternate_presence_is_judged_per_unit() {
    let mut lib = two_bodies();
    lib.graphics[1].unit = 2;
    assert!(lib.has_alternate_body(2));
    assert!(!lib.has_alternate_body(1));
}

#[test]
fn lower_placed_symbol_gives_a_power_port_without_a_library_its_body() {
    let sym = Symbol {
        lib_id: "power:GND".into(),
        is_power: true,
        position: Point::new(10.0, 10.0),
        ..Symbol::default()
    };
    let prims = lower_placed_symbol(None, &sym, &crate::schematic::SchematicDefaults::BUILT_IN);
    let bounds = primitives_bounds(&prims).expect("body");
    // Ground hangs below the connection point; the stem starts at it.
    assert!(bounds.max_y > 10.0, "{bounds:?}");
    assert!(bounds.min_y <= 10.0, "{bounds:?}");
    let plain = Symbol {
        is_power: false,
        ..sym
    };
    assert!(
        lower_placed_symbol(None, &plain, &crate::schematic::SchematicDefaults::BUILT_IN)
            .is_empty()
    );
}

/// #1858: a pin strokes at its symbol's library width, like the body.
#[test]
fn pins_carry_the_symbols_library_stroke_width() {
    let mut body = rect(10.0, 10.0);
    if let Graphic::Rectangle { width, .. } = &mut body {
        *width = 0.254;
    }
    let l = lib(vec![body], vec![pin(-5.0, 0.0, 180.0, 2.54)]);
    let pin_width = lower_symbol(
        &l,
        &placed(0.0, 0.0, 0.0),
        &crate::schematic::SchematicDefaults::BUILT_IN,
    )
    .into_iter()
    .find_map(|p| match p {
        SymbolPrimitive::Pin { width, .. } => Some(width),
        _ => None,
    })
    .expect("a pin");
    assert_eq!(pin_width, 0.254);
}

/// #2025 - a width-0 symbol's pins draw at the default wire width, like its
/// width-0 outline, not at the render floor.
#[test]
fn a_width_zero_symbol_strokes_its_pins_at_the_default_wire_width() {
    let primitives = lower_symbol(
        &lib(vec![rect(4.0, 2.0)], vec![pin(-4.0, 0.0, 0.0, 2.0)]),
        &placed(0.0, 0.0, 0.0),
        &crate::schematic::SchematicDefaults::BUILT_IN,
    );
    let pin_width = primitives.iter().find_map(|p| match p {
        SymbolPrimitive::Pin { width, .. } => Some(*width),
        _ => None,
    });
    assert_eq!(
        pin_width,
        Some(crate::schematic::SCHEMATIC_DEFAULT_WIRE_STROKE_MM)
    );
}

/// A text's box is the line box of the face it draws in, taller than its
/// cap height (#2099).
#[test]
fn a_text_box_is_the_line_box_of_its_face() {
    for family in [None, Some("Iosevka")] {
        let b = text_aabb(
            "R1",
            1.27,
            (0.0, 0.0),
            0.0,
            HAlign::Left,
            VAlign::Top,
            family,
        );
        let want = crate::font::line_box_mm(1.27, family);
        assert!(((b.max_y - b.min_y) - want).abs() < 1e-9, "{family:?}");
        assert!(
            b.max_y - b.min_y > 1.27,
            "{family:?}: taller than the cap height"
        );
    }
}

/// A multi-line text's box is its line count times the line box (#2120), and
/// the whole block is aligned, not the first line.
#[test]
fn a_multi_line_text_box_is_the_line_count_times_the_line_box() {
    let line = crate::font::line_box_mm(1.27, None);
    let one = text_aabb("ab", 1.27, (4.0, 6.0), 0.0, HAlign::Left, VAlign::Top, None);
    let three = text_aabb(
        "ab\ncd\nef",
        1.27,
        (4.0, 6.0),
        0.0,
        HAlign::Left,
        VAlign::Top,
        None,
    );
    assert!(((three.max_y - three.min_y) - 3.0 * line).abs() < 1e-9);
    assert_eq!(three.min_y, 6.0);
    assert_eq!(
        (three.min_x, three.max_x),
        (one.min_x, one.max_x),
        "equal-width lines keep the one-line width"
    );
    assert_eq!(crate::font::line_count("a\r\nb\rc\n"), 4);
}

#[test]
fn a_multi_line_box_is_aligned_as_a_block() {
    let h = 3.0 * crate::font::line_box_mm(1.27, None);
    for (v, top) in [
        (VAlign::Top, 0.0),
        (VAlign::Center, -h * 0.5),
        (VAlign::Bottom, -h),
    ] {
        let b = text_aabb_lines(8.0, 3, 1.27, (0.0, 0.0), 0.0, HAlign::Left, v, None);
        assert_box(b, 0.0, top, 8.0, top + h);
    }
}

#[test]
fn a_rotated_multi_line_box_turns_about_the_anchor() {
    let h = 3.0 * crate::font::line_box_mm(1.27, None);
    let b = text_aabb_lines(
        8.0,
        3,
        1.27,
        (10.0, 20.0),
        90.0,
        HAlign::Left,
        VAlign::Top,
        None,
    );
    assert_box(b, 10.0 - h, 20.0, 10.0, 28.0);
}

/// One line is byte-identical through the line-aware box.
#[test]
fn a_one_line_text_box_is_unchanged_by_the_line_count() {
    let a = text_aabb_with_width(
        5.5,
        1.27,
        (1.0, 2.0),
        30.0,
        HAlign::Center,
        VAlign::Bottom,
        None,
    );
    let b = text_aabb_lines(
        5.5,
        1,
        1.27,
        (1.0, 2.0),
        30.0,
        HAlign::Center,
        VAlign::Bottom,
        None,
    );
    assert_eq!(
        (a.min_x, a.min_y, a.max_x, a.max_y),
        (b.min_x, b.min_y, b.max_x, b.max_y)
    );
}

fn tips(lib: &LibSymbol, sym: &Symbol) -> Vec<(String, Point)> {
    pin_tips(lib, sym)
        .map(|(lp, tip)| (lp.pin.number.clone(), tip))
        .collect()
}

fn assert_tip(got: Point, x: f64, y: f64) {
    assert!(
        (got.x - x).abs() < 1e-9 && (got.y - y).abs() < 1e-9,
        "expected ({x}, {y}), got {got:?}"
    );
}

#[test]
fn pin_tips_follow_rotation() {
    let l = lib(vec![], vec![pin(5.0, 0.0, 0.0, 2.0)]);
    for (rotation, x, y) in [
        (0.0, 15.0, 20.0),
        (90.0, 10.0, 15.0),
        (180.0, 5.0, 20.0),
        (270.0, 10.0, 25.0),
    ] {
        let got = tips(&l, &placed(10.0, 20.0, rotation));
        assert_eq!(got.len(), 1);
        assert_tip(got[0].1, x, y);
    }
}

#[test]
fn pin_tips_follow_mirror() {
    let l = lib(vec![], vec![pin(5.0, 2.0, 0.0, 2.0)]);
    // Library Y is up, world Y is down: the unmirrored tip is (5, -2).
    let plain = placed(10.0, 20.0, 0.0);
    assert_tip(tips(&l, &plain)[0].1, 15.0, 18.0);
    let mirror_x = Symbol {
        mirror_x: true,
        ..plain.clone()
    };
    assert_tip(tips(&l, &mirror_x)[0].1, 15.0, 22.0);
    let mirror_y = Symbol {
        mirror_y: true,
        ..plain
    };
    assert_tip(tips(&l, &mirror_y)[0].1, 5.0, 18.0);
}

#[test]
fn pin_tips_keep_only_the_placed_unit_and_body() {
    let mut l = lib(vec![], vec![pin(1.0, 0.0, 0.0, 2.0)]);
    let mut add = |number: &str, unit: u32, body_style: u32| {
        let mut p = pin(2.0, 0.0, 0.0, 2.0);
        p.number = number.into();
        l.pins.push(LibPin {
            unit,
            body_style,
            pin: p,
        });
    };
    add("U1", 1, 1);
    add("U2", 2, 1);
    add("ALT", 0, 2);
    let numbers =
        |sym: &Symbol| -> Vec<String> { tips(&l, sym).into_iter().map(|(n, _)| n).collect() };
    let unit2 = Symbol {
        unit: 2,
        ..placed(0.0, 0.0, 0.0)
    };
    assert_eq!(numbers(&unit2), ["1", "U2"]);
    let alt = Symbol {
        unit: 1,
        body_style: BodyStyle::Alternate,
        ..placed(0.0, 0.0, 0.0)
    };
    assert_eq!(numbers(&alt), ["ALT"]);
    let normal = Symbol {
        unit: 1,
        ..placed(0.0, 0.0, 0.0)
    };
    assert_eq!(numbers(&normal), ["1", "U1"]);
}
