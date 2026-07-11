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
    HAlign, Junction, Label, LabelType as LType, Point as SchPoint, SchematicSheet, Symbol, VAlign,
    Wire,
};
use uuid::Uuid;

fn empty_sheet() -> SchematicSheet {
    SchematicSheet {
        uuid: Uuid::nil(),
        version: 1,
        generator: "signex-test".into(),
        generator_version: "0.9".into(),
        paper_size: "A4".into(),
        root_sheet_page: "1".into(),
        symbols: vec![],
        wires: vec![],
        junctions: vec![],
        labels: vec![],
        child_sheets: vec![],
        no_connects: vec![],
        text_notes: vec![],
        buses: vec![],
        bus_entries: vec![],
        drawings: vec![],
        no_erc_directives: vec![],
        title_block: Default::default(),
        lib_symbols: Default::default(),
    }
}

fn empty_board() -> PcbBoard {
    PcbBoard {
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
    assert!(s.contains("format = \"snxpcb/1\""));
    let back = SnxPcb::parse(&s).expect("round-trip");
    assert_eq!(back.format, SNXPCB_FORMAT_V1);
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

fn sample_symbol() -> Symbol {
    Symbol {
        uuid: Uuid::parse_str("0192a8c0-0001-7000-8000-000000000001").unwrap(),
        lib_id: "lm2596.snxsym".to_string(),
        reference: "U1".to_string(),
        value: "LM2596".to_string(),
        footprint: "TO-263.snxfpt".to_string(),
        datasheet: String::new(),
        position: SchPoint { x: 50.8, y: 25.4 },
        rotation: 0.0,
        mirror_x: false,
        mirror_y: false,
        unit: 1,
        is_power: false,
        ref_text: None,
        val_text: None,
        fields_autoplaced: false,
        fields_user_placed: false,
        dnp: false,
        in_bom: true,
        on_board: true,
        exclude_from_sim: false,
        locked: false,
        fields: Default::default(),
        custom_properties: Vec::new(),
        pin_uuids: Default::default(),
        library_id: None,
        row_id: None,
        library_version: String::new(),
        instances: Vec::new(),
    }
}

fn sample_wire(sx: f64, sy: f64, ex: f64, ey: f64) -> Wire {
    Wire {
        uuid: Uuid::new_v4(),
        start: SchPoint { x: sx, y: sy },
        end: SchPoint { x: ex, y: ey },
        stroke_width: 0.0,
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
    });
    sheet.labels.push(Label {
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

    assert_eq!(back.sheet.labels.len(), 1);
    assert_eq!(back.sheet.labels[0].text, "VIN");
    assert_eq!(back.sheet.labels[0].label_type, LType::Net);
}

#[test]
fn snxpcb_round_trip_with_data() {
    let mut board = empty_board();

    let pad1 = Pad {
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
            uuid: Uuid::parse_str(uuid).unwrap(),
            start: PcbPoint { x: sx, y: sy },
            end: PcbPoint { x: ex, y: ey },
            width: 0.254,
            layer: "BottomCopper".into(),
            net: 1,
        });
    }

    board.vias.push(Via {
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
        priority: 0,
        fill_type: String::new(),
        thermal_relief: false,
        thermal_gap: 0.0,
        thermal_width: 0.0,
        clearance: 0.0,
        min_thickness: 0.0,
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
}

#[test]
fn tsv_writer_pads_columns_for_legibility() {
    let rows = vec![
        SchJunctionRow {
            uuid: Uuid::nil(),
            pos_x: 100,
            pos_y: 200,
            diameter: 0.5,
        },
        SchJunctionRow {
            uuid: Uuid::nil(),
            pos_x: 30000000,
            pos_y: 40000000,
            diameter: 0.5,
        },
    ];
    let body = write_tsv_block(&rows);
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
    sheet.symbols.push(sym);

    let s = SnxSchematic::new(sheet).write_string().unwrap();
    let back = SnxSchematic::parse(&s).unwrap();
    let recovered = &back.sheet.symbols[0];
    // MPN flows through TSV column.
    assert_eq!(recovered.fields.get("MPN").unwrap(), "LM2596S-5.0");
    // Tolerance survived through extras.
    assert_eq!(recovered.fields.get("Tolerance").unwrap(), "1%");
    assert!(recovered.dnp);
}
