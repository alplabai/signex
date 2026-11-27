//! `snxfpt/1.2` (Caner 2026-11-20, decisions 5, 9 and 10): a footprint
//! whose sketch holds reference dimensions, pattern sources or mirrors, or
//! per-instance pad overrides is written as 1.2 and round-trips them; one
//! without them stays 1.1, byte for byte; a 1.1-only build refuses 1.2 by
//! name rather than dropping the links on save.

use signex_sketch::SketchData;
use signex_sketch::array::{Array, ArrayId, ArrayKind, GridSweep, NumberingScheme};
use signex_sketch::attr::PadAttr;
use signex_sketch::constraint::{Constraint, ConstraintKind, DimTarget};
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::{ConstraintId, SketchEntityId};
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
use signex_sketch::sketch::{OverrideValue, PadAttrOverride, PatternMirror};
use signex_types::format_version::{FormatVersion, MissingToken, NewerMinor};

use super::format_token::SNXFPT_FORMAT_V1_5;
use super::*;

/// The `.snxfpt` policy of a build that knows only 1.1.
const SNXFPT_1_1_ONLY: FormatVersion = FormatVersion {
    current: "snxfpt/1.1",
    readable: &["snxfpt/1", "snxfpt/1.1"],
    newer_minor: NewerMinor::Refused,
    missing: MissingToken::Refused,
};

/// A pad Point patterned by a polar and a linear array, a mirror Line and a
/// dimension, with none of the 1.2 links yet.
struct Fixture {
    sketch: SketchData,
    polar: ArrayId,
    linear: ArrayId,
    line: SketchEntityId,
    dimension: ConstraintId,
}

fn fixture() -> Fixture {
    let plane = PlaneId::new();
    let mut sketch = SketchData {
        planes: vec![Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        }],
        ..SketchData::default()
    };
    let point = |sketch: &mut SketchData, x, y| {
        let id = SketchEntityId::new();
        sketch
            .entities
            .push(Entity::new(id, plane, EntityKind::Point { x, y }));
        id
    };
    let source = point(&mut sketch, 2.0, 0.0);
    sketch.entities.last_mut().unwrap().pad = Some(PadAttr {
        number: "1".into(),
        size_x_expr: "1mm".into(),
        size_y_expr: "1mm".into(),
        ..PadAttr::default()
    });
    let centre = point(&mut sketch, 0.0, 0.0);
    let a = point(&mut sketch, 10.0, -5.0);
    let b = point(&mut sketch, 10.0, 5.0);
    let line = SketchEntityId::new();
    sketch.entities.push(Entity::new(
        line,
        plane,
        EntityKind::Line { start: a, end: b },
    ));
    let dimension = ConstraintId::new();
    sketch.constraints.push(Constraint {
        id: dimension,
        kind: ConstraintKind::DistancePtPt {
            p1: a,
            p2: b,
            target: DimTarget::Literal(10.0),
        },
    });
    let polar = ArrayId::new();
    let linear = ArrayId::new();
    sketch.arrays.push(Array {
        id: polar,
        kind: ArrayKind::Polar {
            source,
            center: centre,
            count_expr: "4".into(),
            sweep_angle_expr: "360deg".into(),
            depopulation: None,
        },
        numbering: NumberingScheme::default(),
    });
    sketch.arrays.push(Array {
        id: linear,
        kind: ArrayKind::Linear {
            source,
            count_expr: "2".into(),
            dx_expr: "20mm".into(),
            dy_expr: "0mm".into(),
            depopulation: None,
        },
        numbering: NumberingScheme::default(),
    });
    Fixture {
        sketch,
        polar,
        linear,
        line,
        dimension,
    }
}

fn file_of(sketch: SketchData) -> FootprintFile {
    let stamp = DateTime::<Utc>::from_timestamp(1_790_000_000, 0).unwrap();
    let mut fp = Footprint::with_identity("LINKS", Uuid::from_u128(7), stamp);
    fp.sketch = Some(sketch);
    FootprintFile::with_identity(fp, Uuid::from_u128(8), stamp)
}

/// Save `sketch`, read it back, and hand back (the text, the sketch read).
fn round_trip(sketch: SketchData) -> (String, SketchData) {
    let text = file_of(sketch).to_toml_string().expect("serialise");
    let back = FootprintFile::from_toml_str(&text).expect("reads back");
    let sketch = back.footprints[0].sketch.clone().expect("sketch kept");
    (text, sketch)
}

#[test]
fn a_footprint_without_links_stays_snxfpt_1_1_byte_for_byte() {
    let file = file_of(fixture().sketch);
    assert_eq!(file.format_token(), "snxfpt/1.1");
    let text = file.to_toml_string().expect("serialise");
    assert!(text.starts_with("format = \"snxfpt/1.1\"\n"), "{text}");
    for key in [
        "driven_dimensions",
        "pattern_sources",
        "pattern_mirrors",
        "pad_overrides",
    ] {
        assert!(!text.contains(key), "{key} written: {text}");
    }
    let again = FootprintFile::from_toml_str(&text)
        .expect("reads back")
        .to_toml_string()
        .expect("serialise");
    assert_eq!(again, text, "a re-save is byte-identical");
}

#[test]
fn a_reference_dimension_round_trips_as_snxfpt_1_2() {
    let mut f = fixture();
    f.sketch.driven_dimensions.push(f.dimension);
    let (text, back) = round_trip(f.sketch.clone());
    assert!(text.starts_with("format = \"snxfpt/1.2\"\n"), "{text}");
    assert_eq!(back.driven_dimensions, vec![f.dimension]);
    assert_eq!(back, f.sketch);
}

#[test]
fn a_pattern_source_round_trips_as_snxfpt_1_2() {
    let mut f = fixture();
    f.sketch.set_pattern_source(f.linear, f.polar).unwrap();
    let (text, back) = round_trip(f.sketch.clone());
    assert!(text.starts_with("format = \"snxfpt/1.2\"\n"), "{text}");
    assert_eq!(back.pattern_source_of(f.linear), Some(f.polar));
    assert_eq!(back, f.sketch);
}

#[test]
fn a_pattern_mirror_round_trips_as_snxfpt_1_2() {
    let mut f = fixture();
    let mirror = PatternMirror {
        id: ArrayId::new(),
        pattern: f.polar,
        line: f.line,
    };
    f.sketch.add_pattern_mirror(mirror).unwrap();
    let (text, back) = round_trip(f.sketch.clone());
    assert!(text.starts_with("format = \"snxfpt/1.2\"\n"), "{text}");
    assert_eq!(back.pattern_mirrors, vec![mirror]);
    assert_eq!(back, f.sketch);
}

#[test]
fn a_pad_override_round_trips_as_snxfpt_1_2() {
    let mut f = fixture();
    let pad = PadAttrOverride {
        net: Some("GND".into()),
        locked: Some(true),
        paste_margin_expr: Some(OverrideValue::Set("-0.05mm".into())),
        rotation_expr: Some(OverrideValue::Cleared),
        ..PadAttrOverride::default()
    };
    f.sketch.set_override(f.polar, 2, 0, pad.clone());
    let (text, back) = round_trip(f.sketch.clone());
    assert!(text.starts_with("format = \"snxfpt/1.2\"\n"), "{text}");
    assert_eq!(back.override_for(f.polar, 2, 0), Some(&pad));
    assert_eq!(back, f.sketch);
}

#[test]
fn a_file_whose_last_link_goes_saves_as_1_1_again() {
    let mut f = fixture();
    f.sketch.driven_dimensions.push(f.dimension);
    let text = file_of(f.sketch).to_toml_string().expect("serialise");
    let mut file = FootprintFile::from_toml_str(&text).expect("1.2 reads");
    assert_eq!(file.format, "snxfpt/1.2", "the token it was read with");
    if let Some(sketch) = file.footprints[0].sketch.as_mut() {
        sketch.driven_dimensions.clear();
    }
    let saved = file.to_toml_string().expect("serialise");
    assert!(saved.starts_with("format = \"snxfpt/1.1\"\n"), "{saved}");
}

#[test]
fn a_1_1_only_build_refuses_a_1_2_file_by_name() {
    let mut f = fixture();
    f.sketch.set_pattern_source(f.linear, f.polar).unwrap();
    let text = file_of(f.sketch).to_toml_string().expect("serialise");
    let token = text
        .lines()
        .next()
        .and_then(|l| l.strip_prefix("format = \""))
        .and_then(|l| l.strip_suffix('"'))
        .expect("a format line");
    assert_eq!(token, "snxfpt/1.2");
    let refused = SNXFPT_1_1_ONLY
        .read(Some(token))
        .expect_err("a 1.1 build refuses 1.2");
    assert_eq!(refused.found(), Some("snxfpt/1.2"));
    let message = refused.to_string();
    assert!(message.contains("snxfpt/1.2"), "{message}");
    assert!(message.contains("newer Signex"), "{message}");
    // This build reads every token it ever wrote.
    for token in ["snxfpt/1", "snxfpt/1.1", "snxfpt/1.2", "snxfpt/1.3"] {
        assert_eq!(
            FOOTPRINT_FILE_VERSION.read(Some(token)).as_deref(),
            Ok(token)
        );
    }
}

/// A footprint with generated features, its strokes included.
fn generated_file() -> FootprintFile {
    let stamp = DateTime::<Utc>::from_timestamp(1_790_000_000, 0).unwrap();
    let mut fp = Footprint::with_identity("GENERATED", Uuid::from_u128(9), stamp);
    fp.generated = GeneratedFeatures {
        courtyard_line_width_mm: DEFAULT_COURTYARD_LINE_WIDTH_MM,
        silk_outline: true,
        silk_line_width_mm: 0.15,
        pad_clearance_mm: 0.25,
        pin_one_marker: false,
        assembly_outline: true,
        silk: vec![FpGraphic::drawn(
            FpGraphicKind::Line {
                from: [-1.0, 0.0],
                to: [1.0, 0.0],
            },
            0.15,
            false,
        )],
        assembly: Vec::new(),
    };
    FootprintFile::with_identity(fp, Uuid::from_u128(10), stamp)
}

#[test]
fn generated_features_round_trip_as_snxfpt_1_3() {
    let file = generated_file();
    let text = file.to_toml_string().expect("serialise");
    assert!(text.starts_with("format = \"snxfpt/1.3\"\n"), "{text}");
    let back = FootprintFile::from_toml_str(&text).expect("1.3 reads");
    assert_eq!(back.footprints[0].generated, file.footprints[0].generated);
    assert_eq!(back.to_toml_string().expect("serialise"), text);
}

#[test]
fn a_footprint_whose_features_go_back_to_default_saves_as_1_1() {
    let mut file = generated_file();
    file.footprints[0].generated = GeneratedFeatures::default();
    let text = file.to_toml_string().expect("serialise");
    assert!(text.starts_with("format = \"snxfpt/1.1\"\n"), "{text}");
    assert!(!text.contains("generated"), "{text}");
}

/// Mockup row 124: a courtyard line width off its default is a generated
/// feature, written as 1.3 and read back unchanged.
#[test]
fn a_courtyard_line_width_round_trips_as_snxfpt_1_3() {
    let stamp = DateTime::<Utc>::from_timestamp(1_790_000_000, 0).unwrap();
    let mut fp = Footprint::with_identity("COURTYARD", Uuid::from_u128(11), stamp);
    fp.generated.courtyard_line_width_mm = 0.1524;
    let file = FootprintFile::with_identity(fp, Uuid::from_u128(12), stamp);
    let text = file.to_toml_string().expect("serialise");
    assert!(text.starts_with("format = \"snxfpt/1.3\"\n"), "{text}");
    assert!(text.contains("courtyard_line_width_mm = 0.1524"), "{text}");
    let back = FootprintFile::from_toml_str(&text).expect("1.3 reads");
    assert_eq!(back.footprints[0].generated.courtyard_line_width_mm, 0.1524);
    assert_eq!(back.to_toml_string().expect("serialise"), text);
}

#[test]
fn a_default_courtyard_line_width_keeps_the_lower_token() {
    let stamp = DateTime::<Utc>::from_timestamp(1_790_000_000, 0).unwrap();
    let fp = Footprint::with_identity("COURTYARD", Uuid::from_u128(13), stamp);
    assert_eq!(
        fp.generated.courtyard_line_width_mm,
        DEFAULT_COURTYARD_LINE_WIDTH_MM
    );
    let file = FootprintFile::with_identity(fp, Uuid::from_u128(14), stamp);
    let text = file.to_toml_string().expect("serialise");
    assert!(text.starts_with("format = \"snxfpt/1.1\"\n"), "{text}");
    assert!(!text.contains("courtyard_line_width_mm"), "{text}");
}

#[test]
fn a_1_2_build_refuses_generated_features_by_name() {
    const SNXFPT_1_2_ONLY: FormatVersion = FormatVersion {
        current: "snxfpt/1.2",
        readable: &["snxfpt/1", "snxfpt/1.1", "snxfpt/1.2"],
        newer_minor: NewerMinor::Refused,
        missing: MissingToken::Refused,
    };
    let refused = SNXFPT_1_2_ONLY
        .read(Some(generated_file().format_token()))
        .expect_err("a 1.2 build refuses 1.3");
    assert_eq!(refused.found(), Some("snxfpt/1.3"));
}

/// The fixture with its linear array numbered in `sweep` order.
fn swept_sketch(sweep: GridSweep) -> SketchData {
    let f = fixture();
    let mut sketch = f.sketch;
    if let Some(array) = sketch.arrays.iter_mut().find(|a| a.id == f.linear) {
        array.numbering = NumberingScheme::LinearIncrement {
            start_expr: "1".into(),
            step_expr: "1".into(),
            sweep,
        };
    }
    sketch
}

#[test]
fn a_numbering_sweep_round_trips_as_snxfpt_1_3() {
    for sweep in [GridSweep::ColumnByColumn, GridSweep::DownThenUp] {
        let sketch = swept_sketch(sweep);
        let (text, back) = round_trip(sketch.clone());
        assert!(text.starts_with("format = \"snxfpt/1.3\"\n"), "{text}");
        assert_eq!(back, sketch, "the sweep survives");
    }
}

#[test]
fn a_row_by_row_sweep_stays_snxfpt_1_1() {
    let file = file_of(swept_sketch(GridSweep::RowByRow));
    assert_eq!(file.format_token(), "snxfpt/1.1");
    let text = file.to_toml_string().expect("serialise");
    assert!(!text.contains("sweep ="), "{text}");
}

#[test]
fn a_footprint_whose_sweep_goes_back_saves_as_1_1() {
    let text = file_of(swept_sketch(GridSweep::DownThenUp))
        .to_toml_string()
        .expect("serialise");
    let mut file = FootprintFile::from_toml_str(&text).expect("1.3 reads");
    if let Some(sketch) = file.footprints[0].sketch.as_mut() {
        for array in &mut sketch.arrays {
            array.numbering = NumberingScheme::default();
        }
    }
    let saved = file.to_toml_string().expect("serialise");
    assert!(saved.starts_with("format = \"snxfpt/1.1\"\n"), "{saved}");
}

#[test]
fn a_1_2_build_refuses_a_numbering_sweep_by_name() {
    const SNXFPT_1_2_ONLY: FormatVersion = FormatVersion {
        current: "snxfpt/1.2",
        readable: &["snxfpt/1", "snxfpt/1.1", "snxfpt/1.2"],
        newer_minor: NewerMinor::Refused,
        missing: MissingToken::Refused,
    };
    let file = file_of(swept_sketch(GridSweep::DownThenUp));
    let refused = SNXFPT_1_2_ONLY
        .read(Some(file.format_token()))
        .expect_err("a 1.2 build refuses 1.3");
    assert_eq!(refused.found(), Some("snxfpt/1.3"));
}

/// #2688: the fixture's sketch with instance `(1, 0)` of its linear array
/// suppressed.
fn with_linear_suppressed(f: &Fixture) -> SketchData {
    let mut sketch = f.sketch.clone();
    if let Some(array) = sketch.arrays.iter_mut().find(|a| a.id == f.linear) {
        assert!(array.kind.set_instance_suppressed(1, 0, true));
    }
    sketch
}

#[test]
fn a_linear_suppression_round_trips_as_snxfpt_1_5() {
    let sketch = with_linear_suppressed(&fixture());
    let (text, back) = round_trip(sketch.clone());
    assert!(
        text.starts_with(&format!("format = \"{SNXFPT_FORMAT_V1_5}\"\n")),
        "{text}"
    );
    assert_eq!(back, sketch, "the suppression list survives");
    let again = FootprintFile::from_toml_str(&text)
        .expect("reads back")
        .to_toml_string()
        .expect("serialise");
    assert_eq!(again, text, "a re-save is byte-identical");
}

/// The 1.5 token wins over the content of every lower one.
#[test]
fn a_linear_suppression_with_a_numbering_sweep_writes_1_5() {
    let mut sketch = with_linear_suppressed(&fixture());
    for array in &mut sketch.arrays {
        array.numbering = NumberingScheme::LinearIncrement {
            start_expr: "1".into(),
            step_expr: "1".into(),
            sweep: GridSweep::ColumnByColumn,
        };
    }
    assert_eq!(file_of(sketch).format_token(), SNXFPT_FORMAT_V1_5);
}

/// Restoring the last suppressed instance writes the bytes the file had
/// before the suppression.
#[test]
fn a_footprint_whose_last_linear_suppression_goes_saves_as_before() {
    let f = fixture();
    let before = file_of(f.sketch.clone())
        .to_toml_string()
        .expect("serialise");
    let text = file_of(with_linear_suppressed(&f))
        .to_toml_string()
        .expect("serialise");
    let mut file = FootprintFile::from_toml_str(&text).expect("1.5 reads");
    assert_eq!(
        file.format, SNXFPT_FORMAT_V1_5,
        "the token it was read with"
    );
    if let Some(sketch) = file.footprints[0].sketch.as_mut() {
        for array in &mut sketch.arrays {
            array.kind.set_instance_suppressed(1, 0, false);
        }
    }
    let saved = file.to_toml_string().expect("serialise");
    assert_eq!(saved, before, "the lower token, byte for byte");
}

#[test]
fn a_1_4_build_refuses_a_linear_suppression_by_name() {
    const SNXFPT_1_4_ONLY: FormatVersion = FormatVersion {
        current: "snxfpt/1.4",
        readable: &[
            "snxfpt/1",
            "snxfpt/1.1",
            "snxfpt/1.2",
            "snxfpt/1.3",
            "snxfpt/1.4",
        ],
        newer_minor: NewerMinor::Refused,
        missing: MissingToken::Refused,
    };
    let file = file_of(with_linear_suppressed(&fixture()));
    let refused = SNXFPT_1_4_ONLY
        .read(Some(file.format_token()))
        .expect_err("a 1.4 build refuses 1.5");
    assert_eq!(refused.found(), Some(SNXFPT_FORMAT_V1_5));
    let message = refused.to_string();
    assert!(message.contains(SNXFPT_FORMAT_V1_5), "{message}");
    assert!(message.contains("newer Signex"), "{message}");
    assert_eq!(
        FOOTPRINT_FILE_VERSION
            .read(Some(SNXFPT_FORMAT_V1_5))
            .as_deref(),
        Ok(SNXFPT_FORMAT_V1_5),
        "this build reads it"
    );
}
