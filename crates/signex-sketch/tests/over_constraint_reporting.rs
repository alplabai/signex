//! GH #685 — a conflicting constraint set must come back from
//! `Solver::solve` as an over-constraint that names the constraints,
//! not as a generic non-convergence.

mod common;
use common::Sketch;

use signex_sketch::constraint::{Constraint, ConstraintKind, DimTarget};
use signex_sketch::error::SolveError;
use signex_sketch::id::ConstraintId;
use signex_sketch::solver::Solver;
use signex_sketch::solver::residual::ResolvedParams;

fn distance(
    p1: signex_sketch::id::SketchEntityId,
    p2: signex_sketch::id::SketchEntityId,
    mm: f64,
) -> Constraint {
    Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::DistancePtPt {
            p1,
            p2,
            target: DimTarget::Literal(mm),
        },
    }
}

#[test]
fn conflicting_distances_are_reported_as_over_constrained() {
    let mut s = Sketch::new();
    let p1 = s.add_point(0.0, 0.0);
    let p2 = s.add_point(1.0, 0.0);
    s.data.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Fixed { point: p1 },
    });
    let (five, ten) = (distance(p1, p2, 5.0), distance(p1, p2, 10.0));
    let (five_id, ten_id) = (five.id, ten.id);
    s.data.constraints.push(five);
    s.data.constraints.push(ten);

    let err = Solver::default()
        .solve(&s.data, &ResolvedParams::new())
        .expect_err("5 mm and 10 mm cannot both hold");

    assert!(matches!(err, SolveError::OverConstrained(_)), "{err:?}");
    let msg = err.to_string();
    assert!(msg.contains(&five_id.to_string()), "{msg}");
    assert!(msg.contains(&ten_id.to_string()), "{msg}");
}

#[test]
fn a_satisfiable_sketch_is_not_reported_as_over_constrained() {
    let mut s = Sketch::new();
    let p1 = s.add_point(0.0, 0.0);
    let p2 = s.add_point(1.0, 0.0);
    s.data.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Fixed { point: p1 },
    });
    s.data.constraints.push(distance(p1, p2, 5.0));

    let out = Solver::default()
        .solve(&s.data, &ResolvedParams::new())
        .expect("one distance is satisfiable");
    assert!(out.over_constraints.is_empty());
}
