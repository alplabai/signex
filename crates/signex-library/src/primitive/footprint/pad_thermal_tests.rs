//! #2593: a footprint pad's own thermal relief values ride three optional
//! pad columns and the sketch pad's stack, write `snxfpt/1.4`, survive save
//! and reopen, and a 1.3 build refuses the file by name.

use signex_sketch::SketchData;
use signex_sketch::attr::PadAttr;
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
use signex_types::format_version::{FormatVersion, MissingToken, NewerMinor};
use signex_types::pad_thermal::PadThermal;

use super::*;

fn manual() -> PadThermal {
    PadThermal {
        air_gap: Some(0.3),
        spoke_width: None,
        spokes: Some(2),
    }
}

fn file_with(thermal: PadThermal) -> FootprintFile {
    let stamp = DateTime::<Utc>::from_timestamp(1_790_000_000, 0).unwrap();
    let mut fp = Footprint::with_identity("THERMAL", Uuid::from_u128(21), stamp);
    fp.pads.push(Pad {
        number: "1".into(),
        kind: PadKind::Tht,
        size: [1.7, 1.7],
        layers: vec![LayerId("F.Cu".into()), LayerId("B.Cu".into())],
        drill: Some(Drill {
            diameter: 1.0,
            slot_length: None,
        }),
        thermal,
        ..Pad::default()
    });
    FootprintFile::with_identity(fp, Uuid::from_u128(22), stamp)
}

#[test]
fn a_pads_own_values_write_snxfpt_1_4_and_survive_save_and_reopen() {
    let file = file_with(manual());
    assert_eq!(file.format_token(), "snxfpt/1.4");
    let text = file.to_toml_string().expect("serialise");
    assert!(text.starts_with("format = \"snxfpt/1.4\"\n"), "{text}");
    assert!(text.contains("thermal_air_gap"), "{text}");
    assert!(text.contains("thermal_spokes"), "{text}");
    assert!(
        !text.contains("thermal_spoke_width"),
        "a rule column stays out"
    );
    let back = FootprintFile::from_toml_str(&text).expect("reads back");
    assert_eq!(back.footprints[0].pads[0].thermal, manual());
}

/// A pad on the rule writes no thermal column and keeps the lower token.
#[test]
fn a_pad_on_the_rule_writes_no_thermal_column() {
    let file = file_with(PadThermal::RULE);
    assert_eq!(file.format_token(), "snxfpt/1.1");
    let text = file.to_toml_string().expect("serialise");
    assert!(!text.contains("thermal_"), "{text}");
}

#[test]
fn a_1_3_build_refuses_a_pads_own_values_by_name() {
    const SNXFPT_1_3_ONLY: FormatVersion = FormatVersion {
        current: "snxfpt/1.3",
        readable: &["snxfpt/1", "snxfpt/1.1", "snxfpt/1.2", "snxfpt/1.3"],
        newer_minor: NewerMinor::Refused,
        missing: MissingToken::Refused,
    };
    let refused = SNXFPT_1_3_ONLY
        .read(Some(file_with(manual()).format_token()))
        .expect_err("a 1.3 build refuses 1.4");
    assert_eq!(refused.found(), Some("snxfpt/1.4"));
}

#[test]
fn a_value_no_pour_can_use_fails_to_save_and_to_load() {
    let bad = PadThermal {
        spokes: Some(7),
        ..PadThermal::RULE
    };
    assert!(file_with(bad).to_toml_string().is_err());
    let text = file_with(manual()).to_toml_string().expect("serialise");
    let zero_gap = text.replacen("\t0.3\t", "\t0\t", 1);
    assert_ne!(zero_gap, text, "the gap cell is in the text");
    assert!(FootprintFile::from_toml_str(&zero_gap).is_err());
}

/// A sketch pad's own values are kept in its stack and need 1.4 as well.
#[test]
fn a_sketch_pads_own_values_write_snxfpt_1_4_and_round_trip() {
    let plane = PlaneId::new();
    let mut sketch = SketchData {
        planes: vec![Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        }],
        ..SketchData::default()
    };
    let mut entity = Entity::new(
        SketchEntityId::new(),
        plane,
        EntityKind::Point { x: 0.0, y: 0.0 },
    );
    let mut attr = PadAttr {
        number: "1".into(),
        size_x_expr: "1mm".into(),
        size_y_expr: "1mm".into(),
        ..PadAttr::default()
    };
    attr.stack.thermal = manual();
    entity.pad = Some(attr);
    sketch.entities.push(entity);
    let mut file = file_with(PadThermal::RULE);
    file.footprints[0].sketch = Some(sketch);
    assert_eq!(file.format_token(), "snxfpt/1.4");
    let text = file.to_toml_string().expect("serialise");
    let back = FootprintFile::from_toml_str(&text).expect("reads back");
    let sketch = back.footprints[0].sketch.as_ref().expect("sketch kept");
    let pad = sketch.entities[0].pad.as_ref().expect("the pad");
    assert_eq!(pad.stack.thermal, manual());
}
