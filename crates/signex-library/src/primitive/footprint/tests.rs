//! Unit tests for the footprint primitive + pad TSV codec.

use super::serde_tsv::*;
use super::*;

fn fixture_pad(num: &str) -> Pad {
    Pad {
        number: num.into(),
        kind: PadKind::Smd,
        shape: PadShape::Rect,
        size: [1.025, 1.4],
        position: [0.0, 0.0],
        rotation: 0.0,
        layers: vec![LayerId::new("F.Cu"), LayerId::new("F.Mask")],
        drill: None,
        solder_mask_margin: None,
        paste_margin: None,
        ..Pad::default()
    }
}

#[test]
fn footprint_json_roundtrip_with_body3d() {
    let fp = Footprint {
        uuid: Uuid::now_v7(),
        name: "SOIC-8".into(),
        anchor: [0.0, 0.0],
        pads: vec![fixture_pad("1"), fixture_pad("2")],
        courtyard: Polygon::new(vec![[-2.5, -2.5], [2.5, -2.5], [2.5, 2.5], [-2.5, 2.5]]),
        silk_f: vec![FpGraphic {
            kind: FpGraphicKind::Line {
                from: [-1.0, 0.0],
                to: [1.0, 0.0],
            },
            stroke_width: 0.12,
            filled: false,
        }],
        silk_b: Vec::new(),
        fab_f: Vec::new(),
        fab_b: Vec::new(),
        body_3d: Body3D {
            shape: BodyShape::Extrude,
            height_mm: 1.6,
            offset_z_mm: 0.1,
            top_color: [0.10, 0.10, 0.10, 1.0],
            side_color: [0.20, 0.20, 0.20, 1.0],
            outline: None,
        },
        step_attachment: Some(StepAttachment {
            content_hash: "abcdef0123456789".into(),
            filename: "SOIC-8.step".into(),
            offset_xyz: [0.0, 0.0, 0.5],
            rotation_xyz: [0.0, 0.0, 90.0],
        }),
        pcb_params: ParamMap::new(),
        version: "0.0.1".into(),
        released: false,
        created: Utc::now(),
        updated: Utc::now(),
        schema_version: 2,
        sketch: None,
        load_repairs: Vec::new(),
        pours: Vec::new(),
        keepouts: Vec::new(),
        cutouts: Vec::new(),
        v_scores: Vec::new(),
        mask_openings: Vec::new(),
        mask_excludes: Vec::new(),
        paste_apertures: Vec::new(),
        description: String::new(),
        default_designator: String::new(),
        component_type: ComponentType::Standard,
        height_mm: None,
    };
    let json = serde_json::to_string(&fp).unwrap();
    let back: Footprint = serde_json::from_str(&json).unwrap();
    assert_eq!(fp, back);
}

#[test]
fn body3d_default_is_grey_extrude_at_zero_offset() {
    let b = Body3D::default();
    assert_eq!(b.shape, BodyShape::Extrude);
    assert_eq!(b.offset_z_mm, 0.0);
    assert!(b.outline.is_none());
    // Round-trip must succeed even without explicit fields.
    let json = serde_json::to_string(&b).unwrap();
    let back: Body3D = serde_json::from_str(&json).unwrap();
    assert_eq!(b, back);
}

#[test]
fn pad_kind_round_trip_all_variants() {
    for k in [
        PadKind::Smd,
        PadKind::Tht,
        PadKind::NptHole,
        PadKind::ConnectorPad,
    ] {
        let json = serde_json::to_string(&k).unwrap();
        let back: PadKind = serde_json::from_str(&json).unwrap();
        assert_eq!(k, back);
    }
}

#[test]
fn pad_shape_round_trip_each_variant() {
    let cases = [
        PadShape::Round,
        PadShape::Rect,
        PadShape::RoundRect { radius_ratio: 0.25 },
        PadShape::Oval,
        PadShape::Custom(Polygon::new(vec![[0.0, 0.0], [1.0, 0.0], [0.5, 1.0]])),
    ];
    for s in cases {
        let json = serde_json::to_string(&s).unwrap();
        let back: PadShape = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }
}

#[test]
fn body_shape_round_trip_all_variants() {
    for s in BodyShape::ALL {
        let json = serde_json::to_string(s).unwrap();
        let back: BodyShape = serde_json::from_str(&json).unwrap();
        assert_eq!(*s, back);
    }
}

#[test]
fn empty_footprint_has_no_pads() {
    let fp = Footprint::empty("test");
    assert_eq!(fp.name, "test");
    assert!(fp.pads.is_empty());
    assert_eq!(fp.body_3d, Body3D::default());
}

#[test]
fn step_attachment_round_trip() {
    let s = StepAttachment {
        content_hash: "0123456789abcdef".into(),
        filename: "Test.step".into(),
        offset_xyz: [1.0, 2.0, 3.0],
        rotation_xyz: [10.0, 20.0, 30.0],
    };
    let json = serde_json::to_string(&s).unwrap();
    let back: StepAttachment = serde_json::from_str(&json).unwrap();
    assert_eq!(s, back);
}

// ---- v0.18.2 — FootprintFile TOML envelope round-trip + JSON ----

#[test]
fn footprint_file_toml_round_trip_empty() {
    let fp = Footprint::empty("SOIC-8");
    let original = FootprintFile::from_footprint(fp.clone());
    let toml_text = original.to_toml_string().expect("serialise");
    let back = FootprintFile::from_toml_str(&toml_text).expect("parse");
    assert_eq!(back.footprints.len(), 1);
    assert_eq!(back.footprints[0].name, "SOIC-8");
    assert_eq!(back.format, "snxfpt/1");
    assert_eq!(back.file_uuid, original.file_uuid);
}

#[test]
fn footprint_file_toml_round_trip_with_pads() {
    let mut fp = Footprint::empty("R0805");
    fp.pads.push(fixture_pad("1"));
    fp.pads.push(fixture_pad("2"));
    let original = FootprintFile::from_footprint(fp);
    let toml_text = original.to_toml_string().expect("serialise");
    let back = FootprintFile::from_toml_str(&toml_text).expect("parse");
    assert_eq!(back.footprints[0].pads.len(), 2);
    assert_eq!(back.footprints[0].pads[0].number, "1");
    assert_eq!(back.footprints[0].pads[1].number, "2");
}

#[test]
fn footprint_file_toml_round_trip_multi() {
    let mut file = FootprintFile::from_footprint(Footprint::empty("SOIC-8"));
    file.footprints.push(Footprint::empty("QFN-16"));
    file.footprints.push(Footprint::empty("R0805"));
    file.display_name = "Reference parts".into();
    let toml_text = file.to_toml_string().expect("serialise");
    let back = FootprintFile::from_toml_str(&toml_text).expect("parse");
    assert_eq!(back.footprints.len(), 3);
    let names: Vec<&str> = back.footprints.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, vec!["SOIC-8", "QFN-16", "R0805"]);
}

#[test]
fn footprint_file_from_bytes_decodes_toml_envelope() {
    let mut file = FootprintFile::from_footprint(Footprint::empty("TOML-Test"));
    file.footprints.push(Footprint::empty("Second"));
    let toml_bytes = file.to_toml_string().unwrap().into_bytes();
    let back = FootprintFile::from_bytes(&toml_bytes).expect("parse");
    assert_eq!(back.footprints.len(), 2);
    assert_eq!(back.footprints[0].name, "TOML-Test");
}

#[test]
fn footprint_file_from_bytes_rejects_empty_payload() {
    match FootprintFile::from_bytes(b"   \n  \t\n") {
        Err(FootprintFileError::Empty) => {}
        other => panic!("expected Empty, got {other:?}"),
    }
}

// ---- v0.18.4 — pad TSV codec ------------------------------------

#[test]
fn pad_kind_token_round_trip_all_variants() {
    for k in [
        PadKind::Smd,
        PadKind::Tht,
        PadKind::NptHole,
        PadKind::ConnectorPad,
        PadKind::Castellated,
        PadKind::Fiducial,
    ] {
        let token = pad_kind_token(k);
        let back = pad_kind_from_token(token).unwrap();
        assert_eq!(k, back);
    }
}

#[test]
fn pad_shape_token_round_trip_each_variant() {
    let cases = [
        PadShape::Round,
        PadShape::Rect,
        PadShape::Oval,
        PadShape::RoundRect { radius_ratio: 0.25 },
        PadShape::Chamfered {
            chamfer_ratio: 0.4,
            corners: ChamferedCorners {
                top_left: true,
                top_right: false,
                bottom_left: true,
                bottom_right: false,
            },
        },
        PadShape::Custom(Polygon::new(vec![[0.0, 0.0], [1.5, 0.0], [0.75, 1.0]])),
        PadShape::Custom(Polygon::new(Vec::new())),
    ];
    for s in cases {
        let token = pad_shape_to_token(&s, "t").unwrap();
        let back = pad_shape_from_token(&token).unwrap();
        assert_eq!(s, back, "round-trip failed via token {token:?}");
    }
}

#[test]
fn pads_to_tsv_empty_emits_header_only() {
    let tsv = pads_to_tsv(&[]).expect("serialise");
    assert_eq!(tsv, format!("{}\n", PAD_TSV_COLUMNS.join("\t")));
}

#[test]
fn pads_to_tsv_rejects_tab_in_cell() {
    let mut pad = fixture_pad("1");
    pad.number = "1\t2".into();
    match pads_to_tsv(std::slice::from_ref(&pad)) {
        Err(FootprintFileError::InvalidTsvCell { column, .. }) => {
            assert_eq!(column, "number");
        }
        other => panic!("expected InvalidTsvCell, got {other:?}"),
    }
}

#[test]
fn pads_from_tsv_rejects_schema_mismatch() {
    let bad = "foo\tbar\n1\t2\n";
    match pads_from_tsv(bad) {
        Err(FootprintFileError::PadsTsvSchemaMismatch { got }) => {
            assert_eq!(got, vec!["foo", "bar"]);
        }
        other => panic!("expected PadsTsvSchemaMismatch, got {other:?}"),
    }
}

#[test]
fn pads_from_tsv_rejects_drill_slot_without_diameter() {
    // 13 cells: drill_diameter (col 9) empty, drill_slot_length
    // (col 10) non-empty → invariant violation.
    let header = PAD_TSV_COLUMNS.join("\t");
    let row = "1\tSmd\trect\t1.5\t1.5\t0\t0\t0\tF.Cu\t\t2.0\t\t";
    let body = format!("{header}\n{row}\n");
    match pads_from_tsv(&body) {
        Err(FootprintFileError::InvalidNumericCell { column, .. }) => {
            assert_eq!(column, "drill_slot_length");
        }
        other => panic!("expected InvalidNumericCell, got {other:?}"),
    }
}

/// All-fields round-trip — every Pad field gets a non-default
/// value (chamfered shape, non-trivial drill, multiple layers,
/// solder/paste margins) so the TSV cell encoders / decoders are
/// exercised end-to-end.
#[test]
fn footprint_file_round_trip_with_full_pad_payload() {
    let pad = Pad {
        number: "EP".into(),
        kind: PadKind::Tht,
        shape: PadShape::Chamfered {
            chamfer_ratio: 0.3,
            corners: ChamferedCorners {
                top_left: false,
                top_right: true,
                bottom_left: true,
                bottom_right: false,
            },
        },
        size: [2.5, 1.6],
        position: [-0.75, 1.25],
        rotation: 45.0,
        layers: vec![
            LayerId::new("F.Cu"),
            LayerId::new("F.Mask"),
            LayerId::new("F.Paste"),
        ],
        drill: Some(Drill {
            diameter: 0.8,
            slot_length: Some(2.4),
        }),
        solder_mask_margin: Some(0.05),
        paste_margin: Some(-0.025),
        ..Pad::default()
    };
    let mut fp = Footprint::empty("CUSTOM");
    fp.pads = vec![pad.clone()];
    let file = FootprintFile::from_footprint(fp);
    let toml_text = file.to_toml_string().expect("serialise");
    let back = FootprintFile::from_toml_str(&toml_text).expect("parse");
    assert_eq!(back.footprints[0].pads.len(), 1);
    assert_eq!(back.footprints[0].pads[0], pad);
}

#[test]
fn footprint_file_round_trip_with_custom_polygon_pad() {
    let pad = Pad {
        number: "1".into(),
        kind: PadKind::Smd,
        shape: PadShape::Custom(Polygon::new(vec![
            [0.0, 0.0],
            [1.0, 0.0],
            [1.5, 0.5],
            [1.0, 1.0],
            [0.0, 1.0],
        ])),
        size: [1.5, 1.0],
        position: [0.0, 0.0],
        rotation: 0.0,
        layers: vec![LayerId::new("F.Cu")],
        drill: None,
        solder_mask_margin: None,
        paste_margin: None,
        ..Pad::default()
    };
    let mut fp = Footprint::empty("CUSTOM");
    fp.pads = vec![pad.clone()];
    let file = FootprintFile::from_footprint(fp);
    let toml_text = file.to_toml_string().expect("serialise");
    let back = FootprintFile::from_toml_str(&toml_text).expect("parse");
    assert_eq!(back.footprints[0].pads[0], pad);
}

#[test]
fn footprint_file_to_toml_emits_pads_as_literal_multiline() {
    let mut fp = Footprint::empty("Demo");
    fp.pads.push(fixture_pad("1"));
    let toml_text = FootprintFile::from_footprint(fp).to_toml_string().unwrap();
    assert!(
        toml_text.contains("pads_tsv = '''"),
        "expected literal multi-line opener; got:\n{toml_text}"
    );
    assert!(
        !toml_text.contains(PADS_TSV_PLACEHOLDER_PREFIX),
        "placeholder should be fully replaced; got:\n{toml_text}"
    );
}

#[test]
fn footprint_file_unsupported_format_token_is_rejected() {
    // Any token other than "snxfpt/1" must surface
    // FootprintFileError::UnsupportedFormat.
    let bad = r#"
format = "snxfpt/99"
file_uuid = "00000000-0000-0000-0000-000000000000"
display_name = ""
created = "2026-05-04T00:00:00Z"
updated = "2026-05-04T00:00:00Z"
footprints = []
"#;
    match FootprintFile::from_toml_str(bad) {
        Err(FootprintFileError::UnsupportedFormat { got }) => {
            assert_eq!(got, "snxfpt/99");
        }
        other => panic!("expected UnsupportedFormat, got {other:?}"),
    }
}

#[test]
fn footprint_file_get_by_uuid() {
    let a = Footprint::empty("A");
    let b = Footprint::empty("B");
    let a_uuid = a.uuid;
    let b_uuid = b.uuid;
    let mut file = FootprintFile::from_footprint(a);
    file.footprints.push(b);
    assert_eq!(
        file.get_footprint(a_uuid).map(|f| f.name.as_str()),
        Some("A")
    );
    assert_eq!(
        file.get_footprint(b_uuid).map(|f| f.name.as_str()),
        Some("B")
    );
    assert!(file.get_footprint(Uuid::now_v7()).is_none());
}

/// #924 — a `nan`/`inf` in a `.snxfpt` is refused at load, naming the key.
#[test]
fn from_toml_str_refuses_non_finite_with_key_path() {
    let good = FootprintFile::from_footprint(Footprint::empty("F"))
        .to_toml_string()
        .expect("serialise");
    assert!(FootprintFile::from_toml_str(&good).is_ok());
    for bad in ["nan", "inf"] {
        let text = format!("{good}\n[probe]\nx = {bad}\n");
        match FootprintFile::from_toml_str(&text) {
            Err(FootprintFileError::NonFiniteNumber { path }) => assert_eq!(path, "probe.x"),
            other => panic!("{bad}: expected NonFiniteNumber, got {other:?}"),
        }
    }
}

/// #924 — `nan` in a real typed field (the footprint anchor) is refused.
#[test]
fn from_toml_str_refuses_non_finite_typed_field() {
    let good = FootprintFile::from_footprint(Footprint::empty("F"))
        .to_toml_string()
        .expect("serialise");
    let bad = good.replacen(
        "anchor = [
    0.0,",
        "anchor = [
    nan,",
        1,
    );
    assert_ne!(bad, good, "fixture must contain the anchor line");
    match FootprintFile::from_toml_str(&bad) {
        Err(FootprintFileError::NonFiniteNumber { path }) => {
            assert!(path.ends_with("anchor[0]"), "{path}")
        }
        other => panic!("expected NonFiniteNumber, got {other:?}"),
    }
}

const NON_FINITE: [f64; 3] = [f64::NAN, f64::INFINITY, f64::NEG_INFINITY];

/// A column name and the edit that puts a number in that column.
type PadSetter = (&'static str, fn(&mut Pad, f64));

fn assert_non_finite_names(result: Result<String, FootprintFileError>, needles: &[&str]) {
    match result {
        Err(FootprintFileError::NonFiniteField { field }) => {
            for needle in needles {
                assert!(field.contains(needle), "{field:?} must name {needle:?}");
            }
        }
        other => panic!("expected NonFiniteField, got {other:?}"),
    }
}

/// #828 -- a non-finite pad cell is a save error naming pad and column.
/// Before the guard a release build wrote an empty cell (the value reset
/// silently on reopen) and a debug build panicked in `debug_assert!`.
#[test]
fn pads_to_tsv_refuses_a_non_finite_cell() {
    let setters: [PadSetter; 8] = [
        ("size", |p, v| p.size[0] = v),
        ("pos", |p, v| p.position[1] = v),
        ("rotation", |p, v| p.rotation = v),
        ("drill_diameter", |p, v| {
            p.drill = Some(Drill {
                diameter: v,
                slot_length: None,
            })
        }),
        ("drill_slot_length", |p, v| {
            p.drill = Some(Drill {
                diameter: 0.8,
                slot_length: Some(v),
            })
        }),
        ("solder_mask_margin", |p, v| p.solder_mask_margin = Some(v)),
        ("paste_margin", |p, v| p.paste_margin = Some(v)),
        ("shape", |p, v| {
            p.shape = PadShape::RoundRect { radius_ratio: v }
        }),
    ];
    for (column, set) in setters {
        for bad in NON_FINITE {
            let mut pad = fixture_pad("A1");
            set(&mut pad, bad);
            assert_non_finite_names(pads_to_tsv(&[pad]), &[column, "\"A1\""]);
        }
    }
}

#[test]
fn pads_to_tsv_refuses_a_non_finite_custom_shape_point() {
    for bad in NON_FINITE {
        let mut pad = fixture_pad("C");
        pad.shape = PadShape::Custom(Polygon::new(vec![[0.0, 0.0], [bad, 1.0], [1.0, 1.0]]));
        assert_non_finite_names(pads_to_tsv(&[pad]), &["shape", "\"C\""]);
    }
}

/// #828 -- the TOML-carried numbers (graphics, courtyard, ...) are
/// guarded too, and the error names the key path.
#[test]
fn to_toml_string_refuses_a_non_finite_graphic() {
    for bad in NON_FINITE {
        let mut fp = Footprint::empty("G");
        fp.silk_f.push(FpGraphic {
            kind: FpGraphicKind::Line {
                from: [0.0, 0.0],
                to: [1.0, 0.0],
            },
            stroke_width: bad,
            filled: false,
        });
        assert_non_finite_names(
            FootprintFile::from_footprint(fp).to_toml_string(),
            &["footprints[0].silk_f[0].stroke_width"],
        );
    }
}

/// #828 -- the pad-shape reader refuses what the writer refuses: a
/// `nan` / `inf` ratio or custom point is `InvalidPadShape`, not geometry.
#[test]
fn pad_shape_from_token_refuses_a_non_finite_number() {
    for token in [
        "round_rect:inf",
        "round_rect:-inf",
        "round_rect:nan",
        "chamfered:nan:0000",
        "chamfered:inf:1111",
        "custom:inf,0|1,1",
        "custom:0,0|1,nan|1,1",
        "custom:0,0|1,-inf",
    ] {
        assert!(
            matches!(
                pad_shape_from_token(token),
                Err(FootprintFileError::InvalidPadShape(ref t)) if t == token
            ),
            "{token:?} must be refused as InvalidPadShape"
        );
    }
}

#[test]
fn a_finite_footprint_still_saves() {
    let mut fp = Footprint::empty("OK");
    fp.pads.push(fixture_pad("1"));
    assert!(FootprintFile::from_footprint(fp).to_toml_string().is_ok());
}

/// #960 — a pad with EVERY field at a non-default value. Written as an
/// exhaustive literal (no `..Pad::default()`), so adding a `Pad` field
/// is a compile error here. Give the new field a non-default value
/// here AND add it to `EXTENDED_COLUMNS` in `pad_columns.rs`; the
/// round-trip assertion fails until both are done.
fn every_field_pad() -> Pad {
    use signex_sketch::attr::{ElectricalType, PadFeature, TestpointFlags};
    Pad {
        number: "7".into(),
        kind: PadKind::Tht,
        shape: PadShape::RoundRect { radius_ratio: 0.25 },
        size: [1.7, 2.1],
        position: [3.25, -4.5],
        rotation: 30.0,
        layers: vec![LayerId::new("F.Cu"), LayerId::new("B.Cu")],
        drill: Some(Drill {
            diameter: 0.9,
            slot_length: Some(1.8),
        }),
        solder_mask_margin: Some(0.07),
        paste_margin: Some(-0.03),
        template: "PTH-ROUND".into(),
        template_library: "stock/pads".into(),
        paste_margin_top: Some(-0.04),
        paste_margin_bottom: Some(-0.05),
        paste_enabled_top: false,
        paste_enabled_bottom: false,
        mask_margin_top: Some(0.06),
        mask_margin_bottom: Some(0.08),
        mask_tented_top: true,
        mask_tented_bottom: true,
        thermal_relief: false,
        corner_radius_pct: Some(25.0),
        feature_top: PadFeature::Counterbore,
        feature_bottom: PadFeature::Countersink,
        testpoint: TestpointFlags {
            top_assembly: true,
            top_fab: false,
            bottom_assembly: true,
            bottom_fab: true,
        },
        electrical_type: ElectricalType::Terminator,
        net: "GND".into(),
        locked: true,
        hole_tolerance_plus_mm: Some(0.1),
        hole_tolerance_minus_mm: Some(0.2),
        hole_rotation_deg: Some(90.0),
        copper_offset_x_mm: Some(0.15),
        copper_offset_y_mm: Some(-0.35),
    }
}

#[test]
fn every_pad_field_survives_save_and_reopen() {
    let pad = every_field_pad();
    let mut fp = Footprint::empty("ALLFIELDS");
    fp.pads = vec![pad.clone()];
    let text = FootprintFile::from_footprint(fp).to_toml_string().unwrap();
    let back = FootprintFile::from_toml_str(&text).unwrap();
    assert_eq!(back.footprints[0].pads, vec![pad]);
    // Byte-stable: a second save of the reopened file emits the same pads_tsv.
    let again = back.to_toml_string().unwrap();
    let block = |t: &str| {
        let start = t.find("pads_tsv = '''").unwrap();
        t[start..t[start + 14..].find("'''").unwrap() + start + 14].to_string()
    };
    assert_eq!(block(&text), block(&again));
}

#[test]
fn extended_columns_are_emitted_only_when_a_pad_needs_them() {
    // All-default extended fields: header is the 13 base columns, so
    // pre-#960 files and the stock library re-serialise unchanged.
    let tsv = pads_to_tsv(&[fixture_pad("1")]).unwrap();
    assert_eq!(tsv.lines().next().unwrap(), PAD_TSV_COLUMNS.join("\t"));

    // One non-default field adds exactly its own column.
    let mut pad = fixture_pad("1");
    pad.net = "VCC".into();
    let tsv = pads_to_tsv(&[pad.clone(), fixture_pad("2")]).unwrap();
    let header: Vec<&str> = tsv.lines().next().unwrap().split('\t').collect();
    assert_eq!(&header[..13], PAD_TSV_COLUMNS);
    assert_eq!(&header[13..], &["net"]);
    let back = pads_from_tsv(&tsv).unwrap();
    assert_eq!(back[0].net, "VCC");
    assert_eq!(back[1].net, "");
}

#[test]
fn legacy_file_without_extended_columns_loads_with_defaults() {
    // A pre-#960 file: 13-column header only.
    let text = format!(
        r#"format = "snxfpt/1"
file_uuid = "00000000-0000-0000-0000-000000000001"
display_name = "Legacy"
created = "2026-05-04T00:00:00Z"
updated = "2026-05-04T00:00:00Z"

[[footprints]]
uuid = "00000000-0000-0000-0000-000000000002"
name = "Legacy"
created = "2026-05-04T00:00:00Z"
updated = "2026-05-04T00:00:00Z"
pads_tsv = '''
{}
1	Smd	rect	1.5	1.5	0	0	0	F.Cu|F.Mask			0.05	-0.02
'''
"#,
        PAD_TSV_COLUMNS.join("\t")
    );
    let file = FootprintFile::from_toml_str(&text).unwrap();
    let expected = Pad {
        number: "1".into(),
        shape: PadShape::Rect,
        size: [1.5, 1.5],
        layers: vec![LayerId::new("F.Cu"), LayerId::new("F.Mask")],
        solder_mask_margin: Some(0.05),
        paste_margin: Some(-0.02),
        ..Pad::default()
    };
    assert!(
        expected.paste_enabled_top && expected.paste_enabled_bottom && expected.thermal_relief,
        "a legacy pad loads paste-enabled and thermally relieved"
    );
    assert_eq!(file.footprints[0].pads, vec![expected]);
    // And it re-saves with the same 13-column header.
    let resaved = file.to_toml_string().unwrap();
    assert!(resaved.contains(&format!("'''\n{}\n1\t", PAD_TSV_COLUMNS.join("\t"))));
}

#[test]
fn pads_from_tsv_rejects_unknown_or_duplicate_extended_column() {
    let base = PAD_TSV_COLUMNS.join("\t");
    let row = "1\tSmd\trect\t1\t1\t0\t0\t0\tF.Cu\t\t\t\t";
    for extra in ["bogus", "net\tnet"] {
        let cells = extra.split('\t').map(|_| "").collect::<Vec<_>>().join("\t");
        let body = format!("{base}\t{extra}\n{row}\t{cells}\n");
        assert!(
            matches!(
                pads_from_tsv(&body),
                Err(FootprintFileError::PadsTsvSchemaMismatch { .. })
            ),
            "{extra:?} must be rejected"
        );
    }
}

#[test]
fn pad_default_matches_serde_defaults() {
    let d = Pad::default();
    assert!(d.paste_enabled_top && d.paste_enabled_bottom && d.thermal_relief);
    assert!(!d.mask_tented_top && !d.mask_tented_bottom && !d.locked);
}

#[test]
fn default_stack_pad_emits_only_base_columns() {
    // The editor's new-pad state: paste on both sides, thermal relief,
    // nothing tented or overridden.
    let tsv = pads_to_tsv(&[fixture_pad("1"), fixture_pad("2")]).unwrap();
    assert_eq!(tsv.lines().next().unwrap(), PAD_TSV_COLUMNS.join("\t"));
}

#[test]
fn control_characters_in_free_text_cells_are_refused_at_save() {
    for (field, bad) in [
        ("net", "A\u{1}B"),
        ("net", "GND\r"),
        ("template", "x\u{7f}"),
    ] {
        let mut pad = fixture_pad("1");
        match field {
            "net" => pad.net = bad.into(),
            _ => pad.template = bad.into(),
        }
        let mut fp = Footprint::empty("CTRL");
        fp.pads = vec![pad];
        assert!(
            matches!(
                FootprintFile::from_footprint(fp).to_toml_string(),
                Err(FootprintFileError::InvalidTsvCell { .. })
            ),
            "{bad:?} must be refused"
        );
    }
}

/// A TSV body with the base columns plus `col`, one SMD row, and `cell`
/// as the extended cell.
fn tsv_with_extended(col: &str, cell: &str) -> String {
    format!(
        "{}\t{col}\n1\tSmd\trect\t1\t1\t0\t0\t0\tF.Cu\t\t\t\t\t{cell}\n",
        PAD_TSV_COLUMNS.join("\t")
    )
}

#[test]
fn bad_extended_cells_are_rejected() {
    for (col, cell) in [
        ("feature_top", "bogus"),
        ("electrical_type", "bogus"),
        ("testpoint", "101"),
        ("locked", "true"),
        ("paste_margin_top", "NaN"),
        ("paste_margin_top", "inf"),
        ("paste_margin_top", "1e999"),
    ] {
        assert!(
            pads_from_tsv(&tsv_with_extended(col, cell)).is_err(),
            "{col}={cell:?} must be rejected"
        );
    }
    assert!(pads_from_tsv(&tsv_with_extended("locked", "1")).is_ok());
}

#[test]
fn extended_row_cell_count_mismatch_is_rejected() {
    let mut body = tsv_with_extended("net", "GND");
    body.push_str("2\tSmd\trect\t1\t1\t0\t0\t0\tF.Cu\t\t\t\t\n"); // 13 cells, header has 14
    assert!(matches!(
        pads_from_tsv(&body),
        Err(FootprintFileError::PadsTsvCellCountMismatch {
            got: 13,
            expected: 14,
            ..
        })
    ));
}

#[test]
fn some_zero_and_none_stay_distinct() {
    let mut a = fixture_pad("1");
    a.paste_margin_top = Some(0.0);
    a.hole_rotation_deg = Some(0.0);
    let b = fixture_pad("2");
    let back = pads_from_tsv(&pads_to_tsv(&[a.clone(), b.clone()]).unwrap()).unwrap();
    assert_eq!(back, vec![a, b]);
}

type ExtSetter = (&'static str, fn(&mut Pad, f64));

const EXTENDED_NUMERIC: [ExtSetter; 10] = [
    ("paste_margin_top", |p, v| p.paste_margin_top = Some(v)),
    ("paste_margin_bottom", |p, v| {
        p.paste_margin_bottom = Some(v)
    }),
    ("mask_margin_top", |p, v| p.mask_margin_top = Some(v)),
    ("mask_margin_bottom", |p, v| p.mask_margin_bottom = Some(v)),
    ("corner_radius_pct", |p, v| p.corner_radius_pct = Some(v)),
    ("hole_tolerance_plus_mm", |p, v| {
        p.hole_tolerance_plus_mm = Some(v)
    }),
    ("hole_tolerance_minus_mm", |p, v| {
        p.hole_tolerance_minus_mm = Some(v)
    }),
    ("hole_rotation_deg", |p, v| p.hole_rotation_deg = Some(v)),
    ("copper_offset_x_mm", |p, v| p.copper_offset_x_mm = Some(v)),
    ("copper_offset_y_mm", |p, v| p.copper_offset_y_mm = Some(v)),
];

/// #1037 for the #960 columns: a non-finite value in an extended numeric
/// column refuses the save, naming pad and column, instead of becoming an
/// empty cell (a silent reset to `None`) or dropping the column.
#[test]
fn pads_to_tsv_refuses_a_non_finite_extended_cell() {
    for (column, set) in EXTENDED_NUMERIC {
        for bad in NON_FINITE {
            let mut pad = fixture_pad("B7");
            set(&mut pad, bad);
            assert_non_finite_names(pads_to_tsv(&[pad]), &[column, "\"B7\""]);
            // A finite first pad must not let the bad second pad slip by.
            let mut bad_pad = fixture_pad("B8");
            set(&mut bad_pad, bad);
            assert_non_finite_names(
                pads_to_tsv(&[fixture_pad("B1"), bad_pad]),
                &[column, "\"B8\""],
            );
        }
    }
}

#[test]
fn a_refused_extended_save_leaves_the_file_on_disk_untouched() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.snxfpt");
    let mut fp = Footprint::empty("Disk");
    fp.pads.push(fixture_pad("1"));
    let original = FootprintFile::from_footprint(fp.clone())
        .to_toml_string()
        .unwrap();
    std::fs::write(&path, &original).unwrap();

    fp.pads[0].paste_margin_top = Some(f64::INFINITY);
    // The save contract: serialise first, write only on success.
    let attempt = FootprintFile::from_footprint(fp).to_toml_string();
    assert!(matches!(
        attempt,
        Err(FootprintFileError::NonFiniteField { .. })
    ));
    if let Ok(text) = attempt {
        std::fs::write(&path, text).unwrap();
    }
    assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
}

#[test]
fn arc_sweep_is_ccw_with_wrap() {
    let sweep = FpGraphicKind::arc_sweep_deg;
    assert_eq!(sweep(300.0, 60.0), 120.0);
    assert_eq!(sweep(10.0, 80.0), 70.0);
    assert_eq!(sweep(350.0, 10.0), 20.0);
    // placement tool stores atan2 output in (-180, 180]
    assert_eq!(sweep(-60.0, 60.0), 120.0);
    assert_eq!(sweep(90.0, 90.0), 0.0);
    assert_eq!(sweep(0.0, 360.0), 360.0);
}

/// A sketch entity tagged with every layer marker must survive a save and
/// a reload. `CourtyardAttr` was a unit struct, which the TOML writer
/// refuses, so a footprint with a courtyard-tagged sketch could not be
/// saved at all.
#[test]
fn footprint_file_toml_round_trip_with_every_sketch_marker() {
    use signex_sketch::attr::{
        CourtyardAttr, MaskExcludeAttr, MaskOpeningAttr, PasteApertureAttr, SilkAttr,
    };
    use signex_sketch::entity::{Entity, EntityKind};
    use signex_sketch::id::SketchEntityId;
    use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
    use signex_types::layer::SignexLayer;

    let plane = PlaneId::new();
    let mut sketch = signex_sketch::SketchData {
        planes: vec![Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        }],
        ..Default::default()
    };
    let a = SketchEntityId::new();
    let b = SketchEntityId::new();
    for (id, x) in [(a, 0.0), (b, 2.0)] {
        sketch
            .entities
            .push(Entity::new(id, plane, EntityKind::Point { x, y: 0.0 }));
    }
    let mut line = Entity::new(
        SketchEntityId::new(),
        plane,
        EntityKind::Line { start: a, end: b },
    );
    line.courtyard = Some(CourtyardAttr {});
    line.silk = Some(SilkAttr {
        layer: SignexLayer::TopSilk,
    });
    line.mask_opening = Some(MaskOpeningAttr {
        layer: SignexLayer::TopSolderMask,
    });
    line.mask_exclude = Some(MaskExcludeAttr {
        layer: SignexLayer::TopSolderMask,
    });
    line.paste_aperture = Some(PasteApertureAttr {
        layer: SignexLayer::TopPaste,
    });
    sketch.entities.push(line);

    let mut fp = Footprint::empty("Courtyard-Tagged");
    fp.sketch = Some(sketch);
    let original = FootprintFile::from_footprint(fp);
    let text = original.to_toml_string().expect("a courtyard tag saves");
    let back = FootprintFile::from_toml_str(&text).expect("parse");
    assert_eq!(back.footprints[0].sketch, original.footprints[0].sketch);
    let tagged = back.footprints[0]
        .sketch
        .as_ref()
        .unwrap()
        .entities
        .last()
        .unwrap();
    assert!(tagged.courtyard.is_some());
}

#[test]
fn a_bare_toml_datetime_in_a_known_string_field_still_opens() {
    // `display_name = 2026-10-01` is a datetime to TOML but a string here (#1875).
    let original = FootprintFile::from_footprint(Footprint::empty("SOIC-8"));
    let text = original
        .to_toml_string()
        .expect("serialise")
        .replace("display_name = \"SOIC-8\"", "display_name = 2026-10-01");
    assert!(text.contains("display_name = 2026-10-01"));
    let back = FootprintFile::from_toml_str(&text).expect("opens");
    assert_eq!(back.display_name, "2026-10-01");
}
