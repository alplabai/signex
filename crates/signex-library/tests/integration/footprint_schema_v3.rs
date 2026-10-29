//! v0.14 footprint schema additions — round-trip tests.
//!
//! v0.14 adds optional Vec fields to `Footprint` for closed-profile
//! bake targets that v0.13 only round-tripped: pours, keepouts,
//! cutouts, v_scores, mask_openings, mask_excludes, paste_apertures.
//! It also adds two `PadKind` variants (`Castellated`, `Fiducial`) and
//! a `PadShape::Chamfered` variant.
//!
//! All additions are forward + backward compatible:
//! - new fields use `#[serde(default, skip_serializing_if = "Vec::is_empty")]`
//!   so v2 footprints load with empty Vecs and serialise identically;
//! - new enum variants are gated behind `#[non_exhaustive]` so callers
//!   already match exhaustively in their own code.

use signex_library::primitive::footprint::{
    ChamferedCorners, FOOTPRINT_SCHEMA_VERSION, Footprint, FpCutout, FpKeepout, FpMaskOpening,
    FpPasteAperture, FpPour, FpVScore, KeepoutForbid, LayerId, NetRef, PadKind, PadShape, Polygon,
    PourFillType, ThermalReliefStyle,
};

#[test]
fn empty_footprint_uses_v3_schema_version() {
    let fp = Footprint::empty("test");
    assert_eq!(fp.schema_version, FOOTPRINT_SCHEMA_VERSION);
    assert_eq!(FOOTPRINT_SCHEMA_VERSION, 3);
    // All v3 fields default to empty Vecs.
    assert!(fp.pours.is_empty());
    assert!(fp.keepouts.is_empty());
    assert!(fp.cutouts.is_empty());
    assert!(fp.v_scores.is_empty());
    assert!(fp.mask_openings.is_empty());
    assert!(fp.mask_excludes.is_empty());
    assert!(fp.paste_apertures.is_empty());
}

#[test]
fn v3_pour_round_trips() {
    let mut fp = Footprint::empty("pour-test");
    fp.pours.push(FpPour {
        boundary: Polygon::new(vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]]),
        layer: LayerId::new("Top Layer"),
        net: NetRef::named("GND"),
        fill_type: PourFillType::Solid,
        thermal_relief: ThermalReliefStyle::Spoke,
        clearance: 0.2,
        min_thickness: 0.15,
        priority: 1,
    });

    let serialised = toml::to_string(&fp).expect("v3 pour must serialise");
    let back: Footprint = toml::from_str(&serialised).expect("v3 pour must round-trip");
    assert_eq!(back, fp);
    assert_eq!(back.pours.len(), 1);
    assert_eq!(back.pours[0].layer.as_str(), "Top Layer");
    assert_eq!(back.pours[0].net.0.as_deref(), Some("GND"));
}

#[test]
fn v3_keepout_round_trips() {
    let mut fp = Footprint::empty("keepout-test");
    fp.keepouts.push(FpKeepout {
        boundary: Polygon::new(vec![[0.0, 0.0], [5.0, 0.0], [5.0, 5.0], [0.0, 5.0]]),
        layer: LayerId::new("Top Layer"),
        forbids: KeepoutForbid::Vias,
    });

    let serialised = toml::to_string(&fp).unwrap();
    let back: Footprint = toml::from_str(&serialised).unwrap();
    assert_eq!(back, fp);
    assert_eq!(back.keepouts[0].forbids, KeepoutForbid::Vias);
}

#[test]
fn v3_cutout_v_score_mask_paste_round_trip() {
    let mut fp = Footprint::empty("misc-test");
    fp.cutouts.push(FpCutout {
        boundary: Polygon::new(vec![[0.0, 0.0], [3.0, 0.0], [3.0, 3.0], [0.0, 3.0]]),
        edge_radius_mm: 0.0,
        through: true,
    });
    fp.v_scores.push(FpVScore {
        line: [[0.0, 0.0], [10.0, 0.0]],
        depth: 0.5,
        side: signex_library::primitive::footprint::VScoreSide::Both,
        min_web_mm: 0.0,
    });
    fp.mask_openings.push(FpMaskOpening {
        boundary: Polygon::new(vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]),
        layer: LayerId::new("Top Solder"),
    });
    fp.mask_excludes.push(FpMaskOpening {
        boundary: Polygon::new(vec![[0.0, 0.0], [2.0, 0.0], [2.0, 2.0], [0.0, 2.0]]),
        layer: LayerId::new("Top Solder"),
    });
    fp.paste_apertures.push(FpPasteAperture {
        boundary: Polygon::new(vec![[0.0, 0.0], [0.5, 0.0], [0.5, 0.5], [0.0, 0.5]]),
        layer: LayerId::new("Top Paste"),
    });

    let serialised = toml::to_string(&fp).unwrap();
    let back: Footprint = toml::from_str(&serialised).unwrap();
    assert_eq!(back, fp);
}

#[test]
fn text_frame_round_trips_and_defaults_none() {
    use signex_library::primitive::footprint::{FpGraphic, FpGraphicKind};

    let mut fp = Footprint::empty("FrameTest");
    fp.silk_f.push(FpGraphic {
        kind: FpGraphicKind::Text {
            position: [1.0, 2.0],
            content: "R1".into(),
            size: 1.0,
            frame: Some((5.0, 2.0)),
        },
        stroke_width: 0.15,
        filled: false,
    });

    let serialised = toml::to_string(&fp).unwrap();
    let back: Footprint = toml::from_str(&serialised).unwrap();
    assert_eq!(back, fp);
    match &back.silk_f[0].kind {
        FpGraphicKind::Text { frame, .. } => assert_eq!(*frame, Some((5.0, 2.0))),
        _ => panic!("expected Text"),
    }
}

#[test]
fn text_without_frame_defaults_to_none_on_legacy_load() {
    use signex_library::primitive::footprint::FpGraphicKind;

    // Legacy TOML (pre-frame field) omits the `frame` key entirely —
    // must still deserialise via `#[serde(default)]`.
    let legacy_toml = r#"
kind = "text"
position = [0.0, 0.0]
content = "LEGACY"
size = 1.0
"#;
    let kind: FpGraphicKind =
        toml::from_str(legacy_toml).expect("legacy text without frame must load");
    match kind {
        FpGraphicKind::Text { frame, content, .. } => {
            assert_eq!(frame, None);
            assert_eq!(content, "LEGACY");
        }
        _ => panic!("expected Text"),
    }
}

#[test]
fn v3_castellated_pad_kind_round_trips() {
    let json = r#"{"number":"1","kind":"Castellated","shape":{"kind":"rect"},"size":[1.0,1.0],"position":[0.0,0.0],"rotation":0.0,"layers":["Top Layer"],"drill":{"diameter":0.5,"slot_length":null},"solder_mask_margin":null,"paste_margin":null}"#;
    let pad: signex_library::primitive::footprint::Pad = serde_json::from_str(json).unwrap();
    assert_eq!(pad.kind, PadKind::Castellated);
}

#[test]
fn v3_fiducial_pad_kind_round_trips() {
    let json = r#"{"number":"FID1","kind":"Fiducial","shape":{"kind":"round"},"size":[1.0,1.0],"position":[0.0,0.0],"rotation":0.0,"layers":["Top Layer","Top Solder"],"drill":null,"solder_mask_margin":1.0,"paste_margin":null}"#;
    let pad: signex_library::primitive::footprint::Pad = serde_json::from_str(json).unwrap();
    assert_eq!(pad.kind, PadKind::Fiducial);
}

#[test]
fn v3_chamfered_pad_shape_round_trips() {
    let json = r#"{"kind":"chamfered","chamfer_ratio":0.25,"corners":{"top_left":true,"top_right":false,"bottom_left":false,"bottom_right":true}}"#;
    let shape: PadShape = serde_json::from_str(json).unwrap();
    match shape {
        PadShape::Chamfered {
            chamfer_ratio,
            corners,
        } => {
            assert!((chamfer_ratio - 0.25).abs() < 1e-12);
            assert!(corners.top_left);
            assert!(!corners.top_right);
            assert!(!corners.bottom_left);
            assert!(corners.bottom_right);
        }
        other => panic!("expected Chamfered, got {other:?}"),
    }
}

#[test]
fn v3_chamfered_corners_all() {
    let c = ChamferedCorners::all();
    assert!(c.top_left && c.top_right && c.bottom_left && c.bottom_right);
}

#[test]
fn v3_empty_vecs_skip_serialisation() {
    // A footprint with all v3 fields empty serialises to a TOML that
    // doesn't mention any of the new field names — preserves byte-level
    // forward compat with v2 footprints.
    let fp = Footprint::empty("plain");
    let out = toml::to_string(&fp).unwrap();
    for field in &[
        "pours",
        "keepouts",
        "cutouts",
        "v_scores",
        "mask_openings",
        "mask_excludes",
        "paste_apertures",
    ] {
        assert!(
            !out.contains(field),
            "empty v3 field `{field}` must not appear in serialised output; got:\n{out}"
        );
    }
}

#[test]
fn v2_footprint_loads_without_v3_fields() {
    // A minimal v2 footprint TOML (no v3 fields, schema_version = 2)
    // must deserialise into v3 with the new fields defaulted to empty.
    let v2_toml = r#"
uuid = "0193a8c0-0010-7000-8000-000000000099"
name = "minimal-v2"
anchor = [0.0, 0.0]
pads = []
silk_f = []
silk_b = []
fab_f = []
fab_b = []
courtyard = { points = [] }
pcb_params = {}
version = "0.0.1"
released = false
created = "2026-04-15T12:00:00Z"
updated = "2026-04-15T12:00:00Z"
schema_version = 2

[body_3d]
shape = "Extrude"
height_mm = 1.0
offset_z_mm = 0.0
top_color = [0.10, 0.10, 0.10, 1.0]
side_color = [0.20, 0.20, 0.20, 1.0]
"#;
    let fp: Footprint = toml::from_str(v2_toml).expect("v2 minimal must load into v3 struct");
    assert_eq!(fp.schema_version, 2, "v2 file keeps its on-disk version");
    assert!(fp.pours.is_empty());
    assert!(fp.keepouts.is_empty());
    assert!(fp.cutouts.is_empty());
    assert!(fp.v_scores.is_empty());
    assert!(fp.mask_openings.is_empty());
    assert!(fp.mask_excludes.is_empty());
    assert!(fp.paste_apertures.is_empty());
}
