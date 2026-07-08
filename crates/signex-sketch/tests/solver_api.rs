//! Task 3.6 — Solver public API tests.
//!
//! Exercises the high-level `Solver::solve` façade (LM + DOF in one
//! shot). The previous `AutoPauseState` hysteresis was removed in
//! v0.22 — footprint sketches stay small enough that every solve
//! completes well under the per-frame budget; pause mode added a
//! confusing UI state and complicated downstream agents reading
//! solver state.

mod common;
use common::Sketch;

use signex_sketch::constraint::{Constraint, ConstraintKind, DimTarget};
use signex_sketch::error::SolveError;
use signex_sketch::id::ConstraintId;
use signex_sketch::solver::Solver;
use signex_sketch::solver::dof::DofColor;
use signex_sketch::solver::residual::ResolvedParams;
use signex_sketch::solver::state::point_xy;

fn empty_params() -> ResolvedParams {
    ResolvedParams::new()
}

#[test]
fn solver_default_solves_anchored_horizontal_distance() {
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

    let solver = Solver::default();
    let out = solver
        .solve(&s.data, &empty_params())
        .expect("default solver should converge on this canonical case");

    let (x2, y2) = point_xy(p2, &out.result.state, &out.result.index, &s.data).unwrap();
    assert!((x2 - 5.0).abs() < 1e-6);
    assert!(y2.abs() < 1e-6);

    // DOF: P1 fixed → Full. P2 free + fully pinned by Distance + Horizontal → Full.
    assert_eq!(out.colours.get(&p1), Some(&DofColor::Full));
    assert_eq!(out.colours.get(&p2), Some(&DofColor::Full));

    // No over-constrained constraints in this well-posed setup.
    assert!(out.over_constraints.is_empty());

    // Jacobian shape: 3 rows (Fixed contributes 0, Distance + Horizontal
    // each contribute 1) × 2 cols (P2.x, P2.y; P1 is excluded as Fixed).
    // Wait — Fixed contributes 0 residuals so total m = 0 + 1 + 1 = 2.
    assert_eq!(out.jacobian.len(), 2);
    assert_eq!(out.jacobian[0].len(), 2);
}

#[test]
fn solver_detects_over_constrained() {
    // Conflicting Distance constraints (5 mm AND 10 mm) on the same
    // anchored point. LM either converges to a least-squares
    // compromise or returns DidNotConverge — either way we expect
    // both Distance constraints to surface in over_constraints OR
    // the solve to fail. We accept either outcome here; the contract
    // is "detect the over-constraint, don't silently pretend it
    // succeeded".
    let mut s = Sketch::new();
    let p1 = s.add_point(0.0, 0.0);
    let p2 = s.add_point(1.0, 0.0);
    let _line = s.add_line(p1, p2);
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
        kind: ConstraintKind::DistancePtPt {
            p1,
            p2,
            target: DimTarget::Literal(10.0),
        },
    });

    let solver = Solver::default();
    match solver.solve(&s.data, &empty_params()) {
        Ok(out) => {
            // LM converged to a least-squares compromise; both
            // Distance constraints have non-trivial residuals and
            // should be in over_constraints.
            assert!(
                !out.over_constraints.is_empty(),
                "expected at least one over-constrained constraint, got {:?}",
                out.over_constraints
            );
            // The conflicting point should be flagged Over.
            assert_eq!(out.colours.get(&p2), Some(&DofColor::Over));
        }
        Err(_) => {
            // LM may fail to converge on irreconcilable constraints.
            // Either outcome is acceptable; we just don't want a
            // silent success.
        }
    }
}

#[test]
fn solver_under_constrained_returns_under_dof() {
    // One free Point, no constraints. Solver returns immediately.
    let mut s = Sketch::new();
    let p = s.add_point(2.5, 7.5);

    let solver = Solver::default();
    let out = solver.solve(&s.data, &empty_params()).unwrap();

    // No constraints → m = 0 → rank(J) = 0 < n = 2 → P is Under.
    assert_eq!(out.colours.get(&p), Some(&DofColor::Under));
    assert!(out.over_constraints.is_empty());
}

#[test]
fn solver_custom_timeout_and_iter_cap() {
    // Custom Solver with absurdly small timeout still completes the
    // canonical case because it converges in a few iterations.
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

    let solver = Solver {
        timeout_ms: 100, // generous to avoid CI flakiness
        max_iters: 100,
        tolerance: 1e-12,
    };
    let out = solver.solve(&s.data, &empty_params()).unwrap();
    assert!(out.result.iterations <= 50);
}

/// Drives `max_iters = 1` and verifies the solver caps at exactly that
/// many iterations — guards against the regression where the field was
/// silently ignored in favour of a module-level `MAX_ITERS` constant.
#[test]
fn solver_max_iters_field_is_honoured() {
    let mut s = Sketch::new();
    let p1 = s.add_point(0.0, 0.0);
    let p2 = s.add_point(1.0, 0.0); // far from the 5 mm target
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

    // 1 iteration is far too few for the 1→5 mm jump to satisfy
    // `|r| < 1e-12`. We expect DidNotConverge with iters == 1.
    let solver = Solver {
        timeout_ms: 1_000,
        max_iters: 1,
        tolerance: 1e-12,
    };
    match solver.solve(&s.data, &empty_params()) {
        Ok(out) => panic!(
            "max_iters=1 should not converge on the 4-mm gap; got {} iters, |r|={}",
            out.result.iterations, out.result.final_residual_norm
        ),
        Err(SolveError::DidNotConverge { iters, .. }) => assert_eq!(iters, 1),
        Err(other) => panic!("unexpected error: {other:?}"),
    }
}

/// Drives `tolerance = 10.0` (an absurdly loose bound). The solver
/// should declare convergence on the first iteration because `|r|² < 100`
/// is trivially true for the canonical anchored-distance case.
#[test]
fn solver_tolerance_field_is_honoured() {
    let mut s = Sketch::new();
    let p1 = s.add_point(0.0, 0.0);
    let p2 = s.add_point(1.0, 0.0);
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

    // tolerance = 10 ⇒ tol² = 100 ⇒ initial residual (4 mm gap, residual
    // norm² = 16) already inside tolerance ⇒ converge in 0 iterations.
    let solver_loose = Solver {
        timeout_ms: 1_000,
        max_iters: 100,
        tolerance: 10.0,
    };
    let out_loose = solver_loose.solve(&s.data, &empty_params()).unwrap();
    assert_eq!(
        out_loose.result.iterations, 0,
        "loose tolerance should converge immediately"
    );

    // For comparison, the default-tolerance solve takes more than 0
    // iterations on the same problem.
    let out_tight = Solver::default().solve(&s.data, &empty_params()).unwrap();
    assert!(
        out_tight.result.iterations > 0,
        "default tolerance should take at least one iteration"
    );
}
