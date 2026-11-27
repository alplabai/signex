//! #2598: a footprint sketch pour with corners other than Sharp is a
//! trigger of `snxfpt/1.5` (`FootprintFile::needs_1_5`). Its Corners and
//! Corner Size round-trip; a file whose pours are all Sharp keeps its
//! lower token, byte for byte; a build that knows only 1.4 refuses it by
//! name.

use chrono::{DateTime, Utc};
use signex_sketch::SketchData;
use signex_sketch::attr::{PadAttr, PourAttr, PourFillType, ThermalRelief};
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
use signex_types::format_version::{FormatVersion, MissingToken, NewerMinor};
use signex_types::pad_thermal::PadThermal;
use signex_types::pcb_layer::{CopperLayer, PcbLayer};
use signex_types::zone_pour_shape::PourCorners;
use uuid::Uuid;

use super::format_token::{SNXFPT_FORMAT_V1_4, SNXFPT_FORMAT_V1_5};
use super::{Footprint, FootprintFile};

/// A 5 mm square of lines, the first carrying a Sharp top-copper pour.
fn pour_sketch() -> SketchData {
    let plane = PlaneId::new();
    let mut sketch = SketchData {
        planes: vec![Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        }],
        ..SketchData::default()
    };
    let corners = [(0.0, 0.0), (5.0, 0.0), (5.0, 5.0), (0.0, 5.0)];
    let points: Vec<SketchEntityId> = corners
        .iter()
        .map(|&(x, y)| {
            let id = SketchEntityId::new();
            sketch
                .entities
                .push(Entity::new(id, plane, EntityKind::Point { x, y }));
            id
        })
        .collect();
    for i in 0..4 {
        let (start, end) = (points[i], points[(i + 1) % 4]);
        let line = Entity::new(SketchEntityId::new(), plane, EntityKind::Line { start, end });
        sketch.entities.push(line);
    }
    let first_line = sketch.entities.len() - 4;
    sketch.entities[first_line].pour = Some(PourAttr {
        layer: PcbLayer::Copper(CopperLayer::Top),
        net: Some("GND".into()),
        fill_type: PourFillType::Solid,
        thermal_relief: ThermalRelief::default(),
        clearance_expr: None,
        min_thickness_expr: None,
        priority: 0,
        corners: PourCorners::default(),
        corner_size: 0.0,
    });
    sketch
}

/// `sketch` with its pour's corners set to `corners` of `size` mm.
fn with_corners(mut sketch: SketchData, corners: PourCorners, size: f64) -> SketchData {
    for pour in sketch.entities.iter_mut().filter_map(|e| e.pour.as_mut()) {
        pour.corners = corners;
        pour.corner_size = size;
    }
    sketch
}

fn file_of(sketch: SketchData) -> FootprintFile {
    let stamp = DateTime::<Utc>::from_timestamp(1_790_000_000, 0).unwrap();
    let mut fp = Footprint::with_identity("POUR", Uuid::from_u128(17), stamp);
    fp.sketch = Some(sketch);
    FootprintFile::with_identity(fp, Uuid::from_u128(18), stamp)
}

fn text_of(sketch: SketchData) -> String {
    file_of(sketch).to_toml_string().expect("serialise")
}

fn the_pour(file: &FootprintFile) -> PourAttr {
    let sketch = file.footprints[0].sketch.as_ref().expect("sketch kept");
    sketch
        .entities
        .iter()
        .find_map(|e| e.pour.clone())
        .expect("the pour")
}

/// Decision 4 (Sharp default): a new pour has Sharp corners and size 0,
/// and neither key is written.
#[test]
fn a_new_pour_is_sharp_and_writes_no_corner_keys() {
    let sketch = pour_sketch();
    let text = text_of(sketch.clone());
    let pour = the_pour(&file_of(sketch));
    assert_eq!(pour.corners, PourCorners::Sharp);
    assert_eq!(pour.corner_size, 0.0);
    assert!(!text.contains("corners = "), "{text}");
    assert!(!text.contains("corner_size = "), "{text}");
    assert!(text.starts_with("format = \"snxfpt/1.1\"\n"), "{text}");
}

#[test]
fn rounded_and_chamfered_pours_round_trip_as_snxfpt_1_5() {
    for (corners, token) in [
        (PourCorners::Rounded, "rounded"),
        (PourCorners::Chamfer, "chamfer"),
    ] {
        let text = text_of(with_corners(pour_sketch(), corners, 0.5));
        assert!(
            text.starts_with(&format!("format = \"{SNXFPT_FORMAT_V1_5}\"\n")),
            "{text}"
        );
        assert!(text.contains(&format!("corners = \"{token}\"")), "{text}");
        assert!(text.contains("corner_size = 0.5"), "{text}");
        let back = FootprintFile::from_toml_str(&text).expect("reads back");
        let pour = the_pour(&back);
        assert_eq!((pour.corners, pour.corner_size), (corners, 0.5));
        let again = back.to_toml_string().expect("serialise");
        assert_eq!(again, text, "a re-save is byte-identical");
    }
}

/// Rounded with no size yet bakes sharp, but a 1.4 build would drop the
/// key, so the token is raised all the same.
#[test]
fn rounded_corners_without_a_size_still_write_1_5() {
    let file = file_of(with_corners(pour_sketch(), PourCorners::Rounded, 0.0));
    assert_eq!(file.format_token(), SNXFPT_FORMAT_V1_5);
}

/// A size alone, with Sharp corners, shapes nothing and raises nothing.
#[test]
fn a_sharp_pour_with_a_size_keeps_the_lower_token() {
    let file = file_of(with_corners(pour_sketch(), PourCorners::Sharp, 0.5));
    assert_eq!(file.format_token(), "snxfpt/1.1");
}

/// Setting the pour back to Sharp with size 0 writes the bytes the file
/// had before.
#[test]
fn a_pour_set_back_to_sharp_saves_as_before() {
    let sketch = pour_sketch();
    let before = text_of(sketch.clone());
    let text = text_of(with_corners(sketch, PourCorners::Chamfer, 1.0));
    let mut file = FootprintFile::from_toml_str(&text).expect("1.5 reads");
    if let Some(sketch) = file.footprints[0].sketch.as_mut() {
        *sketch = with_corners(sketch.clone(), PourCorners::Sharp, 0.0);
    }
    let saved = file.to_toml_string().expect("serialise");
    assert_eq!(saved, before, "the lower token, byte for byte");
}

/// 1.5 wins over the 1.4 triggers: a sketch pad with its own thermal
/// relief beside a rounded pour writes 1.5.
#[test]
fn pour_corners_beside_a_pad_thermal_write_1_5() {
    let mut sketch = with_corners(pour_sketch(), PourCorners::Rounded, 0.5);
    let pad_point = sketch
        .entities
        .iter_mut()
        .find(|e| matches!(e.kind, EntityKind::Point { .. }))
        .expect("a point");
    pad_point.pad = Some(PadAttr {
        number: "1".into(),
        size_x_expr: "1mm".into(),
        size_y_expr: "1mm".into(),
        ..PadAttr::default()
    });
    if let Some(pad) = pad_point.pad.as_mut() {
        pad.stack.thermal = PadThermal {
            spokes: Some(2),
            ..PadThermal::RULE
        };
    }
    assert_eq!(file_of(sketch.clone()).format_token(), SNXFPT_FORMAT_V1_5);
    let sharp = with_corners(sketch, PourCorners::Sharp, 0.0);
    assert_eq!(file_of(sharp).format_token(), SNXFPT_FORMAT_V1_4);
}

#[test]
fn a_1_4_build_refuses_pour_corners_by_name() {
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
    let file = file_of(with_corners(pour_sketch(), PourCorners::Chamfer, 0.5));
    let refused = SNXFPT_1_4_ONLY
        .read(Some(file.format_token()))
        .expect_err("a 1.4 build refuses 1.5");
    assert_eq!(refused.found(), Some(SNXFPT_FORMAT_V1_5));
    let message = refused.to_string();
    assert!(message.contains(SNXFPT_FORMAT_V1_5), "{message}");
    assert!(message.contains("newer Signex"), "{message}");
}
