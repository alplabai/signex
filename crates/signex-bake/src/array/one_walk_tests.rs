//! #2689: every pattern places its instances through one walk. A plain
//! array bakes the pads its own layout gives (the step, or the turned source
//! less the source), the same whether or not one of its instances carries an
//! override, and the instance rings sit on the baked pads.

use std::collections::HashMap;

use signex_library::primitive::footprint::Pad as LibPad;
use signex_sketch::array::{Array, ArrayId, ArrayKind, GridDepopulation, NumberingScheme};
use signex_sketch::attr::{PadAttr, PadKind, PadShape, PadSide};
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
use signex_sketch::sketch::{PadAttrOverride, SketchData};
use signex_sketch::solver::residual::ResolvedParams;
use signex_sketch::solver::state::point_xy;
use signex_sketch::solver::{FullSolveOutput, Solver};

use crate::{bake_arrays, bake_pads, pattern_slots};

/// Off-grid places, so a sum that drifts by one ulp shows.
const SOURCE: (f64, f64) = (0.1, 0.2);
const CENTRE: (f64, f64) = (-0.3, 0.7);

struct Fixture {
    sketch: SketchData,
    array: ArrayId,
}

fn fixture(kind: impl Fn(SketchEntityId, SketchEntityId) -> ArrayKind) -> Fixture {
    let plane = PlaneId::new();
    let mut sketch = SketchData {
        planes: vec![Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        }],
        ..SketchData::default()
    };
    let (source, centre) = (SketchEntityId::new(), SketchEntityId::new());
    let mut pad = Entity::new(
        source,
        plane,
        EntityKind::Point {
            x: SOURCE.0,
            y: SOURCE.1,
        },
    );
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
    sketch.entities.push(Entity::new(
        centre,
        plane,
        EntityKind::Point {
            x: CENTRE.0,
            y: CENTRE.1,
        },
    ));
    let array = ArrayId::new();
    sketch.arrays.push(Array {
        id: array,
        kind: kind(source, centre),
        numbering: NumberingScheme::default(),
    });
    Fixture { sketch, array }
}

fn suppressing(cells: &[(u32, u32)]) -> Option<GridDepopulation> {
    Some(GridDepopulation {
        mask_expr: String::new(),
        suppressed_instances: cells.to_vec(),
    })
}

fn grid_suppressing_1_0() -> Fixture {
    fixture(|source, _| ArrayKind::Grid {
        source,
        nx_expr: "3".into(),
        ny_expr: "2".into(),
        dx_expr: "1.27mm".into(),
        dy_expr: "0.65mm".into(),
        depopulation: suppressing(&[(1, 0)]),
    })
}

fn polar_suppressing_2() -> Fixture {
    fixture(|source, center| ArrayKind::Polar {
        source,
        center,
        count_expr: "5".into(),
        sweep_angle_expr: "300deg".into(),
        depopulation: suppressing(&[(2, 0)]),
    })
}

fn linear_of_four() -> Fixture {
    fixture(|source, _| ArrayKind::Linear {
        source,
        count_expr: "4".into(),
        dx_expr: "1.27mm".into(),
        dy_expr: "0.3mm".into(),
    })
}

fn solve(sketch: &SketchData) -> FullSolveOutput {
    Solver::default()
        .solve(sketch, &ResolvedParams::new())
        .expect("the fixture solves")
}

fn xy(solve: &FullSolveOutput, sketch: &SketchData, at: (f64, f64)) -> (f64, f64) {
    let id = sketch
        .entities
        .iter()
        .find(|e| matches!(e.kind, EntityKind::Point { x, y } if (x, y) == at))
        .expect("the point")
        .id;
    point_xy(id, &solve.result.state, &solve.result.index, sketch).expect("solved")
}

/// What the pads and arrays bake together: each pad's number and place.
fn baked(sketch: &SketchData) -> Vec<(String, [f64; 2])> {
    let solved = solve(sketch);
    let (mut out, mut warnings): (Vec<LibPad>, Vec<String>) = (Vec::new(), Vec::new());
    bake_pads(sketch, &solved, &HashMap::new(), &mut out, &mut warnings).expect("pads bake");
    bake_arrays(sketch, &solved, &HashMap::new(), &mut out, &mut warnings).expect("arrays bake");
    assert!(warnings.is_empty(), "{warnings:?}");
    out.into_iter().map(|p| (p.number, p.position)).collect()
}

/// A pad placed `offset` from the source, as the pad bake adds it.
fn at_offset(source: (f64, f64), offset: [f64; 2]) -> [f64; 2] {
    [source.0 + offset[0] + 0.0, source.1 + offset[1] + 0.0]
}

fn numbered(numbers: &[&str], places: Vec<[f64; 2]>) -> Vec<(String, [f64; 2])> {
    numbers.iter().map(|n| n.to_string()).zip(places).collect()
}

#[test]
fn a_linear_array_bakes_each_step_from_the_source() {
    let f = linear_of_four();
    let s = xy(&solve(&f.sketch), &f.sketch, SOURCE);
    let places = (0..4)
        .map(|i| at_offset(s, [i as f64 * 1.27, i as f64 * 0.3]))
        .collect();
    assert_eq!(baked(&f.sketch), numbered(&["1", "2", "3", "4"], places));
}

#[test]
fn a_grid_bakes_each_kept_cell_from_the_source() {
    let f = grid_suppressing_1_0();
    let s = xy(&solve(&f.sketch), &f.sketch, SOURCE);
    let places = [(0, 0), (2, 0), (0, 1), (1, 1), (2, 1)]
        .into_iter()
        .map(|(i, j)| at_offset(s, [i as f64 * 1.27, j as f64 * 0.65]))
        .collect();
    assert_eq!(
        baked(&f.sketch),
        numbered(&["1", "3", "4", "5", "6"], places)
    );
}

#[test]
fn a_polar_array_bakes_each_kept_turn_of_the_source() {
    let f = polar_suppressing_2();
    let solved = solve(&f.sketch);
    let (s, c) = (
        xy(&solved, &f.sketch, SOURCE),
        xy(&solved, &f.sketch, CENTRE),
    );
    let sweep = signex_expr::parse_quantity("300deg")
        .unwrap()
        .as_rad()
        .unwrap();
    let (dx, dy) = (s.0 - c.0, s.1 - c.1);
    let places = [0, 1, 3, 4]
        .into_iter()
        .map(|i| {
            let (sin, cos) = (i as f64 * sweep / 5.0).sin_cos();
            let rx = c.0 + dx * cos - dy * sin;
            let ry = c.1 + dx * sin + dy * cos;
            at_offset(s, [rx - s.0, ry - s.1])
        })
        .collect();
    assert_eq!(baked(&f.sketch), numbered(&["1", "2", "4", "5"], places));
}

/// An override on one instance changes that pad only: every pad still
/// sits and is numbered where the plain array puts it.
#[test]
fn an_override_leaves_every_instance_where_the_plain_array_puts_it() {
    for (what, f, cell) in [
        ("linear", linear_of_four(), (2, 0)),
        ("grid", grid_suppressing_1_0(), (2, 1)),
        ("polar", polar_suppressing_2(), (3, 0)),
    ] {
        let plain = baked(&f.sketch);
        let mut sketch = f.sketch.clone();
        sketch.set_override(
            f.array,
            cell.0,
            cell.1,
            PadAttrOverride {
                net: Some("GND".into()),
                ..PadAttrOverride::default()
            },
        );
        assert_eq!(baked(&sketch), plain, "{what}");
    }
}

#[test]
fn the_instance_rings_sit_on_the_baked_pads() {
    for (what, f) in [
        ("linear", linear_of_four()),
        ("grid", grid_suppressing_1_0()),
        ("polar", polar_suppressing_2()),
    ] {
        let solved = solve(&f.sketch);
        let s = xy(&solved, &f.sketch, SOURCE);
        let rings: Vec<(f64, f64)> = pattern_slots(&f.sketch, &solved, f.array, s, 100)
            .expect("slots")
            .into_iter()
            .filter(|slot| !slot.suppressed)
            .map(|slot| slot.at)
            .collect();
        let pads = baked(&f.sketch);
        assert_eq!(rings.len(), pads.len(), "{what}");
        for ((x, y), (number, [px, py])) in rings.into_iter().zip(pads) {
            assert!(
                (x - px).abs() < 1e-12 && (y - py).abs() < 1e-12,
                "{what} pad {number}: ring ({x}, {y}), pad ({px}, {py})"
            );
        }
    }
}
