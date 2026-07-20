//! Unit tests for the symbol primitive + pin TSV codec.

use super::serde_tsv::*;
use super::*;

#[test]
fn symbol_json_roundtrip() {
    let s = Symbol {
        uuid: Uuid::now_v7(),
        name: "OPAMP-DUAL-8".into(),
        anchor: [0.0, 0.0],
        pins: vec![SymbolPin {
            number: "1".into(),
            name: "OUT_A".into(),
            electrical: PinDirection::Output,
            position: [0.0, 2.54],
            orientation: PinOrientation::Right,
            length: 2.54,
            description: String::new(),
            function: Vec::new(),
            pin_package_length: None,
            propagation_delay_ns: None,
            designator_visible: true,
            name_visible: true,
            inside_symbol: PinSymbolKind::None,
            inside_edge_symbol: PinSymbolKind::None,
            outside_edge_symbol: PinSymbolKind::None,
            outside_symbol: PinSymbolKind::None,
            hidden: false,
            locked: false,
            part_number: 1,
        }],
        graphics: vec![SymbolGraphic {
            kind: SymbolGraphicKind::Rectangle {
                from: [-2.5, -2.5],
                to: [2.5, 2.5],
            },
            stroke_width: 0.15,
            part_number: 0,
            fill: None,
        }],
        schematic_params: ParamMap::new(),
        designator: "U?".into(),
        comment: "*".into(),
        description: String::new(),
        component_type: ComponentType::Standard,
        mirrored: false,
        local_fill_color: None,
        local_line_color: None,
        local_pin_color: None,
        version: "0.0.1".into(),
        released: false,
        part_count: 1,
        created: Utc::now(),
        updated: Utc::now(),
    };
    let json = serde_json::to_string(&s).unwrap();
    let back: Symbol = serde_json::from_str(&json).unwrap();
    assert_eq!(s, back);
}

/// `SymbolFile::upsert` replaces a matching-uuid symbol in-place
/// and returns true; non-matching uuids return false so the
/// caller can `push` instead.
#[test]
fn symbol_file_upsert_replaces_matching_uuid() {
    let original = Symbol::empty("FIRST");
    let mut file = SymbolFile::from_symbol(original.clone());
    let mut updated = original.clone();
    updated.name = "FIRST_RENAMED".into();
    assert!(file.upsert(updated.clone()));
    assert_eq!(file.symbols.len(), 1);
    assert_eq!(file.symbols[0].name, "FIRST_RENAMED");

    let unrelated = Symbol::empty("OTHER");
    assert!(!file.upsert(unrelated));
    // Caller would push; we just verify upsert didn't accidentally add.
    assert_eq!(file.symbols.len(), 1);
}

// ---- v0.18.4 — SymbolFile TOML+TSV round-trip + pin TSV codec ----

#[test]
fn symbol_file_toml_round_trip_empty_symbol() {
    // `Symbol::empty` is fully empty — this exercises the
    // header-only TSV path.
    let s = Symbol::empty("Test");
    let original = SymbolFile::from_symbol(s.clone());
    let toml_text = original.to_toml_string().expect("serialise");
    let back = SymbolFile::from_toml_str(&toml_text).expect("parse");
    assert_eq!(back.symbols.len(), 1);
    assert_eq!(back.symbols[0].name, "Test");
    assert_eq!(back.symbols[0].pins.len(), 0);
    assert_eq!(back.format, "snxsym/v1");
    assert_eq!(back.file_uuid, original.file_uuid);
}

#[test]
fn symbol_file_toml_round_trip_multi() {
    let mut file = SymbolFile::from_symbol(Symbol::empty("A"));
    file.symbols.push(Symbol::empty("B"));
    file.symbols.push(Symbol::empty("C"));
    let toml_text = file.to_toml_string().expect("serialise");
    let back = SymbolFile::from_toml_str(&toml_text).expect("parse");
    assert_eq!(back.symbols.len(), 3);
    let names: Vec<&str> = back.symbols.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, vec!["A", "B", "C"]);
}

#[test]
fn symbol_file_from_bytes_decodes_toml_envelope() {
    let mut file = SymbolFile::from_symbol(Symbol::empty("TOML-A"));
    file.symbols.push(Symbol::empty("TOML-B"));
    let toml_bytes = file.to_toml_string().unwrap().into_bytes();
    let back = SymbolFile::from_bytes(&toml_bytes).expect("parse");
    assert_eq!(back.symbols.len(), 2);
}

#[test]
fn symbol_file_from_bytes_rejects_empty_payload() {
    match SymbolFile::from_bytes(b"   \n  \t\n") {
        Err(SymbolFileError::Empty) => {}
        other => panic!("expected Empty, got {other:?}"),
    }
}

/// All-fields round-trip — every SymbolPin field gets a non-default
/// value so the TSV cell encoders / decoders are exercised end-to-end.
#[test]
fn symbol_file_round_trip_with_full_pin_payload() {
    let pin = SymbolPin {
        number: "VCC".into(),
        name: "Power".into(),
        electrical: PinDirection::Power,
        position: [-3.81, 5.08],
        orientation: PinOrientation::Up,
        length: 2.54,
        description: "main rail".into(),
        function: vec!["VDD".into(), "VCC_3V3".into()],
        pin_package_length: Some(1.5),
        propagation_delay_ns: Some(0.25),
        designator_visible: false,
        name_visible: true,
        inside_symbol: PinSymbolKind::Dot,
        inside_edge_symbol: PinSymbolKind::ClockEdge,
        outside_edge_symbol: PinSymbolKind::ActiveLowInput,
        outside_symbol: PinSymbolKind::SchmittTrigger,
        hidden: true,
        locked: true,
        part_number: 2,
    };
    let mut sym = Symbol::empty("PWR");
    sym.pins = vec![pin.clone()];
    let file = SymbolFile::from_symbol(sym);
    let toml_text = file.to_toml_string().expect("serialise");
    let back = SymbolFile::from_toml_str(&toml_text).expect("parse");
    assert_eq!(back.symbols[0].pins.len(), 1);
    assert_eq!(back.symbols[0].pins[0], pin);
}

#[test]
fn symbol_file_to_toml_emits_pins_as_literal_multiline() {
    // Output must contain the `pins_tsv = '''` opener — placeholder
    // post-processing landed.
    let s = Symbol::empty("Demo");
    let toml_text = SymbolFile::from_symbol(s).to_toml_string().unwrap();
    assert!(
        toml_text.contains("pins_tsv = '''"),
        "expected literal multi-line opener; got:\n{toml_text}"
    );
    // ... and no leftover placeholder string.
    assert!(
        !toml_text.contains(PINS_TSV_PLACEHOLDER_PREFIX),
        "placeholder should be fully replaced; got:\n{toml_text}"
    );
}

#[test]
fn pins_to_tsv_empty_emits_header_only() {
    let tsv = pins_to_tsv(&[]).expect("serialise");
    // Header row terminated by a newline, no data rows.
    assert_eq!(tsv, format!("{}\n", PIN_TSV_COLUMNS.join("\t")));
}

#[test]
fn pins_to_tsv_rejects_tab_in_cell() {
    let mut pin = SymbolPin::new("1", "name");
    pin.description = "tab\there".into();
    match pins_to_tsv(std::slice::from_ref(&pin)) {
        Err(SymbolFileError::InvalidTsvCell { column, .. }) => {
            assert_eq!(column, "description");
        }
        other => panic!("expected InvalidTsvCell, got {other:?}"),
    }
}

#[test]
fn pins_to_tsv_rejects_newline_in_cell() {
    let mut pin = SymbolPin::new("1", "multi\nline");
    pin.description = String::new();
    match pins_to_tsv(std::slice::from_ref(&pin)) {
        Err(SymbolFileError::InvalidTsvCell { column, .. }) => {
            assert_eq!(column, "name");
        }
        other => panic!("expected InvalidTsvCell, got {other:?}"),
    }
}

#[test]
fn pins_to_tsv_rejects_triple_quote_in_cell() {
    let mut pin = SymbolPin::new("1", "X");
    pin.description = "smuggle '''".into();
    match pins_to_tsv(std::slice::from_ref(&pin)) {
        Err(SymbolFileError::InvalidTsvCell { column, .. }) => {
            assert_eq!(column, "description");
        }
        other => panic!("expected InvalidTsvCell, got {other:?}"),
    }
}

#[test]
fn pins_from_tsv_rejects_schema_mismatch() {
    // Header naming "wrong" columns triggers PinsTsvSchemaMismatch.
    let bad_tsv = "foo\tbar\tbaz\n1\t2\t3\n";
    match pins_from_tsv(bad_tsv) {
        Err(SymbolFileError::PinsTsvSchemaMismatch { got }) => {
            assert_eq!(got, vec!["foo", "bar", "baz"]);
        }
        other => panic!("expected PinsTsvSchemaMismatch, got {other:?}"),
    }
}

#[test]
fn pins_from_tsv_rejects_cell_count_mismatch() {
    let header = PIN_TSV_COLUMNS.join("\t");
    // 5 cells in a 20-column schema.
    let body = format!("{header}\n1\tname\tInput\t0\t0\n");
    match pins_from_tsv(&body) {
        Err(SymbolFileError::PinsTsvCellCountMismatch {
            row_index,
            got,
            expected,
        }) => {
            assert_eq!(row_index, 0);
            assert_eq!(got, 5);
            assert_eq!(expected, PIN_TSV_COLUMNS.len());
        }
        other => panic!("expected PinsTsvCellCountMismatch, got {other:?}"),
    }
}

#[test]
fn pin_direction_token_round_trip_all_variants() {
    for d in [
        PinDirection::Input,
        PinDirection::Output,
        PinDirection::Bidirectional,
        PinDirection::Power,
        PinDirection::Passive,
        PinDirection::OpenCollector,
        PinDirection::OpenEmitter,
        PinDirection::NotConnected,
        PinDirection::Tristate,
        PinDirection::Unspecified,
    ] {
        let token = pin_direction_token(d);
        let back = pin_direction_from_token(token).unwrap();
        assert_eq!(d, back);
    }
}

#[test]
fn pin_orientation_token_round_trip_all_variants() {
    for o in [
        PinOrientation::Up,
        PinOrientation::Down,
        PinOrientation::Left,
        PinOrientation::Right,
    ] {
        let token = pin_orientation_token(o);
        let back = pin_orientation_from_token(token).unwrap();
        assert_eq!(o, back);
    }
}

#[test]
fn pin_symbol_kind_token_round_trip_all_variants() {
    for k in [
        PinSymbolKind::None,
        PinSymbolKind::Dot,
        PinSymbolKind::ClockEdge,
        PinSymbolKind::ActiveLowInput,
        PinSymbolKind::ActiveLowOutput,
        PinSymbolKind::SchmittTrigger,
        PinSymbolKind::Analog,
        PinSymbolKind::Digital,
        PinSymbolKind::ShiftRight,
        PinSymbolKind::ShiftLeft,
        PinSymbolKind::Pi,
        PinSymbolKind::Sigma,
        PinSymbolKind::OpenCollector,
        PinSymbolKind::OpenEmitter,
        PinSymbolKind::HiZ,
    ] {
        let token = pin_symbol_kind_token(k);
        let back = pin_symbol_kind_from_token(token).unwrap();
        assert_eq!(k, back);
    }
}

#[test]
fn symbol_file_unsupported_format_token_is_rejected() {
    let bad = r#"
format = "snxsym/99"
file_uuid = "00000000-0000-0000-0000-000000000000"
display_name = ""
created = "2026-05-04T00:00:00Z"
updated = "2026-05-04T00:00:00Z"
symbols = []
"#;
    match SymbolFile::from_toml_str(bad) {
        Err(SymbolFileError::UnsupportedFormat { got }) => {
            assert_eq!(got, "snxsym/99");
        }
        other => panic!("expected UnsupportedFormat, got {other:?}"),
    }
}

#[test]
fn pin_electrical_type_round_trip_all_variants() {
    for t in [
        PinDirection::Input,
        PinDirection::Output,
        PinDirection::Bidirectional,
        PinDirection::Power,
        PinDirection::Passive,
        PinDirection::OpenCollector,
        PinDirection::OpenEmitter,
        PinDirection::NotConnected,
        PinDirection::Tristate,
        PinDirection::Unspecified,
    ] {
        let json = serde_json::to_string(&t).unwrap();
        let back: PinDirection = serde_json::from_str(&json).unwrap();
        assert_eq!(t, back);
    }
}

#[test]
fn pin_orientation_round_trip_all_variants() {
    for o in [
        PinOrientation::Up,
        PinOrientation::Down,
        PinOrientation::Left,
        PinOrientation::Right,
    ] {
        let json = serde_json::to_string(&o).unwrap();
        let back: PinOrientation = serde_json::from_str(&json).unwrap();
        assert_eq!(o, back);
    }
}

#[test]
fn symbol_graphic_kind_round_trip_each_variant() {
    let cases = [
        SymbolGraphicKind::Line {
            from: [0.0, 0.0],
            to: [1.0, 1.0],
        },
        SymbolGraphicKind::Rectangle {
            from: [0.0, 0.0],
            to: [1.0, 1.0],
        },
        SymbolGraphicKind::Circle {
            center: [0.0, 0.0],
            radius: 1.0,
        },
        SymbolGraphicKind::Arc {
            center: [0.0, 0.0],
            radius: 1.0,
            start_deg: 0.0,
            end_deg: 90.0,
        },
        SymbolGraphicKind::Text {
            position: [0.0, 0.0],
            content: "U1".into(),
            size: 1.27,
        },
        SymbolGraphicKind::Polygon {
            vertices: vec![[0.0, 0.0], [1.0, 0.0], [0.5, 1.0]],
        },
    ];
    for k in cases {
        let json = serde_json::to_string(&k).unwrap();
        let back: SymbolGraphicKind = serde_json::from_str(&json).unwrap();
        assert_eq!(k, back);
    }
}

#[test]
fn empty_symbol_starts_without_default_pins() {
    let s = Symbol::empty("test");
    assert_eq!(s.name, "test");
    assert_eq!(s.pins.len(), 0);
}

// ---- Phase B — part_count serialization back-compat ----

/// A declared multi-unit symbol with no pins keeps its `part_count`
/// across a TOML+TSV round-trip (the count is first-class, not derived
/// from pins alone).
#[test]
fn part_count_round_trips() {
    let mut s = Symbol::empty("X");
    s.part_count = 3;
    let file = SymbolFile::from_symbol(s);
    let toml_text = file.to_toml_string().expect("serialise");
    let back = SymbolFile::from_toml_str(&toml_text).expect("parse");
    assert_eq!(back.symbols[0].part_count, 3);
}

/// Legacy `.snxsym` files were written before `part_count` existed, so
/// they load with the serde default (`1`) even when pins live on higher
/// parts. The loader must reconcile the declared count upward from the
/// highest pin `part_number` so no populated unit is lost.
#[test]
fn legacy_file_reconciles_part_count() {
    // Reliable fallback path: a symbol whose declared `part_count` (1)
    // lags its highest pin part (3) — identical to a legacy file whose
    // missing field defaults to 1. Round-tripping must lift the count.
    let mut s = Symbol::empty("LEGACY");
    let mut pin = SymbolPin::new("1", "A");
    pin.part_number = 3;
    s.pins = vec![pin];
    s.part_count = 1;
    let file = SymbolFile::from_symbol(s);
    let toml_text = file.to_toml_string().expect("serialise");
    let back = SymbolFile::from_toml_str(&toml_text).expect("parse");
    assert!(back.symbols[0].part_count >= 3);
}

// ---- Phase C1 — SymbolGraphic.part_number serialization ----

/// A graphic scoped to a specific unit keeps its `part_number` across a
/// TOML+TSV round-trip — body geometry is now per-unit addressable.
#[test]
fn graphic_part_number_round_trips() {
    let mut s = Symbol::empty("X");
    s.graphics.push(SymbolGraphic {
        kind: SymbolGraphicKind::Rectangle {
            from: [-2.5, -2.5],
            to: [2.5, 2.5],
        },
        stroke_width: 0.15,
        part_number: 2,
        fill: None,
    });
    let file = SymbolFile::from_symbol(s);
    let toml_text = file.to_toml_string().expect("serialise");
    let back = SymbolFile::from_toml_str(&toml_text).expect("parse");
    assert_eq!(back.symbols[0].graphics[0].part_number, 2);
}

/// A graphic's fill colour survives a `.snxsym` save/load round-trip,
/// proving the new `SymbolGraphic.fill` field serialises through the
/// TOML manifest alongside the rest of the graphic. Back-compat for
/// legacy files missing the field is covered by the additive
/// `#[serde(default)]`, same as `graphic_missing_part_number_defaults_to_zero`.
#[test]
fn graphic_fill_round_trips() {
    let mut s = Symbol::empty("X");
    s.graphics.push(SymbolGraphic {
        kind: SymbolGraphicKind::Rectangle {
            from: [-2.5, -2.5],
            to: [2.5, 2.5],
        },
        stroke_width: 0.15,
        part_number: 0,
        fill: Some([220, 60, 60, 255]),
    });
    let file = SymbolFile::from_symbol(s);
    let toml_text = file.to_toml_string().expect("serialise");
    let back = SymbolFile::from_toml_str(&toml_text).expect("parse");
    assert_eq!(back.symbols[0].graphics[0].fill, Some([220, 60, 60, 255]));
}

/// A graphic left at the default (`0` = shared / drawn on every unit)
/// reloads as `0` — proves the additive, back-compatible default so
/// pre-C1 files whose graphics carried no part scoping render as before.
#[test]
fn graphic_missing_part_number_defaults_to_zero() {
    let mut s = Symbol::empty("X");
    s.graphics.push(SymbolGraphic {
        kind: SymbolGraphicKind::Rectangle {
            from: [-2.5, -2.5],
            to: [2.5, 2.5],
        },
        stroke_width: 0.15,
        part_number: 0,
        fill: None,
    });
    let file = SymbolFile::from_symbol(s);
    let toml_text = file.to_toml_string().expect("serialise");
    let back = SymbolFile::from_toml_str(&toml_text).expect("parse");
    assert_eq!(back.symbols[0].graphics[0].part_number, 0);
}

// ---- Polygon graphic — new tagged variant, conditional v2 bump ----

/// A `Polygon` graphic's vertices, fill, and stroke width all survive
/// a `.snxsym` save/load round-trip. A new tagged VARIANT (unlike a
/// `#[serde(default)]` field addition) is backward-compat only — an
/// old build can't deserialize an externally-tagged `kind = "polygon"`
/// it doesn't know about — so a file containing one is written as
/// `SYMBOL_FILE_FORMAT_TOKEN_V2` ("snxsym/v2"), not v1.
#[test]
fn polygon_graphic_round_trips() {
    let mut s = Symbol::empty("X");
    s.graphics.push(SymbolGraphic {
        kind: SymbolGraphicKind::Polygon {
            vertices: vec![[-2.5, -1.0], [2.5, -1.0], [2.5, 1.0], [-2.5, 1.0]],
        },
        stroke_width: 0.2,
        part_number: 1,
        fill: Some([30, 144, 255, 255]),
    });
    let file = SymbolFile::from_symbol(s);
    let toml_text = file.to_toml_string().expect("serialise");
    let back = SymbolFile::from_toml_str(&toml_text).expect("parse");
    assert_eq!(back.format, "snxsym/v2");
    let g = &back.symbols[0].graphics[0];
    match &g.kind {
        SymbolGraphicKind::Polygon { vertices } => {
            assert_eq!(
                vertices,
                &vec![[-2.5, -1.0], [2.5, -1.0], [2.5, 1.0], [-2.5, 1.0]]
            );
        }
        other => panic!("expected Polygon, got {other:?}"),
    }
    assert_eq!(g.fill, Some([30, 144, 255, 255]));
    assert_eq!(g.stroke_width, 0.2);
}

/// A file with no `Polygon` graphic anywhere stays on
/// `SYMBOL_FILE_FORMAT_TOKEN` ("snxsym/v1") — maximum compat: builds
/// that predate the `Polygon` variant can still read it.
#[test]
fn symbol_file_with_no_polygon_graphic_stays_v1() {
    let mut s = Symbol::empty("X");
    s.graphics.push(SymbolGraphic {
        kind: SymbolGraphicKind::Rectangle {
            from: [0.0, 0.0],
            to: [1.0, 1.0],
        },
        stroke_width: 0.15,
        part_number: 0,
        fill: None,
    });
    let file = SymbolFile::from_symbol(s);
    let toml_text = file.to_toml_string().expect("serialise");
    let back = SymbolFile::from_toml_str(&toml_text).expect("parse");
    assert_eq!(back.format, "snxsym/v1");
}

// ---- Arc CCW-wraparound load migration ----

fn arc_symbol(start_deg: f64, end_deg: f64) -> Symbol {
    let mut s = Symbol::empty("X");
    s.graphics.push(SymbolGraphic {
        kind: SymbolGraphicKind::Arc {
            center: [0.0, 0.0],
            radius: 5.0,
            start_deg,
            end_deg,
        },
        stroke_width: 0.15,
        part_number: 0,
        fill: None,
    });
    s
}

/// A legacy CW-signed pair (a raw drag delta, never reconciled into
/// `[0, 360)`) migrates on load: swapped and reduced, reproducing the
/// short arc a pre-normalization build's signed CPU draw actually
/// showed the user.
#[test]
fn arc_legacy_cw_signed_pair_migrates_to_ccw_swap_on_load() {
    let file = SymbolFile::from_symbol(arc_symbol(30.0, -60.0));
    let toml_text = file.to_toml_string().expect("serialise");
    let back = SymbolFile::from_toml_str(&toml_text).expect("parse");
    match &back.symbols[0].graphics[0].kind {
        SymbolGraphicKind::Arc {
            start_deg, end_deg, ..
        } => {
            assert_eq!(*start_deg, 300.0);
            assert_eq!(*end_deg, 30.0);
        }
        other => panic!("expected Arc, got {other:?}"),
    }
}

/// A wrapped pair with both endpoints already in `[0, 360)` — the
/// form `rotation.rs`'s Arc rotate transform has always produced for
/// a 0°-crossing arc, meaning wraparound from day one — loads
/// unchanged.
#[test]
fn arc_wraparound_pair_already_in_range_is_unchanged_on_load() {
    let file = SymbolFile::from_symbol(arc_symbol(330.0, 30.0));
    let toml_text = file.to_toml_string().expect("serialise");
    let back = SymbolFile::from_toml_str(&toml_text).expect("parse");
    match &back.symbols[0].graphics[0].kind {
        SymbolGraphicKind::Arc {
            start_deg, end_deg, ..
        } => {
            assert_eq!(*start_deg, 330.0);
            assert_eq!(*end_deg, 30.0);
        }
        other => panic!("expected Arc, got {other:?}"),
    }
}

/// The migration is a load-time fixed point: a second load->save->load
/// cycle must not drift the already-migrated value any further.
#[test]
fn arc_migration_is_a_load_time_fixed_point() {
    let file = SymbolFile::from_symbol(arc_symbol(30.0, -60.0));
    let first_load = SymbolFile::from_toml_str(&file.to_toml_string().unwrap()).unwrap();
    let second_load = SymbolFile::from_toml_str(&first_load.to_toml_string().unwrap()).unwrap();
    assert_eq!(
        first_load.symbols[0].graphics[0].kind,
        second_load.symbols[0].graphics[0].kind
    );
}

#[test]
fn normalize_arc_endpoints_deg_swaps_a_cw_signed_pair() {
    assert_eq!(normalize_arc_endpoints_deg(30.0, -60.0), (300.0, 30.0));
}

#[test]
fn normalize_arc_endpoints_deg_leaves_ccw_pairs_unswapped() {
    assert_eq!(normalize_arc_endpoints_deg(10.0, 100.0), (10.0, 100.0));
}

// ---- Full-turn Arc -> Circle load migration ----

/// An exact, nonzero 360° span (legacy full-circle authoring, or a
/// drag/rotation that landed on exactly one full turn) migrates to a
/// `Circle` on load instead of computing a zero CCW-wraparound sweep
/// and drawing nothing.
#[test]
fn arc_full_turn_migrates_to_circle_on_load() {
    let mut s = Symbol::empty("X");
    s.graphics.push(SymbolGraphic {
        kind: SymbolGraphicKind::Arc {
            center: [1.0, 2.0],
            radius: 5.0,
            start_deg: 0.0,
            end_deg: 360.0,
        },
        stroke_width: 0.2,
        part_number: 1,
        fill: None,
    });
    let file = SymbolFile::from_symbol(s);
    let toml_text = file.to_toml_string().expect("serialise");
    let back = SymbolFile::from_toml_str(&toml_text).expect("parse");
    match &back.symbols[0].graphics[0].kind {
        SymbolGraphicKind::Circle { center, radius } => {
            assert_eq!(*center, [1.0, 2.0]);
            assert_eq!(*radius, 5.0);
        }
        other => panic!("expected Circle, got {other:?}"),
    }
}

/// A negative exact-360° span also converts (the full-turn check runs
/// before the CW-signed swap, which would otherwise collapse this to
/// a degenerate `start == end` point-arc instead of a visible circle).
#[test]
fn arc_negative_full_turn_migrates_to_circle_on_load() {
    let file = SymbolFile::from_symbol(arc_symbol(90.0, -270.0));
    let toml_text = file.to_toml_string().expect("serialise");
    let back = SymbolFile::from_toml_str(&toml_text).expect("parse");
    assert!(matches!(
        back.symbols[0].graphics[0].kind,
        SymbolGraphicKind::Circle { .. }
    ));
}

/// A near-360° (but not exact) span is a real, valid arc — not a
/// full-turn — and stays an `Arc`.
#[test]
fn arc_near_full_turn_stays_an_arc_on_load() {
    let file = SymbolFile::from_symbol(arc_symbol(0.0, 350.0));
    let toml_text = file.to_toml_string().expect("serialise");
    let back = SymbolFile::from_toml_str(&toml_text).expect("parse");
    assert!(matches!(
        back.symbols[0].graphics[0].kind,
        SymbolGraphicKind::Arc { .. }
    ));
}
