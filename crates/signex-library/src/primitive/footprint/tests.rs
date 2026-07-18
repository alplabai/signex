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
        let token = pad_shape_to_token(&s).unwrap();
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
