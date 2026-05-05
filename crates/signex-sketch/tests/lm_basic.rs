//! Task 3.3 — Levenberg–Marquardt iteration smoke tests.
//!
//! Phase 3.4 ships the full canonical-sketch corpus (rectangle,
//! parallelogram, isosceles triangle, regular hexagon). This file
//! covers the minimum cases needed to verify LM is correct on the
//! anchored-line case from the plan.

mod common;
use common::Sketch;

use signex_sketch::constraint::{Constraint, ConstraintKind, DimTarget};
use signex_sketch::id::ConstraintId;
use signex_sketch::solver::lm::solve_lm;
use signex_sketch::solver::residual::ResolvedParams;
use signex_sketch::solver::state::point_xy;

fn empty_params() -> ResolvedParams {
    ResolvedParams::new()
}

#[test]
fn lm_solves_anchored_horizontal_distance() {
    // Setup: P1 fixed at origin, P2 starts at (1, 0). Constraints:
    // Distance(P1, P2) = 5; Horizontal(line(P1, P2)). The fixed P1
    // anchors the system; the only free variables are P2.x, P2.y.
    // Expected solved P2 ≈ (5, 0).
    let mut s = Sketch::new();
    let p1 = s.add_point(0.0, 0.0);
    let p2 = s.add_point(1.0, 0.0);
    let line = s.add_line(p1, p2);

    s.data.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Fixed { point: p1 },
    });
    s.data.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::DistancePtPt {
            p1,
            p2,
            target: DimTarget::Literal(5.0),
        },
    });
    s.data.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Horizontal { line },
    });

    let result = solve_lm(&s.data, &empty_params(), 5_000, 1e-12, 100)
        .expect("LM should converge on anchored 5-mm horizontal line");

    let (x2, y2) = point_xy(p2, &result.state, &result.index, &s.data).unwrap();
    assert!(
        (x2 - 5.0).abs() < 1e-6,
        "P2.x converged to {x2}, expected 5.0"
    );
    assert!(y2.abs() < 1e-6, "P2.y converged to {y2}, expected 0.0");
    assert!(
        result.final_residual_norm < 1e-6,
        "final residual norm {} should be near zero",
        result.final_residual_norm
    );
    assert!(
        result.iterations <= 50,
        "should converge well within 50 iterations on a trivial problem (took {})",
        result.iterations
    );
}

#[test]
fn lm_solves_anchored_distance_in_either_direction() {
    // Same setup but with P2 starting on the OTHER side of P1: the
    // initial guess is (-1, 0), still 1 mm from P1 but in the
    // opposite direction. Horizontal+Distance has two solutions
    // (+5 mm and -5 mm); LM should converge to the nearer one — i.e.
    // (-5, 0) — because the descent is local.
    let mut s = Sketch::new();
    let p1 = s.add_point(0.0, 0.0);
    let p2 = s.add_point(-1.0, 0.0);
    let line = s.add_line(p1, p2);

    s.data.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Fixed { point: p1 },
    });
    s.data.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::DistancePtPt {
            p1,
            p2,
            target: DimTarget::Literal(5.0),
        },
    });
    s.data.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Horizontal { line },
    });

    let result = solve_lm(&s.data, &empty_params(), 5_000, 1e-12, 100)
        .expect("LM should converge on the negative branch as well");

    let (x2, y2) = point_xy(p2, &result.state, &result.index, &s.data).unwrap();
    assert!(
        (x2 + 5.0).abs() < 1e-6,
        "P2.x converged to {x2}, expected -5.0"
    );
    assert!(y2.abs() < 1e-6, "P2.y converged to {y2}, expected 0.0");
}

#[test]
fn lm_no_constraints_returns_immediately() {
    // Sketch with one free point and no constraints. solve_lm
    // should converge in zero iterations with the original state
    // unchanged.
    let mut s = Sketch::new();
    let p = s.add_point(2.5, 7.5);

    let result = solve_lm(&s.data, &empty_params(), 5_000, 1e-12, 100)
        .expect("LM should accept a constraint-free sketch");

    assert_eq!(result.iterations, 0);
    let (x, y) = point_xy(p, &result.state, &result.index, &s.data).unwrap();
    assert_eq!((x, y), (2.5, 7.5));
}

#[test]
fn lm_already_converged_returns_quickly() {
    // The constraints are already satisfied at the initial guess.
    // LM should detect this immediately and return without taking
    // any actual steps.
    let mut s = Sketch::new();
    let p1 = s.add_point(0.0, 0.0);
    let p2 = s.add_point(5.0, 0.0); // already 5 mm away on x-axis
    let line = s.add_line(p1, p2);

    s.data.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Fixed { point: p1 },
    });
    s.data.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::DistancePtPt {
            p1,
            p2,
            target: DimTarget::Literal(5.0),
        },
    });
    s.data.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Horizontal { line },
    });

    let result = solve_lm(&s.data, &empty_params(), 5_000, 1e-12, 100).unwrap();
    let (x2, y2) = point_xy(p2, &result.state, &result.index, &s.data).unwrap();
    assert!((x2 - 5.0).abs() < 1e-9);
    assert!(y2.abs() < 1e-9);
}
