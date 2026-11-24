//! A polar array's sweep turns by the angle in its written unit: `360deg`
//! over four instances steps 90°, and a sweep in radians is unchanged.

use std::collections::HashMap;

use signex_library::primitive::footprint::Pad as LibPad;
use signex_sketch::array::{Array, ArrayId, ArrayKind, NumberingScheme};
use signex_sketch::attr::{PadAttr, PadKind, PadShape, PadSide};
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
use signex_sketch::sketch::SketchData;
use signex_sketch::solver::Solver;
use signex_sketch::solver::residual::ResolvedParams;

use crate::{bake_arrays, bake_pads};

/// A pad source at (1, 0), the centre at the origin, and a 4-way polar
/// array of the source sweeping `sweep`.
fn polar_of_four(sweep: &str) -> SketchData {
    let plane = PlaneId::new();
    let mut sketch = SketchData {
        planes: vec![Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        }],
        ..SketchData::default()
    };
    let source = SketchEntityId::new();
    let mut pad = Entity::new(source, plane, EntityKind::Point { x: 1.0, y: 0.0 });
    pad.pad = Some(PadAttr {
        number: "1".into(),
        kind: PadKind::Smd,
        side: PadSide::Top,
        shape: PadShape::Rect,
        size_x_expr: "0.5mm".into(),
        size_y_expr: "0.5mm".into(),
        ..PadAttr::default()
    });
    sketch.entities.push(pad);
    let center = SketchEntityId::new();
    sketch.entities.push(Entity::new(
        center,
        plane,
        EntityKind::Point { x: 0.0, y: 0.0 },
    ));
    sketch.arrays.push(Array {
        id: ArrayId::new(),
        kind: ArrayKind::Polar {
            source,
            center,
            count_expr: "4".into(),
            sweep_angle_expr: sweep.into(),
            depopulation: None,
        },
        numbering: NumberingScheme::default(),
    });
    sketch
}

/// The pad positions the sketch bakes, sorted by number.
fn bake(sketch: &SketchData) -> Vec<(String, f64, f64)> {
    let solved = Solver::default()
        .solve(sketch, &ResolvedParams::new())
        .expect("the fixture solves");
    let (mut out, mut warnings): (Vec<LibPad>, Vec<String>) = (Vec::new(), Vec::new());
    bake_pads(sketch, &solved, &HashMap::new(), &mut out, &mut warnings).expect("pads bake");
    bake_arrays(sketch, &solved, &HashMap::new(), &mut out, &mut warnings).expect("arrays bake");
    assert!(
        !warnings.iter().any(|w| w.contains("sweep")),
        "{warnings:?}"
    );
    let mut pads: Vec<_> = out
        .into_iter()
        .map(|p| (p.number, p.position[0], p.position[1]))
        .collect();
    pads.sort_by(|a, b| a.0.cmp(&b.0));
    pads
}

fn assert_ring(pads: &[(String, f64, f64)]) {
    let ring = [(1.0, 0.0), (0.0, 1.0), (-1.0, 0.0), (0.0, -1.0)];
    assert_eq!(pads.len(), ring.len(), "{pads:?}");
    for ((number, x, y), (want_x, want_y)) in pads.iter().zip(ring) {
        assert!(
            (x - want_x).abs() < 1e-9 && (y - want_y).abs() < 1e-9,
            "pad {number} at ({x}, {y}), expected ({want_x}, {want_y})"
        );
    }
}

#[test]
fn a_sweep_in_degrees_steps_by_degrees() {
    assert_ring(&bake(&polar_of_four("360deg")));
}

#[test]
fn a_sweep_in_radians_is_unchanged() {
    assert_ring(&bake(&polar_of_four("6.283185307179586rad")));
}
