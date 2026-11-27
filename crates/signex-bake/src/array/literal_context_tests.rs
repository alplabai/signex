//! #2683: every array field and the pad rotation parse in the context its
//! value must have. A pitch is a length (`2.54m` is metres), a count and an
//! angle refuse SI multipliers (`4k7` is a parse error, never 4700).

use std::collections::HashMap;

use signex_library::primitive::footprint::Pad as LibPad;
use signex_sketch::SketchError;
use signex_sketch::array::{Array, ArrayId, ArrayKind, NumberingScheme};
use signex_sketch::attr::{PadAttr, PadKind, PadShape, PadSide};
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::expr::eval::EvalContext;
use signex_sketch::id::SketchEntityId;
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
use signex_sketch::sketch::SketchData;
use signex_sketch::solver::Solver;
use signex_sketch::solver::residual::ResolvedParams;

use crate::{bake_arrays, pattern_slots};

/// A pad source at the origin, a centre at (-1, 0), and the array `kind`
/// built from them; the pad turned by `rotation`.
fn sketch_with(
    rotation: Option<&str>,
    kind: impl Fn(SketchEntityId, SketchEntityId) -> ArrayKind,
) -> (SketchData, ArrayId) {
    let plane = PlaneId::new();
    let mut sketch = SketchData {
        planes: vec![Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        }],
        ..SketchData::default()
    };
    let (source, center) = (SketchEntityId::new(), SketchEntityId::new());
    let mut pad = Entity::new(source, plane, EntityKind::Point { x: 0.0, y: 0.0 });
    pad.pad = Some(PadAttr {
        number: "1".into(),
        kind: PadKind::Smd,
        side: PadSide::Top,
        shape: PadShape::Rect,
        size_x_expr: "0.5mm".into(),
        size_y_expr: "0.5mm".into(),
        rotation_expr: rotation.map(str::to_owned),
        ..PadAttr::default()
    });
    sketch.entities.push(pad);
    sketch.entities.push(Entity::new(
        center,
        plane,
        EntityKind::Point { x: -1.0, y: 0.0 },
    ));
    let id = ArrayId::new();
    sketch.arrays.push(Array {
        id,
        kind: kind(source, center),
        numbering: NumberingScheme::default(),
    });
    (sketch, id)
}

fn linear(count: &str, dx: &str) -> impl Fn(SketchEntityId, SketchEntityId) -> ArrayKind {
    let (count, dx) = (count.to_owned(), dx.to_owned());
    move |source, _| ArrayKind::Linear {
        source,
        count_expr: count.clone(),
        dx_expr: dx.clone(),
        dy_expr: "0mm".into(),
    }
}

fn grid(nx: &str, dx: &str, dy: &str) -> impl Fn(SketchEntityId, SketchEntityId) -> ArrayKind {
    let (nx, dx, dy) = (nx.to_owned(), dx.to_owned(), dy.to_owned());
    move |source, _| ArrayKind::Grid {
        source,
        nx_expr: nx.clone(),
        ny_expr: "2".into(),
        dx_expr: dx.clone(),
        dy_expr: dy.clone(),
        depopulation: None,
    }
}

fn polar(count: &str, sweep: &str) -> impl Fn(SketchEntityId, SketchEntityId) -> ArrayKind {
    let (count, sweep) = (count.to_owned(), sweep.to_owned());
    move |source, center| ArrayKind::Polar {
        source,
        center,
        count_expr: count.clone(),
        sweep_angle_expr: sweep.clone(),
        depopulation: None,
    }
}

fn bake(sketch: &SketchData) -> Result<Vec<LibPad>, SketchError> {
    let solved = Solver::default()
        .solve(sketch, &ResolvedParams::new())
        .expect("the fixture solves");
    let (mut out, mut warnings) = (Vec::new(), Vec::new());
    bake_arrays(sketch, &solved, &HashMap::new(), &mut out, &mut warnings)?;
    Ok(out)
}

fn xs(pads: &[LibPad]) -> Vec<f64> {
    pads.iter().map(|p| p.position[0]).collect()
}

/// The millimetres `text` is worth in a length field such as a pad width.
fn as_a_length_field(text: &str) -> f64 {
    let ctx = EvalContext {
        params: Default::default(),
        array_index: None,
    };
    crate::eval_util::eval_mm(text, &ctx).expect("a length field reads it")
}

#[test]
fn a_linear_pitch_in_metres_is_metres() {
    let (sketch, _) = sketch_with(None, linear("2", "2.54m"));
    assert_eq!(xs(&bake(&sketch).unwrap()), [0.0, 2540.0]);
}

#[test]
fn a_linear_pitch_reads_4k7_as_a_length_field_does() {
    let (sketch, _) = sketch_with(None, linear("2", "4k7"));
    assert_eq!(xs(&bake(&sketch).unwrap()), [0.0, as_a_length_field("4k7")]);
}

#[test]
fn a_grid_pitch_in_metres_is_metres_on_both_axes() {
    let (sketch, _) = sketch_with(None, grid("2", "2.54m", "2.54m"));
    let pads = bake(&sketch).unwrap();
    let at: Vec<[f64; 2]> = pads.iter().map(|p| p.position).collect();
    assert_eq!(
        at,
        [[0.0, 0.0], [2540.0, 0.0], [0.0, 2540.0], [2540.0, 2540.0]]
    );
}

#[test]
fn a_grid_pitch_reads_4k7_as_a_length_field_does() {
    let (sketch, _) = sketch_with(None, grid("2", "4k7", "1mm"));
    let pads = bake(&sketch).unwrap();
    assert_eq!(pads[1].position[0], as_a_length_field("4k7"));
}

#[test]
fn the_instance_rings_read_a_pitch_in_metres_as_metres() {
    let (sketch, id) = sketch_with(None, linear("2", "2.54m"));
    let solved = Solver::default()
        .solve(&sketch, &ResolvedParams::new())
        .unwrap();
    let slots = pattern_slots(&sketch, &solved, id, (0.0, 0.0), 100).expect("slots");
    assert_eq!(slots[1].at, (2540.0, 0.0));
}

#[test]
fn a_count_refuses_si_multipliers() {
    for (what, kind) in [
        ("linear count", sketch_with(None, linear("4k7", "1mm"))),
        (
            "linear count, metre-like",
            sketch_with(None, linear("2m", "1mm")),
        ),
        ("grid nx", sketch_with(None, grid("1k", "1mm", "1mm"))),
        ("polar count", sketch_with(None, polar("4k7", "360deg"))),
    ] {
        let (sketch, _) = kind;
        assert!(
            matches!(bake(&sketch), Err(SketchError::Expr(_))),
            "{what}: an SI multiplier is a parse error"
        );
    }
}

#[test]
fn a_polar_sweep_refuses_si_multipliers() {
    for sweep in ["4k7", "10m", "1u"] {
        let (sketch, _) = sketch_with(None, polar("4", sweep));
        assert!(
            matches!(bake(&sketch), Err(SketchError::Expr(_))),
            "sweep {sweep} is a parse error"
        );
    }
    let (sketch, _) = sketch_with(None, polar("2", "180deg"));
    assert_eq!(bake(&sketch).unwrap().len(), 2, "a sweep in degrees bakes");
}

#[test]
fn a_pad_rotation_refuses_si_multipliers() {
    for rotation in ["4k7", "10m", "5k"] {
        let (sketch, _) = sketch_with(Some(rotation), linear("1", "1mm"));
        assert!(
            matches!(bake(&sketch), Err(SketchError::Expr(_))),
            "rotation {rotation} is a parse error, not {rotation} as a multiplier"
        );
    }
}

#[test]
fn a_pad_rotation_in_degrees_or_a_plain_number_still_bakes() {
    for (rotation, deg) in [("45", 45.0), ("90deg", 90.0), ("=30 + 15", 45.0)] {
        let (sketch, _) = sketch_with(Some(rotation), linear("1", "1mm"));
        let pads = bake(&sketch).unwrap();
        assert!(
            (pads[0].rotation - deg).abs() < 1e-9,
            "{rotation}: {}",
            pads[0].rotation
        );
    }
}
