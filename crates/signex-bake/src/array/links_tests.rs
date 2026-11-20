//! mockup row 191 / mockup row 192, item D1 — a pattern of a pattern, a live mirror, per-instance
//! overrides and `instance_at`, through the public bake entry points.

use std::collections::HashMap;

use signex_library::primitive::footprint::Pad as LibPad;
use signex_sketch::array::{Array, ArrayId, ArrayKind, NumberingScheme};
use signex_sketch::attr::{PadAttr, PadKind, PadShape, PadSide};
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
use signex_sketch::sketch::{
    OverrideValue, PadAttrOverride, PatternMirror, PatternSource, SketchData,
};
use signex_sketch::solver::residual::ResolvedParams;
use signex_sketch::solver::{FullSolveOutput, Solver};

use crate::{bake_arrays, bake_pads, instance_at};

/// A pad source at (1, 0), a polar centre at the origin and a vertical
/// Line at x = 5; a 4-way polar array of the source.
struct Fixture {
    sketch: SketchData,
    source: SketchEntityId,
    line: SketchEntityId,
    polar: ArrayId,
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
    let source = point(&mut sketch, 1.0, 0.0);
    sketch.entities.last_mut().unwrap().pad = Some(PadAttr {
        number: "1".into(),
        kind: PadKind::Smd,
        side: PadSide::Top,
        shape: PadShape::Rect,
        size_x_expr: "0.5mm".into(),
        size_y_expr: "0.5mm".into(),
        ..PadAttr::default()
    });
    let centre = point(&mut sketch, 0.0, 0.0);
    let a = point(&mut sketch, 5.0, -1.0);
    let b = point(&mut sketch, 5.0, 1.0);
    let line = SketchEntityId::new();
    sketch.entities.push(Entity::new(
        line,
        plane,
        EntityKind::Line { start: a, end: b },
    ));
    let polar = ArrayId::new();
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
    Fixture {
        sketch,
        source,
        line,
        polar,
    }
}

fn linear(source: SketchEntityId, count: u32, dx_mm: f64) -> Array {
    Array {
        id: ArrayId::new(),
        kind: ArrayKind::Linear {
            source,
            count_expr: count.to_string(),
            dx_expr: format!("{dx_mm}mm"),
            dy_expr: "0mm".into(),
        },
        numbering: NumberingScheme::default(),
    }
}

fn solve(sketch: &SketchData) -> FullSolveOutput {
    Solver::default()
        .solve(sketch, &ResolvedParams::new())
        .expect("the fixture solves")
}

/// Every pad the sketch bakes, and the warnings.
fn bake(sketch: &SketchData) -> (Vec<LibPad>, Vec<String>) {
    let solved = solve(sketch);
    let (mut out, mut warnings) = (Vec::new(), Vec::new());
    bake_pads(sketch, &solved, &HashMap::new(), &mut out, &mut warnings).expect("pads bake");
    bake_arrays(sketch, &solved, &HashMap::new(), &mut out, &mut warnings).expect("arrays bake");
    (out, warnings)
}

fn pad_at(pads: &[LibPad], x: f64, y: f64) -> &LibPad {
    pads.iter()
        .find(|p| (p.position[0] - x).abs() < 1e-6 && (p.position[1] - y).abs() < 1e-6)
        .unwrap_or_else(|| panic!("no pad at ({x}, {y}): {pads:?}"))
}

/// The polar ring's four places, turned through 90° each.
const RING: [(f64, f64); 4] = [(1.0, 0.0), (0.0, 1.0), (-1.0, 0.0), (0.0, -1.0)];

#[test]
fn a_linear_pattern_of_a_polar_pattern_bakes_every_instance() {
    let mut f = fixture();
    let outer = linear(f.source, 3, 10.0);
    let outer_id = outer.id;
    f.sketch.arrays.push(outer);
    f.sketch.set_pattern_source(outer_id, f.polar).unwrap();

    let (pads, warnings) = bake(&f.sketch);

    assert_eq!(pads.len(), 3 * 4, "the product of the counts: {warnings:?}");
    for step in 0..3 {
        for (k, &(x, y)) in RING.iter().enumerate() {
            let pad = pad_at(&pads, x + 10.0 * f64::from(step), y);
            let number = step * 4 + k as u32 + 1;
            assert_eq!(pad.number, number.to_string(), "slot {step}, instance {k}");
        }
    }
}

#[test]
fn a_mirror_reflects_every_instance_and_continues_the_numbering() {
    let mut f = fixture();
    let row = linear(f.source, 2, 2.0);
    let row_id = row.id;
    // The polar array goes: the row alone, at (1, 0) and (3, 0).
    f.sketch.arrays = vec![row];
    let mirror = ArrayId::new();
    f.sketch
        .add_pattern_mirror(PatternMirror {
            id: mirror,
            pattern: row_id,
            line: f.line,
        })
        .unwrap();

    let (pads, warnings) = bake(&f.sketch);

    assert_eq!(pads.len(), 4, "{warnings:?}");
    assert_eq!(pad_at(&pads, 1.0, 0.0).number, "1");
    assert_eq!(pad_at(&pads, 3.0, 0.0).number, "2");
    let image = pad_at(&pads, 9.0, 0.0);
    assert_eq!(
        image.number, "3",
        "the mirror continues the row's numbering"
    );
    assert_eq!(pad_at(&pads, 7.0, 0.0).number, "4");
    // About a vertical line a pad turns through 180°, the same copper.
    assert!((image.rotation - 180.0).abs() < 1e-9, "{}", image.rotation);
}

#[test]
fn a_mirror_follows_its_pattern_when_the_pattern_changes() {
    let mut f = fixture();
    let mirror = ArrayId::new();
    f.sketch
        .add_pattern_mirror(PatternMirror {
            id: mirror,
            pattern: f.polar,
            line: f.line,
        })
        .unwrap();
    let (pads, _) = bake(&f.sketch);
    assert_eq!(pads.len(), 8);
    // Six instances instead of four: the mirror bakes six too.
    if let ArrayKind::Polar { count_expr, .. } = &mut f.sketch.arrays[0].kind {
        *count_expr = "6".into();
    }
    let (pads, _) = bake(&f.sketch);
    assert_eq!(pads.len(), 12, "the mirror is live");
    pad_at(&pads, 9.0, 0.0);
}

#[test]
fn an_override_changes_its_instance_only() {
    let mut f = fixture();
    f.sketch.set_override(
        f.polar,
        2,
        0,
        PadAttrOverride {
            net: Some("GND".into()),
            rotation_expr: Some(OverrideValue::Set("45deg".into())),
            ..PadAttrOverride::default()
        },
    );

    let (pads, warnings) = bake(&f.sketch);

    assert_eq!(pads.len(), 4, "{warnings:?}");
    let hit = pad_at(&pads, -1.0, 0.0);
    assert_eq!(hit.net, "GND");
    assert!((hit.rotation - 45.0).abs() < 1e-9, "{}", hit.rotation);
    assert_eq!(hit.number, "3", "the numbering is the pattern's");
    for &(x, y) in RING.iter().filter(|&&p| p != (-1.0, 0.0)) {
        let pad = pad_at(&pads, x, y);
        assert_eq!(pad.net, "", "({x}, {y}) keeps the source net");
        assert!(pad.rotation.abs() < 1e-9);
    }
}

#[test]
fn an_override_of_a_mirrored_instance_changes_that_image_only() {
    let mut f = fixture();
    let mirror = ArrayId::new();
    f.sketch
        .add_pattern_mirror(PatternMirror {
            id: mirror,
            pattern: f.polar,
            line: f.line,
        })
        .unwrap();
    // Instance 0 of the polar array sits at (1, 0); its image at (9, 0).
    f.sketch.set_override(
        mirror,
        0,
        0,
        PadAttrOverride {
            net: Some("VCC".into()),
            ..PadAttrOverride::default()
        },
    );
    let (pads, _) = bake(&f.sketch);
    assert_eq!(pad_at(&pads, 9.0, 0.0).net, "VCC");
    assert_eq!(pad_at(&pads, 1.0, 0.0).net, "", "the original stays");
    assert_eq!(pads.iter().filter(|p| p.net == "VCC").count(), 1);
}

#[test]
fn instance_at_names_the_pattern_and_instance_of_a_pad() {
    let mut f = fixture();
    let outer = linear(f.source, 2, 20.0);
    let outer_id = outer.id;
    f.sketch.arrays.push(outer);
    f.sketch.set_pattern_source(outer_id, f.polar).unwrap();
    let mirror = ArrayId::new();
    f.sketch
        .add_pattern_mirror(PatternMirror {
            id: mirror,
            pattern: f.polar,
            line: f.line,
        })
        .unwrap();
    let solved = solve(&f.sketch);
    let at = |x, y| instance_at(&f.sketch, &solved, (x, y));

    assert_eq!(at(1.0, 0.0), Some((f.polar, 0, 0)), "the source pad");
    assert_eq!(at(-1.0, 0.0), Some((f.polar, 2, 0)));
    assert_eq!(
        at(20.0, 1.0),
        Some((outer_id, 1, 1)),
        "slot 1 of instance 1"
    );
    assert_eq!(at(9.0, 0.0), Some((mirror, 0, 0)));
    assert_eq!(at(11.0, 0.0), Some((mirror, 2, 0)), "the image of (-1, 0)");
    assert_eq!(at(2.5, 2.5), None);
}

#[test]
fn a_source_loop_in_a_damaged_file_is_refused_with_a_warning() {
    let mut f = fixture();
    let other = linear(f.source, 2, 3.0);
    let other_id = other.id;
    f.sketch.arrays.push(other);
    f.sketch.pattern_sources = vec![
        PatternSource {
            array: other_id,
            source: f.polar,
        },
        PatternSource {
            array: f.polar,
            source: other_id,
        },
    ];

    let (pads, warnings) = bake(&f.sketch);

    assert!(warnings.iter().any(|w| w.contains("loops")), "{warnings:?}");
    // Neither looped pattern places a pad; nothing hangs.
    assert!(pads.is_empty(), "{pads:?}");
}
