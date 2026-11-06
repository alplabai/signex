use super::*;

const B: SchematicDefaults = SchematicDefaults::BUILT_IN;

/// A project that sets only its wire and bus widths.
fn project(wire: f64, bus: f64) -> SchematicDefaults {
    SchematicDefaults {
        wire_width_mm: wire,
        bus_width_mm: bus,
        ..B
    }
}

#[test]
fn the_built_in_table_keeps_the_values_sheets_drew_before_it() {
    assert_eq!(B.wire_width_mm, 0.254);
    assert_eq!(B.bus_width_mm, 0.45);
    assert_eq!(B.symbol_line_width_mm, 0.254);
    assert_eq!(B.shape_stroke_mm, 0.254, "the #2091 one stroke rule");
    assert_eq!(B.junction_diameter_mm, 0.7);
    assert_eq!(B.text_size_pt, 10.0);
}

#[test]
fn a_wire_width_is_one_value_for_the_canvas_and_the_exports() {
    // Default: no own width, or a class width of 0.
    assert_eq!(B.wire_mm(0.0, None), SCHEMATIC_DEFAULT_WIRE_STROKE_MM);
    assert_eq!(B.wire_mm(0.0, Some(0.0)), SCHEMATIC_DEFAULT_WIRE_STROKE_MM);
    assert_eq!(B.wire_mm(0.3, Some(0.0)), 0.3);
    assert_eq!(B.wire_mm(f64::NAN, None), SCHEMATIC_DEFAULT_WIRE_STROKE_MM);
    // Explicit widths keep their value, down to the floor.
    assert_eq!(B.wire_mm(0.5, None), 0.5);
    assert_eq!(B.wire_mm(0.1, None), SCHEMATIC_RENDER_MIN_STROKE_MM);
}

/// #2026: explicit > net class > project > built-in.
#[test]
fn the_wire_width_falls_back_explicit_class_project_built_in() {
    let p = project(0.4, 0.0);
    assert_eq!(p.wire_mm(0.5, Some(0.8)), 0.5, "explicit beats the class");
    assert_eq!(p.wire_mm(0.0, Some(0.8)), 0.8, "class beats the project");
    assert_eq!(p.wire_mm(0.0, None), 0.4, "project beats the built-in");
    assert_eq!(B.wire_mm(0.0, None), 0.254, "built-in last");
}

#[test]
fn the_bus_width_falls_back_project_built_in_and_beats_the_wire() {
    assert_eq!(project(0.0, 0.6).bus_mm(), 0.6);
    assert_eq!(project(0.0, f64::NAN).bus_mm(), 0.45);
    assert_eq!(B.bus_mm(), 0.45);
    assert!(
        B.bus_mm() > B.wire_mm(0.0, None),
        "a bus is thicker by nature"
    );
}

#[test]
fn a_pin_draws_as_wide_as_a_wire_by_default() {
    let unset_symbol = B.symbol_stroke_mm([0.0, 0.0]);
    assert_eq!(unset_symbol, B.wire_mm(0.0, None));
    assert_eq!(B.symbol_stroke_mm([]), B.wire_mm(0.0, None));
}

#[test]
fn effective_shape_width_defaults_a_zero_or_broken_width() {
    assert_eq!(B.shape_stroke_mm(0.0), SCHEMATIC_DEFAULT_WIRE_STROKE_MM);
    assert_eq!(
        B.shape_stroke_mm(f64::NAN),
        SCHEMATIC_DEFAULT_WIRE_STROKE_MM
    );
    assert_eq!(B.shape_stroke_mm(0.254), 0.254);
    assert_eq!(
        B.shape_stroke_mm(0.05),
        SCHEMATIC_RENDER_MIN_STROKE_MM,
        "floor"
    );
    let p = SchematicDefaults {
        shape_stroke_mm: 0.3,
        ..B
    };
    assert_eq!(p.shape_stroke_mm(0.0), 0.3);
    assert_eq!(p.shape_stroke_mm(0.2), 0.2);
}

#[test]
fn a_symbol_strokes_at_its_most_common_library_width() {
    assert_eq!(B.symbol_stroke_mm([0.254, 0.254, 0.5]), 0.254);
    assert_eq!(
        B.symbol_stroke_mm([0.254, 0.5]),
        0.5,
        "a tie takes the wider"
    );
    assert_eq!(
        B.symbol_stroke_mm([0.0, 0.0, 0.5]),
        SCHEMATIC_DEFAULT_WIRE_STROKE_MM
    );
    let p = SchematicDefaults {
        symbol_line_width_mm: 0.3,
        ..B
    };
    assert_eq!(p.symbol_stroke_mm([0.0, 0.0, 0.5]), 0.3);
}

/// #2025 - a width-0 symbol stroke is the default wire width, not the floor;
/// explicit widths keep the floor.
#[test]
fn a_zero_symbol_stroke_is_the_default_wire_width() {
    assert_eq!(B.symbol_line_mm(0.0), SCHEMATIC_DEFAULT_WIRE_STROKE_MM);
    assert_eq!(B.symbol_line_mm(f64::NAN), SCHEMATIC_DEFAULT_WIRE_STROKE_MM);
    assert_eq!(B.symbol_line_mm(0.5), 0.5);
    assert_eq!(B.symbol_line_mm(0.01), SCHEMATIC_RENDER_MIN_STROKE_MM);
}

#[test]
fn a_junction_keeps_the_canvas_floor_and_takes_the_project_size() {
    assert_eq!(B.junction_radius_mm(0.0), 0.35);
    assert_eq!(B.junction_radius_mm(0.4), 0.35, "the floor holds");
    assert_eq!(B.junction_radius_mm(1.2), 0.6);
    let p = SchematicDefaults {
        junction_diameter_mm: 1.0,
        ..B
    };
    assert_eq!(p.junction_radius_mm(0.0), 0.5);
    assert_eq!(p.junction_radius_mm(1.2), 0.6, "an explicit size wins");
}

#[test]
fn a_wire_line_style_falls_back_class_project_built_in() {
    use crate::net_class::NetLineStyle as L;
    assert_eq!(B.wire_line_style, L::Solid);
    assert_eq!(B.shape_fill, super::super::FillType::None);
    let p = SchematicDefaults {
        wire_line_style: L::Dashed,
        ..B
    };
    assert_eq!(
        p.wire_line(Some(L::Dotted)),
        L::Dotted,
        "class beats project"
    );
    assert_eq!(p.wire_line(None), L::Dashed, "project beats built-in");
    assert_eq!(
        p.wire_line(Some(L::Solid)),
        L::Solid,
        "an explicit Solid class overrides the project"
    );
    assert_eq!(B.wire_line(None), L::Solid);
}

#[test]
fn placed_text_rounds_once_and_never_to_zero() {
    let p = |pt| SchematicDefaults {
        text_size_pt: pt,
        ..B
    };
    assert_eq!(p(10.4).placed_text_pt(), 10);
    assert_eq!(
        p(10.4).placed_text_mm(),
        10.0 * crate::schematic::SCHEMATIC_PT_TO_MM
    );
    assert_eq!(p(0.2).placed_text_pt(), 1);
    assert_eq!(B.placed_text_mm(), crate::schematic::SCHEMATIC_TEXT_MM);
}

/// #2091, on the table: every stored-stroke kind (symbol graphic and pin,
/// drawing and child sheet frame, wire) resolves a stored 0 or a broken
/// width to 0.254 mm and floors an explicit width at 0.15 mm.
#[test]
fn every_stored_stroke_kind_resolves_zero_to_the_default() {
    for broken in [0.0, f64::NAN, f64::INFINITY, -1.0] {
        assert_eq!(B.symbol_line_mm(broken), SCHEMATIC_DEFAULT_WIRE_STROKE_MM);
        assert_eq!(B.shape_stroke_mm(broken), SCHEMATIC_DEFAULT_WIRE_STROKE_MM);
        assert_eq!(B.wire_mm(broken, None), SCHEMATIC_DEFAULT_WIRE_STROKE_MM);
    }
    for kind in [
        B.symbol_line_mm(0.05),
        B.shape_stroke_mm(0.05),
        B.wire_mm(0.05, None),
    ] {
        assert_eq!(kind, SCHEMATIC_RENDER_MIN_STROKE_MM);
    }
}
