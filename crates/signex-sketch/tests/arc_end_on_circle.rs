//! GH #684 — an arc's end point must stay on the circle defined by its
//! centre and start point, however the sketch is solved or edited.

mod common;
use common::Sketch;

use signex_sketch::constraint::{Constraint, ConstraintKind, DimTarget};
use signex_sketch::id::{ConstraintId, SketchEntityId};
use signex_sketch::solver::Solver;
use signex_sketch::solver::residual::ResolvedParams;
use signex_sketch::solver::state::point_xy;

const TOL: f64 = 1e-6;

fn fixed(point: SketchEntityId) -> Constraint {
    Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Fixed { point },
    }
}

#[test]
fn off_circle_end_point_is_pulled_onto_the_circle() {
    // C = (0,0), S = (5,0), E = (0,7): E starts 2 mm off the circle.
    let mut s = Sketch::new();
    let c = s.add_point(0.0, 0.0);
    let start = s.add_point(5.0, 0.0);
    let end = s.add_point(0.0, 7.0);
    s.add_arc(c, start, end, true);

    let out = Solver::default()
        .solve(&s.data, &ResolvedParams::new())
        .expect("an arc alone is satisfiable");

    let xy = |p| point_xy(p, &out.result.state, &out.result.index, &s.data).unwrap();
    let (cx, cy) = xy(c);
    let (sx, sy) = xy(start);
    let (ex, ey) = xy(end);
    let r_start = (sx - cx).hypot(sy - cy);
    let r_end = (ex - cx).hypot(ey - cy);
    assert!(
        (r_start - r_end).abs() < TOL,
        "|S-C| = {r_start} but |E-C| = {r_end}"
    );
}

#[test]
fn end_point_stays_on_circle_while_dimensioned() {
    // C and S pinned; E is dimensioned 3 mm from S. A dimension must
    // not be able to move E off the r = 5 circle.
    let mut s = Sketch::new();
    let c = s.add_point(0.0, 0.0);
    let start = s.add_point(5.0, 0.0);
    let end = s.add_point(4.0, 3.0);
    s.add_arc(c, start, end, true);
    s.data.constraints.push(fixed(c));
    s.data.constraints.push(fixed(start));
    s.data.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::DistancePtPt {
            p1: start,
            p2: end,
            target: DimTarget::Literal(3.0),
        },
    });

    let out = Solver::default()
        .solve(&s.data, &ResolvedParams::new())
        .expect("circle and dimension intersect, so this is satisfiable");

    let (ex, ey) = point_xy(end, &out.result.state, &out.result.index, &s.data).unwrap();
    assert!(
        (ex.hypot(ey) - 5.0).abs() < TOL,
        "end left the circle: |E-C| = {}",
        ex.hypot(ey)
    );
    assert!(((ex - 5.0).hypot(ey) - 3.0).abs() < TOL);
}
