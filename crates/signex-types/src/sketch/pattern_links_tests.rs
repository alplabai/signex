//! mockup row 191 / mockup row 192, item D1 / decision 5 — pattern links, live mirrors, per-instance
//! pad overrides and reference dimensions on the sketch model: the cycle
//! guard, the delete cascade, the file round trip and the format level.

use crate::sketch::SketchData;
use crate::sketch::array::{Array, ArrayId, ArrayKind, NumberingScheme};
use crate::sketch::attr::{DrillSpec, ElectricalType, PadAttr};
use crate::sketch::constraint::{Constraint, ConstraintKind, DimTarget};
use crate::sketch::data::{
    InstanceOverride, OverrideValue, PadAttrOverride, PatternCycle, PatternMirror, PatternSource,
};
use crate::sketch::entity::{Entity, EntityKind};
use crate::sketch::id::{ConstraintId, SketchEntityId};
use crate::sketch::plane::PlaneId;

/// A sketch with a pad source Point, a polar centre, a mirror Line and a
/// dimension on the Line's ends; a polar array of the source, a linear
/// array repeating the polar one and a mirror of the linear one.
struct Fixture {
    sketch: SketchData,
    source: SketchEntityId,
    line_end: SketchEntityId,
    line: SketchEntityId,
    dimension: ConstraintId,
    polar: ArrayId,
    linear: ArrayId,
}

fn fixture() -> Fixture {
    let plane = PlaneId::new();
    let mut sketch = SketchData::default();
    let point = |sketch: &mut SketchData, x, y| {
        let id = SketchEntityId::new();
        sketch
            .entities
            .push(Entity::new(id, plane, EntityKind::Point { x, y }));
        id
    };
    let source = point(&mut sketch, 1.0, 0.0);
    let centre = point(&mut sketch, 0.0, 0.0);
    let line_start = point(&mut sketch, 10.0, -5.0);
    let line_end = point(&mut sketch, 10.0, 5.0);
    let line = SketchEntityId::new();
    sketch.entities.push(Entity::new(
        line,
        plane,
        EntityKind::Line {
            start: line_start,
            end: line_end,
        },
    ));
    let dimension = ConstraintId::new();
    sketch.constraints.push(Constraint {
        id: dimension,
        kind: ConstraintKind::DistancePtPt {
            p1: line_start,
            p2: line_end,
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
    // The linear array's own source starts at the centre; linking it to the
    // polar array points it at the innermost source.
    sketch.arrays.push(Array {
        id: linear,
        kind: ArrayKind::Linear {
            source: centre,
            count_expr: "3".into(),
            dx_expr: "5mm".into(),
            dy_expr: "0mm".into(),
            depopulation: None,
        },
        numbering: NumberingScheme::default(),
    });
    Fixture {
        sketch,
        source,
        line_end,
        line,
        dimension,
        polar,
        linear,
    }
}

#[test]
fn a_pattern_cannot_take_itself_as_source() {
    let mut f = fixture();
    assert_eq!(
        f.sketch.set_pattern_source(f.polar, f.polar),
        Err(PatternCycle {
            pattern: f.polar,
            through: f.polar
        })
    );
    assert!(f.sketch.pattern_sources.is_empty(), "nothing recorded");
}

#[test]
fn a_two_pattern_loop_is_refused() {
    let mut f = fixture();
    f.sketch.set_pattern_source(f.linear, f.polar).unwrap();
    let before = f.sketch.clone();
    assert!(f.sketch.set_pattern_source(f.polar, f.linear).is_err());
    assert_eq!(f.sketch, before, "the refused link changes nothing");
}

#[test]
fn a_loop_through_a_mirror_is_refused() {
    let mut f = fixture();
    let mirror = ArrayId::new();
    f.sketch
        .add_pattern_mirror(PatternMirror {
            id: mirror,
            pattern: f.polar,
            line: f.line,
        })
        .unwrap();
    assert!(f.sketch.set_pattern_source(f.polar, mirror).is_err());
    // A mirror of itself is refused too.
    assert!(
        f.sketch
            .add_pattern_mirror(PatternMirror {
                id: mirror,
                pattern: mirror,
                line: f.line,
            })
            .is_err()
    );
    assert_eq!(f.sketch.pattern_mirrors.len(), 1);
    assert_eq!(f.sketch.pattern_mirrors[0].pattern, f.polar);
}

#[test]
fn a_link_onto_a_chain_that_already_loops_is_refused() {
    let mut f = fixture();
    // A damaged file: the two arrays name each other.
    f.sketch.pattern_sources = vec![
        PatternSource {
            array: f.linear,
            source: f.polar,
        },
        PatternSource {
            array: f.polar,
            source: f.linear,
        },
    ];
    let mirror = ArrayId::new();
    let refused = f.sketch.add_pattern_mirror(PatternMirror {
        id: mirror,
        pattern: f.linear,
        line: f.line,
    });
    assert!(refused.is_err());
    assert!(f.sketch.pattern_chain_loops(f.linear));
    assert_eq!(f.sketch.innermost_source(f.linear), None);
}

#[test]
fn a_mirror_of_a_pattern_of_a_pattern_is_allowed() {
    let mut f = fixture();
    f.sketch.set_pattern_source(f.linear, f.polar).unwrap();
    let ArrayKind::Linear { source, .. } = &f.sketch.arrays[1].kind else {
        panic!("still linear");
    };
    assert_eq!(*source, f.source, "the kind names the innermost source");
    let mirror = ArrayId::new();
    f.sketch
        .add_pattern_mirror(PatternMirror {
            id: mirror,
            pattern: f.linear,
            line: f.line,
        })
        .unwrap();
    assert_eq!(f.sketch.pattern_input(mirror), Some(f.linear));
    assert_eq!(f.sketch.pattern_input(f.linear), Some(f.polar));
    assert_eq!(f.sketch.pattern_input(f.polar), None);
    assert!(f.sketch.pattern_reaches(mirror, f.polar));
    assert_eq!(f.sketch.innermost_source(mirror), Some(f.source));
    // A further array may repeat the mirror.
    let outer = ArrayId::new();
    f.sketch.arrays.push(Array {
        id: outer,
        kind: ArrayKind::Linear {
            source: f.source,
            count_expr: "2".into(),
            dx_expr: "0mm".into(),
            dy_expr: "20mm".into(),
            depopulation: None,
        },
        numbering: NumberingScheme::default(),
    });
    f.sketch.set_pattern_source(outer, mirror).unwrap();
    assert!(f.sketch.remove_pattern_mirror(mirror));
    assert!(!f.sketch.remove_pattern_mirror(mirror));
}

#[test]
fn deleting_the_mirror_line_drops_the_mirror_and_links_onto_it() {
    let mut f = fixture();
    let mirror = ArrayId::new();
    f.sketch
        .add_pattern_mirror(PatternMirror {
            id: mirror,
            pattern: f.polar,
            line: f.line,
        })
        .unwrap();
    // A mirror of the mirror, and the linear array repeating it.
    let second = ArrayId::new();
    f.sketch
        .add_pattern_mirror(PatternMirror {
            id: second,
            pattern: mirror,
            line: f.line,
        })
        .unwrap();
    f.sketch.set_pattern_source(f.linear, mirror).unwrap();
    f.sketch.driven_dimensions.push(f.dimension);
    f.sketch.set_override(
        mirror,
        1,
        0,
        PadAttrOverride {
            net: Some("GND".into()),
            ..PadAttrOverride::default()
        },
    );

    let removed = f.sketch.remove_entity_cascade(f.line_end);

    assert!(f.sketch.pattern_mirrors.is_empty(), "both mirrors go");
    assert!(removed.arrays.contains(&mirror) && removed.arrays.contains(&second));
    assert!(f.sketch.pattern_sources.is_empty(), "the link onto it goes");
    assert!(
        f.sketch.arrays.iter().any(|a| a.id == f.linear),
        "the dependent array stays"
    );
    assert!(
        f.sketch.pad_overrides.is_empty(),
        "the mirror's override goes"
    );
    assert!(f.sketch.driven_dimensions.is_empty(), "its dimension went");
    assert!(f.sketch.dangling_refs().is_empty());
}

#[test]
fn deleting_the_source_drops_every_pattern_built_on_it() {
    let mut f = fixture();
    f.sketch.set_pattern_source(f.linear, f.polar).unwrap();
    f.sketch.set_override(
        f.polar,
        2,
        0,
        PadAttrOverride {
            locked: Some(true),
            ..PadAttrOverride::default()
        },
    );
    let removed = f.sketch.remove_entity_cascade(f.source);
    assert!(f.sketch.arrays.is_empty(), "{removed:?}");
    assert!(f.sketch.pattern_sources.is_empty());
    assert!(f.sketch.pad_overrides.is_empty());
}

#[test]
fn a_mirror_line_the_sketch_lacks_is_dangling_and_repaired() {
    let mut f = fixture();
    let ghost = SketchEntityId::new();
    f.sketch.pattern_mirrors.push(PatternMirror {
        id: ArrayId::new(),
        pattern: f.polar,
        line: ghost,
    });
    assert_eq!(f.sketch.dangling_refs(), vec![ghost]);
    let removed = f.sketch.remove_entity_cascade(ghost);
    assert_eq!(removed.repaired, 1, "{removed:?}");
    assert!(f.sketch.pattern_mirrors.is_empty());
}

#[test]
fn an_override_records_only_what_changed_and_applies_back() {
    let before = PadAttr {
        number: "1".into(),
        rotation_expr: Some("90deg".into()),
        ..PadAttr::default()
    };
    let mut after = before.clone();
    after.net = "VCC".into();
    after.electrical_type = ElectricalType::Source;
    after.rotation_expr = None;
    after.drill = Some(DrillSpec {
        diameter_expr: "0.8mm".into(),
        slot_length_expr: None,
        plated: true,
    });
    let delta = PadAttrOverride::between(&before, &after);
    assert_eq!(delta.number, None, "unchanged attributes stay out");
    assert_eq!(delta.net.as_deref(), Some("VCC"));
    assert_eq!(delta.rotation_expr, Some(OverrideValue::Cleared));
    assert!(matches!(delta.drill, Some(OverrideValue::Set(_))));

    let mut applied = before.clone();
    delta.apply_to(&mut applied);
    assert_eq!(applied, after);

    let mut merged = PadAttrOverride {
        net: Some("GND".into()),
        locked: Some(true),
        ..PadAttrOverride::default()
    };
    merged.merge(&delta);
    assert_eq!(merged.net.as_deref(), Some("VCC"), "the later edit wins");
    assert_eq!(merged.locked, Some(true), "the earlier one stays");
    assert!(PadAttrOverride::between(&before, &before).is_empty());
}

#[test]
fn overrides_are_keyed_by_instance() {
    let mut sketch = SketchData::default();
    let array = ArrayId::new();
    let net = |n: &str| PadAttrOverride {
        net: Some(n.into()),
        ..PadAttrOverride::default()
    };
    sketch.set_override(array, 1, 0, net("A"));
    sketch.set_override(array, 2, 0, net("B"));
    sketch.set_override(array, 1, 0, net("C"));
    assert_eq!(sketch.pad_overrides.len(), 2);
    assert_eq!(sketch.override_for(array, 1, 0), Some(&net("C")));
    assert_eq!(sketch.override_for(array, 0, 0), None);
    assert!(sketch.clear_override(array, 2, 0));
    assert!(!sketch.clear_override(array, 2, 0));
    sketch.set_override(array, 1, 0, PadAttrOverride::default());
    assert!(sketch.pad_overrides.is_empty(), "an empty override clears");
}

/// Every new list round-trips through TOML, and a sketch without them
/// writes none of their keys.
#[test]
fn the_links_round_trip_and_stay_absent_when_empty() {
    let plain = fixture().sketch;
    let text = toml::to_string(&plain).unwrap();
    for key in [
        "driven_dimensions",
        "pattern_sources",
        "pattern_mirrors",
        "pad_overrides",
    ] {
        assert!(!text.contains(key), "{key} in {text}");
    }
    assert!(!plain.needs_format_1_2());

    let mut f = fixture();
    f.sketch.driven_dimensions.push(f.dimension);
    f.sketch.set_pattern_source(f.linear, f.polar).unwrap();
    let mirror = ArrayId::new();
    f.sketch
        .add_pattern_mirror(PatternMirror {
            id: mirror,
            pattern: f.linear,
            line: f.line,
        })
        .unwrap();
    f.sketch.pad_overrides.push(InstanceOverride {
        array: mirror,
        i: 3,
        j: 0,
        pad: PadAttrOverride {
            net: Some("GND".into()),
            hole_rotation_deg: Some(OverrideValue::Set(45.0)),
            rotation_expr: Some(OverrideValue::Cleared),
            drill: Some(OverrideValue::Set(DrillSpec {
                diameter_expr: "1mm".into(),
                slot_length_expr: Some("2mm".into()),
                plated: false,
            })),
            ..PadAttrOverride::default()
        },
    });
    let text = toml::to_string(&f.sketch).unwrap();
    let back: SketchData = toml::from_str(&text).unwrap();
    assert_eq!(back, f.sketch, "{text}");
    let json = serde_json::to_string(&f.sketch).unwrap();
    assert_eq!(serde_json::from_str::<SketchData>(&json).unwrap(), f.sketch);
}

#[test]
fn each_list_alone_asks_for_format_1_2() {
    let base = fixture();
    let mut driven = base.sketch.clone();
    driven.driven_dimensions.push(base.dimension);
    let mut linked = base.sketch.clone();
    linked.set_pattern_source(base.linear, base.polar).unwrap();
    let mut mirrored = base.sketch.clone();
    mirrored
        .add_pattern_mirror(PatternMirror {
            id: ArrayId::new(),
            pattern: base.polar,
            line: base.line,
        })
        .unwrap();
    let mut overridden = base.sketch.clone();
    overridden.set_override(
        base.polar,
        1,
        0,
        PadAttrOverride {
            locked: Some(true),
            ..PadAttrOverride::default()
        },
    );
    for sketch in [driven, linked, mirrored, overridden] {
        assert!(sketch.needs_format_1_2(), "{sketch:?}");
    }
}
