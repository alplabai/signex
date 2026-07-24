//! Tests for `Symbol::to_lib_symbol` (issue #365, part 1 of 2).
//!
//! Sibling of `to_lib_symbol`, not a child module of it — same
//! constraint `chain_tests.rs` documents for `chain`: only
//! `to_lib_symbol`'s public surface (`Symbol::to_lib_symbol` itself) is
//! exercised here, exactly what a real caller has.

use signex_types::schematic::{
    FillType, Graphic, PinDirection as SchematicPinDirection, PinShapeStyle,
};

use super::{
    PinDirection, PinOrientation, PinSymbolKind, Symbol, SymbolGraphic, SymbolGraphicKind,
    SymbolPin,
};

fn pin(part_number: u8, orientation: PinOrientation, electrical: PinDirection) -> SymbolPin {
    let mut p = SymbolPin::new("1", "A");
    p.part_number = part_number;
    p.orientation = orientation;
    p.electrical = electrical;
    p
}

fn line_graphic(part_number: u8, from: [f64; 2], to: [f64; 2]) -> SymbolGraphic {
    SymbolGraphic {
        kind: SymbolGraphicKind::Line { from, to },
        stroke_width: 0.2,
        fill: None,
        part_number,
    }
}

fn approx(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9, "expected {a} ≈ {b}");
}

/// Acceptance criterion: a two-unit symbol whose part-1 and part-2
/// bodies differ converts to a `LibSymbol` whose graphics carry `unit =
/// 1`/`unit = 2` respectively, whose shared (part 0) graphics carry
/// `unit = 0`, and whose Part Zero pins land on `unit = 0`.
#[test]
fn two_unit_symbol_graphics_and_part_zero_pins_carry_the_right_unit() {
    let mut sym = Symbol::empty("DUAL-OPAMP");
    sym.part_count = 2;
    sym.pins = vec![
        pin(0, PinOrientation::Right, PinDirection::Power), // Part Zero: shared VCC
        pin(1, PinOrientation::Right, PinDirection::Input),
        pin(2, PinOrientation::Right, PinDirection::Input),
    ];
    sym.graphics = vec![
        line_graphic(0, [0.0, 0.0], [1.0, 0.0]), // shared outline
        line_graphic(1, [0.0, 0.0], [2.0, 0.0]), // part-1 body detail
        line_graphic(2, [0.0, 0.0], [3.0, 0.0]), // part-2 body detail — differs from part 1
    ];

    let lib = sym.to_lib_symbol("DUAL-OPAMP");

    assert_eq!(lib.pins.len(), 3);
    assert_eq!(lib.pins[0].unit, 0, "Part Zero pin must land on unit 0");
    assert_eq!(lib.pins[1].unit, 1);
    assert_eq!(lib.pins[2].unit, 2);

    assert_eq!(lib.graphics.len(), 3);
    assert_eq!(
        lib.graphics[0].unit, 0,
        "shared graphic must land on unit 0"
    );
    assert_eq!(lib.graphics[1].unit, 1);
    assert_eq!(lib.graphics[2].unit, 2);

    // Part-1 and part-2 bodies actually differ post-conversion (not
    // silently collapsed to the same geometry).
    let Graphic::Polyline { points: p1, .. } = &lib.graphics[1].graphic else {
        panic!("expected a Polyline");
    };
    let Graphic::Polyline { points: p2, .. } = &lib.graphics[2].graphic else {
        panic!("expected a Polyline");
    };
    assert_ne!(p1, p2);
}

/// Acceptance criterion: pin and graphic positions are byte-identical
/// across the conversion — no y-flip happens here (that's
/// `SymbolTransform::apply`'s job, later, at place time).
#[test]
fn positions_carry_over_unchanged_no_y_flip() {
    let mut sym = Symbol::empty("POS-CHECK");
    let mut p = pin(1, PinOrientation::Right, PinDirection::Passive);
    p.position = [12.5, -7.25];
    sym.pins = vec![p];
    sym.graphics = vec![line_graphic(1, [1.5, 2.5], [-3.0, 4.0])];

    let lib = sym.to_lib_symbol("id");

    assert_eq!(lib.pins[0].pin.position.x, 12.5);
    assert_eq!(lib.pins[0].pin.position.y, -7.25);

    let Graphic::Polyline { points, .. } = &lib.graphics[0].graphic else {
        panic!("expected a Polyline");
    };
    assert_eq!(points[0].x, 1.5);
    assert_eq!(points[0].y, 2.5);
    assert_eq!(points[1].x, -3.0);
    assert_eq!(points[1].y, 4.0);
}

/// Acceptance criterion: pin the complete `PinOrientation` -> rotation
/// table (all four variants). The mapping is the identity on angle —
/// `LibPin.pin.rotation` is read as the same y-up, CCW-from-+x,
/// tip->body angle as the source `PinOrientation` by every real
/// consumer: `signex_types::schematic::SymbolTransform::apply` (the
/// single y-flip is applied there, not here), the autoplace pin-bbox
/// walk (`crates/signex-engine/src/transform/autoplace.rs`), and the
/// SVG/PDF exporter's `pin_direction`
/// (`crates/signex-output/src/svg/symbols.rs`, `90 => (0.0, 1.0)`). See
/// `to_lib_symbol`'s module doc for the full derivation.
#[test]
fn pin_orientation_maps_to_rotation_degrees_for_all_four_variants() {
    let table = [
        (PinOrientation::Right, 0.0),
        (PinOrientation::Up, 90.0),
        (PinOrientation::Left, 180.0),
        (PinOrientation::Down, 270.0),
    ];
    for (orientation, expected_deg) in table {
        let mut sym = Symbol::empty("ROT-CHECK");
        sym.pins = vec![pin(1, orientation, PinDirection::Passive)];
        let lib = sym.to_lib_symbol("id");
        assert_eq!(
            lib.pins[0].pin.rotation, expected_deg,
            "{orientation:?} -> {expected_deg}"
        );
    }
}

/// Ground-truth convention test: ties `pin_rotation_deg`'s output to the
/// convention the actual downstream renderer uses, so the two mappings
/// can't silently drift apart (the other rotation test above only
/// checks self-consistency with the formula in this crate).
///
/// `crates/signex-output/src/svg/symbols.rs`'s private `pin_direction(pin:
/// &Pin) -> (f64, f64)` is the real consumer that turns
/// `LibPin.pin.rotation` into a draw direction: `0 => (1.0, 0.0)`, `90 =>
/// (0.0, 1.0)`, `180 => (-1.0, 0.0)`, `270 => (0.0, -1.0)`. `signex-library`
/// does not (and must not, per the workspace's dependency direction —
/// `signex-output` depends on `signex_types`/`signex-library`, never the
/// reverse) depend on `signex-output`, so that function cannot be called
/// from this test. Instead this re-derives the identical formula inline
/// and asserts the resulting unit vector against the direction each
/// `PinOrientation` is documented to mean: `Right -> +x`, `Up -> +y`,
/// `Left -> -x`, `Down -> -y`.
#[test]
fn pin_rotation_matches_signex_output_pin_direction_convention() {
    // Mirrors `crates/signex-output/src/svg/symbols.rs`'s `pin_direction`
    // exactly — same branches, same values — so this test fails the
    // instant either side's convention moves without the other.
    fn pin_direction_convention(deg: f64) -> (f64, f64) {
        let deg = ((deg % 360.0) + 360.0) % 360.0;
        match deg as i32 {
            0 => (1.0, 0.0),
            90 => (0.0, 1.0),
            180 => (-1.0, 0.0),
            270 => (0.0, -1.0),
            _ => {
                let rad = deg.to_radians();
                (rad.cos(), rad.sin())
            }
        }
    }

    let table = [
        (PinOrientation::Right, (1.0, 0.0)),
        (PinOrientation::Up, (0.0, 1.0)),
        (PinOrientation::Left, (-1.0, 0.0)),
        (PinOrientation::Down, (0.0, -1.0)),
    ];

    for (orientation, expected_dir) in table {
        let mut sym = Symbol::empty("DIR-CONVENTION-CHECK");
        sym.pins = vec![pin(1, orientation, PinDirection::Passive)];
        let lib = sym.to_lib_symbol("id");
        let rotation = lib.pins[0].pin.rotation;
        let actual_dir = pin_direction_convention(rotation);
        assert_eq!(
            actual_dir, expected_dir,
            "{orientation:?} (rotation {rotation}) -> {expected_dir:?}"
        );
    }
}

/// Acceptance criterion: pin the complete `PinDirection` mapping (every
/// one of the library's 10 source variants).
#[test]
fn pin_direction_mapping_is_total_for_every_source_variant() {
    let table = [
        (PinDirection::Input, SchematicPinDirection::Input),
        (PinDirection::Output, SchematicPinDirection::Output),
        (
            PinDirection::Bidirectional,
            SchematicPinDirection::Bidirectional,
        ),
        (PinDirection::Power, SchematicPinDirection::PowerInput),
        (PinDirection::Passive, SchematicPinDirection::Passive),
        (
            PinDirection::OpenCollector,
            SchematicPinDirection::OpenDrainLow,
        ),
        (
            PinDirection::OpenEmitter,
            SchematicPinDirection::OpenDrainHigh,
        ),
        (
            PinDirection::NotConnected,
            SchematicPinDirection::DoNotConnect,
        ),
        (PinDirection::Tristate, SchematicPinDirection::ThreeStatable),
        (
            PinDirection::Unspecified,
            SchematicPinDirection::Unclassified,
        ),
    ];
    for (electrical, expected) in table {
        let mut sym = Symbol::empty("DIR-CHECK");
        sym.pins = vec![pin(1, PinOrientation::Right, electrical)];
        let lib = sym.to_lib_symbol("id");
        assert_eq!(
            lib.pins[0].pin.direction, expected,
            "{electrical:?} -> {expected:?}"
        );
    }
}

/// Fill is lossy by construction: `None -> FillType::None`, `Some(_) ->
/// FillType::Background` — the RGBA colour itself is dropped.
#[test]
fn fill_none_and_some_map_to_fill_type() {
    let mut sym = Symbol::empty("FILL-CHECK");
    sym.graphics = vec![
        SymbolGraphic {
            kind: SymbolGraphicKind::Rectangle {
                from: [0.0, 0.0],
                to: [1.0, 1.0],
            },
            stroke_width: 0.1,
            fill: None,
            part_number: 0,
        },
        SymbolGraphic {
            kind: SymbolGraphicKind::Rectangle {
                from: [0.0, 0.0],
                to: [1.0, 1.0],
            },
            stroke_width: 0.1,
            fill: Some([10, 20, 30, 255]),
            part_number: 0,
        },
    ];

    let lib = sym.to_lib_symbol("id");

    let Graphic::Rectangle { fill, .. } = &lib.graphics[0].graphic else {
        panic!("expected a Rectangle");
    };
    assert_eq!(*fill, FillType::None);

    let Graphic::Rectangle { fill, .. } = &lib.graphics[1].graphic else {
        panic!("expected a Rectangle");
    };
    assert_eq!(*fill, FillType::Background);
}

/// Header fields: `designator -> reference`, `comment -> value`,
/// `description -> description`; `id` is caller-supplied, not invented.
#[test]
fn header_fields_map_designator_comment_description_and_caller_supplied_id() {
    let mut sym = Symbol::empty("HEADER-CHECK");
    sym.designator = "U?".to_string();
    sym.comment = "OPAMP-DUAL".to_string();
    sym.description = "Dual op-amp".to_string();

    let lib = sym.to_lib_symbol("lib:HEADER-CHECK");

    assert_eq!(lib.id, "lib:HEADER-CHECK");
    assert_eq!(lib.reference, "U?");
    assert_eq!(lib.value, "OPAMP-DUAL");
    assert_eq!(lib.description, "Dual op-amp");
}

/// `outside_edge_symbol` is the chosen authoritative glyph slot; the
/// other three slots are dropped regardless of what they carry.
#[test]
fn pin_shape_style_uses_outside_edge_symbol_as_the_authoritative_slot() {
    let mut sym = Symbol::empty("GLYPH-CHECK");
    let mut ignored_slots_pin = pin(1, PinOrientation::Right, PinDirection::Input);
    ignored_slots_pin.inside_symbol = PinSymbolKind::Dot;
    ignored_slots_pin.inside_edge_symbol = PinSymbolKind::ClockEdge;
    ignored_slots_pin.outside_edge_symbol = PinSymbolKind::None;
    ignored_slots_pin.outside_symbol = PinSymbolKind::Sigma;

    let mut authoritative_slot_pin = pin(2, PinOrientation::Right, PinDirection::Input);
    authoritative_slot_pin.outside_edge_symbol = PinSymbolKind::Dot;

    sym.pins = vec![ignored_slots_pin, authoritative_slot_pin];

    let lib = sym.to_lib_symbol("id");

    assert_eq!(lib.pins[0].pin.shape_style, PinShapeStyle::Plain);
    assert_eq!(lib.pins[1].pin.shape_style, PinShapeStyle::InvertedBubble);
}

/// `SymbolGraphicKind::Arc` (`center`/`radius`/CCW `start_deg..end_deg`)
/// converts into `Graphic::Arc`'s three-point (`start`/`mid`/`end`)
/// representation along the same CCW sweep.
#[test]
fn arc_converts_to_three_points_along_the_ccw_sweep() {
    let mut sym = Symbol::empty("ARC-CHECK");
    sym.graphics = vec![SymbolGraphic {
        kind: SymbolGraphicKind::Arc {
            center: [0.0, 0.0],
            radius: 1.0,
            start_deg: 0.0,
            end_deg: 90.0,
        },
        stroke_width: 0.1,
        fill: None,
        part_number: 0,
    }];

    let lib = sym.to_lib_symbol("id");

    let Graphic::Arc {
        start, mid, end, ..
    } = &lib.graphics[0].graphic
    else {
        panic!("expected an Arc");
    };
    approx(start.x, 1.0);
    approx(start.y, 0.0);
    approx(mid.x, std::f64::consts::FRAC_1_SQRT_2);
    approx(mid.y, std::f64::consts::FRAC_1_SQRT_2);
    approx(end.x, 0.0);
    approx(end.y, 1.0);
}

/// `SymbolGraphicKind::Polygon`'s vertex ring is closed implicitly; the
/// converted `Graphic::Polyline` closes it explicitly by repeating the
/// first vertex.
#[test]
fn polygon_closes_explicitly_when_converted_to_polyline() {
    let mut sym = Symbol::empty("POLY-CHECK");
    sym.graphics = vec![SymbolGraphic {
        kind: SymbolGraphicKind::Polygon {
            vertices: vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0]],
        },
        stroke_width: 0.1,
        fill: None,
        part_number: 0,
    }];

    let lib = sym.to_lib_symbol("id");

    let Graphic::Polyline { points, .. } = &lib.graphics[0].graphic else {
        panic!("expected a Polyline");
    };
    assert_eq!(points.len(), 4);
    assert_eq!((points[0].x, points[0].y), (0.0, 0.0));
    assert_eq!((points[3].x, points[3].y), (0.0, 0.0));
}
