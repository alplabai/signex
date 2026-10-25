//! GH #685 — a zero-length line is reported as the user's geometry
//! problem, and the solve is bounded by a work count, not by the clock.

mod common;
use common::Sketch;

use signex_sketch::constraint::{Constraint, ConstraintKind, DimTarget};
use signex_sketch::error::SolveError;
use signex_sketch::id::ConstraintId;
use signex_sketch::solver::lm::DEFAULT_WORK_BUDGET;
use signex_sketch::solver::{Solver, residual::ResolvedParams};

fn constraint(kind: ConstraintKind) -> Constraint {
    Constraint {
        id: ConstraintId::new(),
        kind,
    }
}

#[test]
fn a_constraint_on_a_zero_length_line_names_the_line() {
    for target in [None, Some(2.0)] {
        let mut s = Sketch::new();
        let a = s.add_point(1.0, 1.0);
        let b = s.add_point(1.0, 1.0);
        let p = s.add_point(3.0, 2.0);
        let line = s.add_line(a, b);
        s.data.constraints.push(constraint(match target {
            None => ConstraintKind::PointOnLine { point: p, line },
            Some(mm) => ConstraintKind::DistancePtLine {
                point: p,
                line,
                target: DimTarget::Literal(mm),
            },
        }));

        let err = common::solver()
            .solve(&s.data, &ResolvedParams::new())
            .expect_err("a zero-length line has no direction");

        assert!(
            matches!(err, SolveError::DegenerateLine(id) if id == line),
            "{err:?}"
        );
        let msg = err.to_string();
        assert!(msg.contains("zero length"), "{msg}");
        assert!(msg.contains(&line.to_string()), "{msg}");
    }
}

/// A chain of 40 points, spaced 1.05 mm, each link constrained to 1 mm.
fn chain() -> Sketch {
    let mut s = Sketch::new();
    let mut prev = s.add_point(0.0, 0.0);
    for i in 1..40 {
        let q = s.add_point(f64::from(i) * 1.05, 0.05 * f64::from(i % 3));
        s.data
            .constraints
            .push(constraint(ConstraintKind::DistancePtPt {
                p1: prev,
                p2: q,
                target: DimTarget::Literal(1.0),
            }));
        prev = q;
    }
    s
}

#[test]
fn a_spent_work_budget_fails_identically_every_time() {
    let s = chain();
    let solver = Solver {
        work_budget: 10_000,
        ..Solver::default()
    };

    let outcomes: Vec<String> = (0..6)
        .map(|_| {
            let err = solver
                .solve(&s.data, &ResolvedParams::new())
                .expect_err("10 000 evaluations cannot solve the chain");
            assert!(matches!(err, SolveError::BudgetExceeded { .. }), "{err:?}");
            err.to_string()
        })
        .collect();

    assert!(outcomes.windows(2).all(|w| w[0] == w[1]), "{outcomes:?}");
}

#[test]
fn the_default_budget_solves_the_chain_the_same_way_every_time() {
    let s = chain();
    let solver = Solver::default();
    assert_eq!(solver.work_budget, DEFAULT_WORK_BUDGET);

    let runs: Vec<(usize, u64)> = (0..4)
        .map(|_| {
            let out = solver
                .solve(&s.data, &ResolvedParams::new())
                .expect("the chain converges within the default budget");
            (
                out.result.iterations,
                out.result.final_residual_norm.to_bits(),
            )
        })
        .collect();

    assert!(runs.windows(2).all(|w| w[0] == w[1]), "{runs:?}");
}
