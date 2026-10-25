//! Round-trip / serialization tests for the `.snxsch` / `.snxpcb`
//! wire format, including the issue-#96 save-corruption regressions.
//! Pure code motion out of `mod.rs`; the assertions are unchanged —
//! this suite is the data-loss guard for the format layer.

use super::tsv::{decode_cell, encode_cell, escape_tsv_body_for_toml};
use super::*;
use crate::pcb::{
    Footprint, Pad, PadNet, PadShape, PadType, PcbBoard, Point as PcbPoint, Segment, Via, ViaType,
    Zone,
};
use crate::schematic::{
    HAlign, Junction, Label, LabelType as LType, LibSymbol, Point as SchPoint, SchematicSheet,
    Symbol, VAlign, Wire,
};
use std::collections::BTreeMap;
use uuid::Uuid;

fn assert_btree_map<K, V>(_: &BTreeMap<K, V>) {}

pub(super) fn empty_sheet() -> SchematicSheet {
    SchematicSheet {
        version: 1,
        generator: "signex-test".into(),
        generator_version: "0.9".into(),
        paper_size: "A4".into(),
        ..Default::default()
    }
}

pub(super) fn empty_board() -> PcbBoard {
    PcbBoard {
        unknown: Default::default(),
        uuid: Uuid::nil(),
        version: 1,
        generator: "signex-test".into(),
        thickness: 1.6,
        outline: vec![],
        layers: vec![],
        setup: None,
        nets: vec![],
        footprints: vec![],
        segments: vec![],
        vias: vec![],
        zones: vec![],
        graphics: vec![],
        texts: vec![],
    }
}

#[test]
fn snxsch_round_trip_empty() {
    let snx = SnxSchematic::new(empty_sheet());
    let s = snx.write_string().expect("serialise");
    assert!(s.contains("format = \"snxsch/1\""));
    let back = SnxSchematic::parse(&s).expect("round-trip");
    assert_eq!(back.format, SNXSCH_FORMAT_V1);
    assert!(back.sheet.symbols.is_empty());
}

// ── Save-corruption regressions (issue #96) ──────────────────────

#[test]
fn encode_decode_cell_round_trips_dangerous_characters() {
    // Each of these previously either corrupted the TOML envelope
    // (backslash, quote) or silently dropped data (bare '-').
    for original in [
        "",
        "-",
        "C:\\Users\\x",        // Windows path — backslash
        "1/4\"",               // inch mark — trailing quote
        "he said \"hi\"",      // embedded quotes
        "a\tb",                // tab
        "line1\nline2",        // embedded newline
        "back\\slash \"q\" -", // mixed
        "plain",
        "with space",
    ] {
        let round = decode_cell(&encode_cell(original));
        assert_eq!(round, original, "cell round-trip failed for {original:?}");
    }
}

#[test]
fn escape_tsv_body_for_toml_never_leaves_a_triple_quote_run() {
    // A cell containing a single quote encodes to a 4-quote run
    // (`""""`), which would otherwise terminate the `"""` block.
    let escaped = escape_tsv_body_for_toml("x  \"\"\"\"  y\n");
    assert!(
        !escaped.contains("\"\"\""),
        "escaped body must not contain a raw ''' run: {escaped:?}"
    );
}

fn label_with_text(text: &str) -> Label {
    Label {
        unknown_columns: Default::default(),
        uuid: Uuid::nil(),
        text: text.to_string(),
        position: SchPoint { x: 0.0, y: 0.0 },
        rotation: 0.0,
        label_type: LType::Net,
        shape: String::new(),
        font_size: 1.27,
        justify: Default::default(),
        justify_v: Default::default(),
    }
}

#[test]
fn non_finite_rotation_fails_the_write_instead_of_saving_an_unloadable_file() {
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut sheet = empty_sheet();
        let mut sym = sample_symbol();
        sym.rotation = bad;
        sheet.symbols.push(sym);
        match SnxSchematic::new(sheet).write_string() {
            Err(FormatError::NonFiniteField { field, row, .. }) => {
                assert_eq!(field, "rotation");
                assert_eq!(row, 0);
            }
            other => panic!("rotation={bad}: expected NonFiniteField, got {other:?}"),
        }
    }
}

#[test]
fn snxsch_survives_backslash_and_quote_in_label_text() {
    // Backslash + trailing quote in a label used to produce a file
    // that `SnxSchematic::parse` could never reopen.
    let mut sheet = empty_sheet();
    sheet.labels.push(label_with_text("NET_C:\\x \"1/4\""));
    let s = SnxSchematic::new(sheet).write_string().expect("serialise");
    let back = SnxSchematic::parse(&s).expect("dangerous label text must not corrupt the file");
    assert_eq!(back.sheet.labels.len(), 1);
    assert_eq!(back.sheet.labels[0].text, "NET_C:\\x \"1/4\"");
}

#[test]
fn snxsch_survives_c0_control_bytes_in_label_text() {
    // A C0 control byte other than tab/newline/CR (BEL, VT, FF here)
    // used to pass through `escape_tsv_body_for_toml` raw, producing
    // a `.snxsch` that saved fine but `toml::from_str` rejected on
    // reopen — total document loss (#386).
    let text = "BEL\u{0007}VT\u{000B}FF\u{000C}end";
    let mut sheet = empty_sheet();
    sheet.labels.push(label_with_text(text));
    let s = SnxSchematic::new(sheet).write_string().expect("serialise");

    // Must be valid TOML on its own terms, not just parseable by our
    // own (possibly too-lenient) reader.
    toml::from_str::<toml::Value>(&s).expect("emitted file must be valid TOML");

    let back = SnxSchematic::parse(&s).expect("C0 control bytes must not corrupt the file");
    assert_eq!(back.sheet.labels.len(), 1);
    assert_eq!(back.sheet.labels[0].text, text);
}

#[test]
fn snxsch_does_not_drop_a_literal_dash_label() {
    let mut sheet = empty_sheet();
    sheet.labels.push(label_with_text("-"));
    let s = SnxSchematic::new(sheet).write_string().unwrap();
    let back = SnxSchematic::parse(&s).unwrap();
    assert_eq!(
        back.sheet.labels[0].text, "-",
        "a literal '-' must survive round-trip, not become empty"
    );
}

#[test]
fn snxsch_survives_embedded_newline_in_label_text() {
    let mut sheet = empty_sheet();
    sheet.labels.push(label_with_text("line1\nline2"));
    let s = SnxSchematic::new(sheet).write_string().unwrap();
    let back = SnxSchematic::parse(&s).unwrap();
    assert_eq!(back.sheet.labels[0].text, "line1\nline2");
}

#[test]
fn snxpcb_round_trip_empty() {
    let snx = SnxPcb::new(empty_board());
    let s = snx.write_string().expect("serialise");
    assert!(s.contains(&format!("format = \"{SNXPCB_FORMAT_V1}\"")));
    let back = SnxPcb::parse(&s).expect("round-trip");
    assert_eq!(back.format, SNXPCB_FORMAT_V1);
}

#[test]
fn snxpcb_keeps_setup_when_stackup_has_no_layers() {
    let mut board = empty_board();
    board.setup = Some(crate::pcb::PcbSetup {
        grid_size: 0.5,
        clearance: 0.25,
        trace_width: 0.2,
        via_diameter: 0.6,
        via_drill: 0.3,
        track_min_width: 0.1,
        via_min_diameter: 0.4,
        via_min_drill: 0.2,
    });
    let s = SnxPcb::new(board).write_string().expect("serialise");
    let back = SnxPcb::parse(&s).expect("round-trip");
    assert!(back.board.layers.is_empty());
    let setup = back.board.setup.expect("setup survives");
    assert_eq!(setup.grid_size, 0.5);
    assert_eq!(setup.clearance, 0.25);
}

#[test]
fn rejects_wrong_format_version() {
    // Hand-craft a TOML document with an unsupported version token.
    let bad = "format = \"snxsch/99\"\nschematic_id = \"00000000-0000-0000-0000-000000000000\"\n";
    let err = SnxSchematic::parse(bad).expect_err("must reject");
    match err {
        FormatError::UnsupportedVersion { found, expected } => {
            assert_eq!(found, "snxsch/99");
            assert_eq!(expected, SNXSCH_FORMAT_V1);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn rejects_wrong_pcb_format_version() {
    let bad = "format = \"snxpcb/99\"\npcb_id = \"00000000-0000-0000-0000-000000000000\"\n";
    let err = SnxPcb::parse(bad).expect_err("must reject");
    assert!(matches!(err, FormatError::UnsupportedVersion { .. }));
}

#[test]
fn snxsch_includes_tsv_blocks_substring() {
    let snx = SnxSchematic::new(empty_sheet());
    let s = snx.write_string().unwrap();
    assert!(s.contains("[sheets.components]\ncontent = \"\"\""));
    assert!(s.contains("[sheets.wires]\ncontent = \"\"\""));
    assert!(s.contains("[sheets.junctions]\ncontent = \"\"\""));
    assert!(s.contains("[sheets.labels]\ncontent = \"\"\""));
}

#[test]
fn snxpcb_includes_tsv_blocks_substring() {
    let snx = SnxPcb::new(empty_board());
    let s = snx.write_string().unwrap();
    assert!(s.contains("[footprints]\ncontent = \"\"\""));
    assert!(s.contains("[pads]\ncontent = \"\"\""));
    assert!(s.contains("[tracks]\ncontent = \"\"\""));
    assert!(s.contains("[vias]\ncontent = \"\"\""));
}

pub(super) fn sample_symbol() -> Symbol {
    Symbol {
        uuid: Uuid::parse_str("0192a8c0-0001-7000-8000-000000000001").unwrap(),
        lib_id: "lm2596.snxsym".to_string(),
        reference: "U1".to_string(),
        value: "LM2596".to_string(),
        footprint: "TO-263.snxfpt".to_string(),
        position: SchPoint { x: 50.8, y: 25.4 },
        ..Default::default()
    }
}

fn sample_wire(sx: f64, sy: f64, ex: f64, ey: f64) -> Wire {
    Wire {
        uuid: Uuid::new_v4(),
        start: SchPoint { x: sx, y: sy },
        end: SchPoint { x: ex, y: ey },
        ..Default::default()
    }
}

#[test]
fn show_serialised_pcb_for_inspection() {
    // diagnostic — emits a human-readable serialisation of a small
    // PCB so reviewers can eyeball the on-disk shape. Intentionally
    // doesn't assert anything; the round-trip tests cover parity.
    let mut board = empty_board();
    board.layers = vec![
        crate::pcb::LayerDef {
            id: 0,
            name: "TopCopper".into(),
            layer_type: "copper".into(),
        },
        crate::pcb::LayerDef {
            id: 1,
            name: "BottomCopper".into(),
            layer_type: "copper".into(),
        },
    ];
    board.footprints.push(Footprint {
        unknown_columns: Default::default(),
        unknown: Default::default(),
        uuid: Uuid::parse_str("0192a8c0-0010-7000-8000-000000000001").unwrap(),
        reference: "U1".into(),
        value: "STM32F407".into(),
        footprint_id: "stm32f407.snxfpt".into(),
        position: PcbPoint { x: 50.0, y: 25.0 },
        rotation: 0.0,
        layer: "TopCopper".into(),
        locked: false,
        pads: vec![],
        graphics: vec![],
        properties: vec![],
    });
    let snx = SnxPcb::new(board);
    let _ = snx.write_string().expect("serialise");
}

#[test]
fn snxsch_round_trip_with_data() {
    let mut sheet = empty_sheet();
    sheet.symbols.push(sample_symbol());
    sheet.wires.push(sample_wire(10.0, 20.0, 30.0, 20.0));
    sheet.wires.push(sample_wire(30.0, 20.0, 30.0, 40.0));
    sheet.junctions.push(Junction {
        uuid: Uuid::parse_str("0192a8c0-0002-7000-8000-000000000001").unwrap(),
        position: SchPoint { x: 30.0, y: 20.0 },
        diameter: 0.5,
        minted: true,
        ..Default::default()
    });
    sheet.labels.push(Label {
        unknown_columns: Default::default(),
        uuid: Uuid::parse_str("0192a8c0-0003-7000-8000-000000000001").unwrap(),
        text: "VIN".to_string(),
        position: SchPoint { x: 10.0, y: 20.0 },
        rotation: 0.0,
        label_type: LType::Net,
        shape: String::new(),
        font_size: 1.27,
        justify: HAlign::Left,
        justify_v: VAlign::Bottom,
    });

    let snx = SnxSchematic::new(sheet.clone());
    let serialised = snx.write_string().expect("serialise");

    let back = SnxSchematic::parse(&serialised).expect("round-trip");
    assert_eq!(back.sheet.symbols.len(), 1);
    assert_eq!(back.sheet.symbols[0].reference, "U1");
    assert_eq!(back.sheet.symbols[0].lib_id, "lm2596.snxsym");
    assert_eq!(back.sheet.symbols[0].value, "LM2596");
    assert!((back.sheet.symbols[0].position.x - 50.8).abs() < 1e-6);
    assert!((back.sheet.symbols[0].position.y - 25.4).abs() < 1e-6);
    assert_eq!(back.sheet.symbols[0].footprint, "TO-263.snxfpt");

    assert_eq!(back.sheet.wires.len(), 2);
    assert!((back.sheet.wires[0].start.x - 10.0).abs() < 1e-6);
    assert!((back.sheet.wires[1].end.y - 40.0).abs() < 1e-6);

    assert_eq!(back.sheet.junctions.len(), 1);
    assert_eq!(back.sheet.junctions[0].diameter, 0.5);
    assert!(
        back.sheet.junctions[0].minted,
        "minted provenance must survive the extras round-trip"
    );

    assert_eq!(back.sheet.labels.len(), 1);
    assert_eq!(back.sheet.labels[0].text, "VIN");
    assert_eq!(back.sheet.labels[0].label_type, LType::Net);

    // encode -> decode -> encode must be stable (issue #422 added a new
    // persisted field; a re-serialise of the round-tripped sheet must not
    // drift byte-for-byte from the original).
    let re_serialised = SnxSchematic::new(back.sheet)
        .write_string()
        .expect("re-serialise");
    assert_eq!(
        serialised, re_serialised,
        "encode -> decode -> encode must be stable"
    );
}

#[test]
fn snxsch_refuses_non_finite_extras_with_key_path() {
    // #924 — TOML accepts `nan`/`inf`, writers refuse them, so a file
    // carrying one must be refused at load with the key path named.
    let good = SnxSchematic::new(empty_sheet())
        .write_string()
        .expect("serialise");
    assert!(
        SnxSchematic::parse(&good).is_ok(),
        "finite file still loads"
    );
    for bad in ["nan", "inf", "-inf"] {
        let text = format!("{good}\n[extras.probe]\nvalues = [1.0, {bad}]\n");
        match SnxSchematic::parse(&text) {
            Err(FormatError::NonFiniteNumber { path }) => {
                assert_eq!(path, "extras.probe.values[1]")
            }
            other => panic!("{bad}: expected NonFiniteNumber, got {other:?}"),
        }
    }
}

/// A `.snxsch` written before issue #422 added `Junction::minted` never
/// emitted `[extras.junctions.<uuid>]`. Loading one must not error and
/// must treat every junction it names as user-placed (never auto-removed),
/// not as an unminted/undefined provenance.
#[test]
fn snxsch_without_junction_extras_defaults_to_user_placed() {
    let mut sheet = empty_sheet();
    sheet.junctions.push(Junction {
        uuid: Uuid::parse_str("0192a8c0-0002-7000-8000-000000000002").unwrap(),
        position: SchPoint { x: 5.0, y: 0.0 },
        minted: true,
        ..Default::default()
    });

    let full = SnxSchematic::new(sheet).write_string().expect("serialise");

    // Simulate the pre-#422 writer: strip the `[extras...]` tail this
    // writer added for the minted dot, leaving only what an older Signex
    // would have written for the same sheet.
    let legacy = full
        .split("\n[extras")
        .next()
        .expect("split always yields the head")
        .to_string();
    assert!(
        !legacy.contains("[extras"),
        "test fixture must actually drop the extras tail"
    );

    let back = SnxSchematic::parse(&legacy).expect("legacy file still loads");
    assert_eq!(back.sheet.junctions.len(), 1);
    assert!(
        !back.sheet.junctions[0].minted,
        "a junction with no extras entry must default to user-placed, never minted"
    );
}

#[test]
fn snxpcb_round_trip_with_data() {
    let mut board = empty_board();

    let pad1 = Pad {
        unknown_columns: Default::default(),
        unknown: Default::default(),
        uuid: Uuid::parse_str("0192a8c0-0011-7000-8000-000000000001").unwrap(),
        number: "1".into(),
        pad_type: PadType::Smd,
        shape: PadShape::RoundRect,
        position: PcbPoint { x: 50.5, y: 25.0 },
        size: PcbPoint { x: 1.0, y: 0.6 },
        drill: None,
        layers: vec!["TopCopper".into()],
        net: Some(PadNet {
            number: 1,
            name: "VCC".into(),
        }),
        roundrect_ratio: 0.25,
    };
    let pad2 = Pad {
        unknown_columns: Default::default(),
        unknown: Default::default(),
        uuid: Uuid::parse_str("0192a8c0-0011-7000-8000-000000000002").unwrap(),
        number: "2".into(),
        pad_type: PadType::Smd,
        shape: PadShape::RoundRect,
        position: PcbPoint { x: 51.5, y: 25.0 },
        size: PcbPoint { x: 1.0, y: 0.6 },
        drill: None,
        layers: vec!["TopCopper".into()],
        net: Some(PadNet {
            number: 2,
            name: "GND".into(),
        }),
        roundrect_ratio: 0.25,
    };

    let footprint = Footprint {
        unknown_columns: Default::default(),
        unknown: Default::default(),
        uuid: Uuid::parse_str("0192a8c0-0010-7000-8000-000000000001").unwrap(),
        reference: "U1".into(),
        value: "STM32F407".into(),
        footprint_id: "stm32f407.snxfpt".into(),
        position: PcbPoint { x: 50.0, y: 25.0 },
        rotation: 0.0,
        layer: "TopCopper".into(),
        locked: false,
        pads: vec![pad1, pad2],
        graphics: Vec::new(),
        properties: Vec::new(),
    };
    board.footprints.push(footprint);

    for (uuid, sx, sy, ex, ey) in [
        (
            "0192a8c0-0020-7000-8000-000000000001",
            100.0,
            200.0,
            150.0,
            200.0,
        ),
        (
            "0192a8c0-0020-7000-8000-000000000002",
            150.0,
            200.0,
            200.0,
            200.0,
        ),
        (
            "0192a8c0-0020-7000-8000-000000000003",
            200.0,
            200.0,
            300.0,
            200.0,
        ),
    ] {
        board.segments.push(Segment {
            unknown_columns: Default::default(),
            uuid: Uuid::parse_str(uuid).unwrap(),
            start: PcbPoint { x: sx, y: sy },
            end: PcbPoint { x: ex, y: ey },
            width: 0.254,
            layer: "BottomCopper".into(),
            net: 1,
        });
    }

    board.vias.push(Via {
        unknown_columns: Default::default(),
        uuid: Uuid::parse_str("0192a8c0-0030-7000-8000-000000000001").unwrap(),
        position: PcbPoint { x: 100.0, y: 200.0 },
        diameter: 0.6,
        drill: 0.3,
        layers: vec!["TopCopper".into(), "BottomCopper".into()],
        net: 1,
        via_type: ViaType::Through,
    });

    board.zones.push(Zone {
        uuid: Uuid::parse_str("0192a8c0-0040-7000-8000-000000000001").unwrap(),
        net: 2,
        net_name: "GND".into(),
        layer: "BottomCopper".into(),
        outline: vec![
            PcbPoint { x: 10.0, y: 20.0 },
            PcbPoint { x: 30.0, y: 20.0 },
            PcbPoint { x: 30.0, y: 40.0 },
            PcbPoint { x: 10.0, y: 40.0 },
        ],
        holes: vec![vec![
            PcbPoint { x: 15.0, y: 25.0 },
            PcbPoint { x: 20.0, y: 25.0 },
            PcbPoint { x: 20.0, y: 30.0 },
        ]],
        priority: 0,
        fill_type: String::new(),
        thermal_relief: false,
        thermal_gap: 0.0,
        thermal_width: 0.0,
        clearance: 0.0,
        min_thickness: 0.0,
        unknown: Default::default(),
    });

    let snx = SnxPcb::new(board.clone());
    let serialised = snx.write_string().expect("serialise");
    let back = SnxPcb::parse(&serialised).expect("round-trip");

    assert_eq!(back.board.footprints.len(), 1);
    assert_eq!(back.board.footprints[0].reference, "U1");
    assert_eq!(back.board.footprints[0].pads.len(), 2);
    assert_eq!(back.board.footprints[0].pads[0].number, "1");
    assert_eq!(
        back.board.footprints[0].pads[0].net.as_ref().unwrap().name,
        "VCC"
    );

    assert_eq!(back.board.segments.len(), 3);
    assert!((back.board.segments[0].width - 0.254).abs() < 1e-6);
    assert_eq!(back.board.segments[0].layer, "BottomCopper");

    assert_eq!(back.board.vias.len(), 1);
    assert!((back.board.vias[0].diameter - 0.6).abs() < 1e-6);
    assert!((back.board.vias[0].drill - 0.3).abs() < 1e-6);

    assert_eq!(back.board.zones.len(), 1);
    assert_eq!(back.board.zones[0].net_name, "GND");
    assert_eq!(back.board.zones[0].outline.len(), 4);
    assert_eq!(back.board.zones[0].holes, board.zones[0].holes);
}

/// #818: a zone without holes writes no `holes` key, so files saved before
/// the field existed re-save byte-identically, and they still parse.
#[test]
fn zone_without_holes_writes_no_holes_key() {
    let mut board = empty_board();
    board.zones.push(Zone {
        uuid: Uuid::parse_str("0192a8c0-0040-7000-8000-000000000002").unwrap(),
        net: 0,
        net_name: String::new(),
        layer: "TopCopper".into(),
        outline: vec![
            PcbPoint { x: 0.0, y: 0.0 },
            PcbPoint { x: 1.0, y: 0.0 },
            PcbPoint { x: 1.0, y: 1.0 },
        ],
        holes: Vec::new(),
        priority: 0,
        fill_type: String::new(),
        thermal_relief: false,
        thermal_gap: 0.0,
        thermal_width: 0.0,
        clearance: 0.0,
        min_thickness: 0.0,
        unknown: Default::default(),
    });
    let text = SnxPcb::new(board).write_string().expect("serialise");
    assert!(!text.contains("holes"), "{text}");
    let back = SnxPcb::parse(&text).expect("parse");
    assert!(back.board.zones[0].holes.is_empty());
}

#[test]
fn tsv_writer_pads_columns_for_legibility() {
    let rows = vec![
        SchJunctionRow {
            unknown_columns: Default::default(),
            uuid: Uuid::nil(),
            pos_x: 100,
            pos_y: 200,
            diameter: 0.5,
        },
        SchJunctionRow {
            unknown_columns: Default::default(),
            uuid: Uuid::nil(),
            pos_x: 30000000,
            pos_y: 40000000,
            diameter: 0.5,
        },
    ];
    let body = write_tsv_block("t", &rows).unwrap();
    let lines: Vec<&str> = body.lines().collect();
    assert_eq!(lines[0].split_whitespace().next().unwrap(), "uuid");
    // header columns "pos_x" "pos_y" "diameter" preserved
    assert!(lines[0].contains("pos_x"));
    assert!(lines[0].contains("diameter"));
    // round-trip
    let parsed: Vec<SchJunctionRow> = parse_tsv_block("sheets.junctions", &body).unwrap();
    assert_eq!(parsed.len(), 2);
    assert_eq!(parsed[0].pos_x, 100);
    assert_eq!(parsed[1].pos_x, 30000000);
}

#[test]
fn tsv_parser_rejects_header_mismatch() {
    let body = "uuid pos_x pos_y wrong_column\n";
    let err = parse_tsv_block::<SchJunctionRow>("sheets.junctions", body).unwrap_err();
    assert!(matches!(err, FormatError::TsvHeaderMismatch { .. }));
}

#[test]
fn tsv_parser_rejects_cell_count_mismatch() {
    let body = "uuid  pos_x  pos_y  diameter\n00000000-0000-0000-0000-000000000000  100  200\n";
    let err = parse_tsv_block::<SchJunctionRow>("sheets.junctions", body).unwrap_err();
    assert!(matches!(err, FormatError::TsvCellCountMismatch { .. }));
}

#[test]
fn integer_nanometre_coords_survive_round_trip() {
    // 50.8 mm = 50800000 nm; check round-trip via i64 wire format.
    let mut sheet = empty_sheet();
    let mut sym = sample_symbol();
    sym.position = SchPoint {
        x: 50.800001,
        y: 25.400002,
    };
    sheet.symbols.push(sym);
    let s = SnxSchematic::new(sheet).write_string().unwrap();
    let back = SnxSchematic::parse(&s).unwrap();
    // expect rounding to nearest nanometre
    assert!((back.sheet.symbols[0].position.x - 50.800001).abs() <= 1e-6);
}

#[test]
fn extras_preserve_symbol_fields() {
    let mut sheet = empty_sheet();
    let mut sym = sample_symbol();
    sym.fields
        .insert("MPN".to_string(), "LM2596S-5.0".to_string());
    sym.fields.insert("Tolerance".to_string(), "1%".to_string());
    sym.dnp = true;
    sym.locked = true;
    sheet.symbols.push(sym);

    let s = SnxSchematic::new(sheet).write_string().unwrap();
    let back = SnxSchematic::parse(&s).unwrap();
    let recovered = &back.sheet.symbols[0];
    assert!(recovered.locked);
    // MPN flows through TSV column.
    assert_eq!(recovered.fields.get("MPN").unwrap(), "LM2596S-5.0");
    // Tolerance survived through extras.
    assert_eq!(recovered.fields.get("Tolerance").unwrap(), "1%");
    assert!(recovered.dnp);
}

#[test]
fn schematic_extras_are_byte_identical_after_parse() {
    let mut sheet = empty_sheet();
    for (key, value) in [
        ("title", "Deterministic extras"),
        ("company", "Alp Lab"),
        ("date", "2026-07-18"),
        ("revision", "A"),
    ] {
        sheet.title_block.insert(key.into(), value.into());
    }

    for id in ["zeta", "alpha", "middle"] {
        sheet.lib_symbols.insert(
            id.into(),
            LibSymbol {
                id: id.into(),
                reference: "U".into(),
                value: id.into(),
                footprint: String::new(),
                datasheet: String::new(),
                description: format!("{id} library symbol"),
                keywords: String::new(),
                fp_filters: String::new(),
                in_bom: true,
                on_board: true,
                in_pos_files: true,
                duplicate_pin_numbers_are_jumpers: false,
                graphics: Vec::new(),
                pins: Vec::new(),
                show_pin_numbers: true,
                show_pin_names: true,
                pin_name_offset: 0.0,
            },
        );
    }

    let mut symbol = sample_symbol();
    for (key, value) in [
        ("Tolerance", "1%"),
        ("MPN", "LM2596S-5.0"),
        ("Manufacturer", "Example Semi"),
        ("Rating", "5V"),
    ] {
        symbol.fields.insert(key.into(), value.into());
    }
    for (pin, uuid) in [
        ("VIN", "0192a8c0-0001-7000-8000-000000000010"),
        ("GND", "0192a8c0-0001-7000-8000-000000000011"),
        ("VOUT", "0192a8c0-0001-7000-8000-000000000012"),
    ] {
        symbol
            .pin_uuids
            .insert(pin.into(), Uuid::parse_str(uuid).unwrap());
    }

    let symbol_extras = SymbolExtras::from_symbol(&symbol);
    assert_btree_map(&symbol_extras.fields);
    assert_btree_map(&symbol_extras.pin_uuids);
    sheet.symbols.push(symbol);

    let sheet_extras = SheetExtras::from_sheet(&sheet);
    assert_btree_map(&sheet_extras.title_block);
    assert_btree_map(&sheet_extras.lib_symbols);

    let first = SnxSchematic::new(sheet).write_string().unwrap();
    let parsed = SnxSchematic::parse(&first).unwrap();
    let second = parsed.write_string().unwrap();

    assert_eq!(first, second);
    assert_eq!(parsed.sheet.title_block["revision"], "A");
    assert_eq!(parsed.sheet.lib_symbols["alpha"].value, "alpha");
    assert_eq!(
        parsed.sheet.symbols[0].fields["Manufacturer"],
        "Example Semi"
    );
    assert_eq!(
        parsed.sheet.symbols[0].pin_uuids["VOUT"],
        Uuid::parse_str("0192a8c0-0001-7000-8000-000000000012").unwrap()
    );
}

/// A symbol with no footprint, datasheet, fields or flags (#789).
fn bare_symbol() -> Symbol {
    Symbol {
        footprint: String::new(),
        ..sample_symbol()
    }
}

/// A bare symbol has all-default extras, so the writer omits its table.
/// Reloading must still give unit 1 and the BOM / board flags, and the
/// next save must be byte-identical (#789).
#[test]
fn snxsch_bare_symbol_round_trips_byte_identically() {
    let mut sheet = empty_sheet();
    sheet.symbols.push(bare_symbol());

    let first = SnxSchematic::new(sheet).write_string().expect("serialise");
    assert!(
        !first.contains("[extras"),
        "a bare symbol must not emit an extras table"
    );

    let back = SnxSchematic::parse(&first).expect("parse");
    let s = &back.sheet.symbols[0];
    assert_eq!(s.unit, 1);
    assert!(s.in_bom, "bare symbol must stay in the BOM");
    assert!(s.on_board, "bare symbol must stay on the board");

    let second = back.write_string().expect("re-serialise");
    assert_eq!(first, second, "save -> load -> save must be byte-identical");
}

/// A file saved by the buggy writer has no extras table for a bare
/// symbol; it must load with the serde defaults, not the derived zeroes.
#[test]
fn snxsch_without_symbol_extras_loads_with_serde_defaults() {
    let mut sheet = empty_sheet();
    sheet.symbols.push(sample_symbol());
    let full = SnxSchematic::new(sheet).write_string().expect("serialise");

    // Drop the whole extras tail, leaving what the old writer emitted
    // for a symbol with no footprint.
    let legacy = full
        .split(
            "
[extras",
        )
        .next()
        .expect("split always yields the head")
        .to_string();
    assert!(!legacy.contains("[extras"));

    let back = SnxSchematic::parse(&legacy).expect("legacy file loads");
    let s = &back.sheet.symbols[0];
    assert_eq!(s.unit, 1);
    assert!(s.in_bom);
    assert!(s.on_board);
}

/// `Default` must agree with serde's empty-table fill for every extras
/// struct whose writer omits default tables, so omitted == default.
#[test]
fn extras_default_matches_an_empty_serde_table() {
    let sym: SymbolExtras = toml::from_str("").expect("empty symbol table");
    assert!(sym.is_default());
    assert_eq!(
        toml::to_string(&sym).unwrap(),
        toml::to_string(&SymbolExtras::default()).unwrap()
    );
    let junction: JunctionExtras = toml::from_str("").expect("empty junction table");
    assert!(junction.is_default() && JunctionExtras::default().is_default());
    let fp: FootprintExtras = toml::from_str("").expect("empty footprint table");
    assert!(fp.is_default() && FootprintExtras::default().is_default());
    let pad: PadExtras = toml::from_str("").expect("empty pad table");
    assert!(pad.is_default() && PadExtras::default().is_default());
}

#[test]
fn snxpcb_refuses_non_finite_with_key_path() {
    // #924 — same refusal for `.snxpcb`.
    let good = SnxPcb::new(empty_board())
        .write_string()
        .expect("serialise");
    assert!(SnxPcb::parse(&good).is_ok(), "finite file still loads");
    for bad in ["nan", "inf", "-inf"] {
        let text = format!(
            "{good}
[extras.probe]
values = [1.0, {bad}]
"
        );
        match SnxPcb::parse(&text) {
            Err(FormatError::NonFiniteNumber { path }) => {
                assert_eq!(path, "extras.probe.values[1]")
            }
            other => panic!("{bad}: expected NonFiniteNumber, got {other:?}"),
        }
    }
}
