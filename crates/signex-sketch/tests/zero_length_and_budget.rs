//! GH #685 — a zero-length line is reported as the user's geometry
//! problem, and the whole solve is bounded by one work count, not by the
//! clock.

mod common;
use common::Sketch;

use signex_sketch::constraint::{Constraint, ConstraintKind, DimTarget};
use signex_sketch::error::SolveError;
use signex_sketch::id::ConstraintId;
use signex_sketch::solver::lm::DEFAULT_WORK_BUDGET;
use signex_sketch::solver::work::WorkMeter;
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
        work_budget: 2_000_000,
        ..Solver::default()
    };

    let outcomes: Vec<String> = (0..6)
        .map(|_| {
            let err = solver
                .solve(&s.data, &ResolvedParams::new())
                .expect_err("2 million work units cannot solve the chain");
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

#[test]
fn a_step_larger_than_the_whole_budget_is_reported_as_too_large() {
    let s = chain();
    let solver = Solver {
        work_budget: 10_000,
        ..Solver::default()
    };

    let err = solver
        .solve(&s.data, &ResolvedParams::new())
        .expect_err("one LM step of the chain costs far more than 10 000");

    let SolveError::TooLarge { needed, budget } = err else {
        panic!("expected TooLarge, got {err:?}");
    };
    assert_eq!(budget, 10_000);
    assert!(needed > budget, "needed {needed}");
    let msg = err.to_string();
    assert!(msg.contains("too large"), "{msg}");
    assert!(msg.contains(&needed.to_string()), "{msg}");
}

/// Both points are fixed, so there is nothing to solve for, but the
/// constraint still needs the direction of a zero-length line.
#[test]
fn a_zero_length_line_on_fixed_points_only_is_still_reported() {
    let mut s = Sketch::new();
    let a = s.add_point(1.0, 1.0);
    let b = s.add_point(1.0, 1.0);
    let p = s.add_point(3.0, 2.0);
    let line = s.add_line(a, b);
    for point in [a, b, p] {
        s.data
            .constraints
            .push(constraint(ConstraintKind::Fixed { point }));
    }
    s.data
        .constraints
        .push(constraint(ConstraintKind::PointOnLine { point: p, line }));

    let err = common::solver()
        .solve(&s.data, &ResolvedParams::new())
        .expect_err("a zero-length line has no direction, fixed or not");

    assert!(
        matches!(err, SolveError::DegenerateLine(id) if id == line),
        "{err:?}"
    );
}

/// A saddle start (retry plus offset undo) and conflicts (recovery plus
/// narrowing): every extra LM run must draw on the one pool.
fn recovery_sketches() -> Vec<(&'static str, Sketch)> {
    let mut saddle = Sketch::new();
    let a = saddle.add_point(0.0, 0.0);
    let b = saddle.add_point(8.0, 0.0);
    let p = saddle.add_point(4.0, 0.0);
    for point in [a, b] {
        saddle
            .data
            .constraints
            .push(constraint(ConstraintKind::Fixed { point }));
    }
    for (from, mm) in [(a, 5.0), (b, 5.0)] {
        saddle
            .data
            .constraints
            .push(constraint(ConstraintKind::DistancePtPt {
                p1: from,
                p2: p,
                target: DimTarget::Literal(mm),
            }));
    }

    let mut conflict = Sketch::new();
    let a = conflict.add_point(0.0, 0.0);
    let b = conflict.add_point(1.0, 0.0);
    let c = conflict.add_point(0.5, 1.5);
    for point in [a, b] {
        conflict
            .data
            .constraints
            .push(constraint(ConstraintKind::Fixed { point }));
    }
    for (p1, p2, mm) in [(a, b, 5.0), (a, c, 2.0), (b, c, 2.5)] {
        conflict
            .data
            .constraints
            .push(constraint(ConstraintKind::DistancePtPt {
                p1,
                p2,
                target: DimTarget::Literal(mm),
            }));
    }
    vec![("saddle retry", saddle), ("conflict recovery", conflict)]
}

#[test]
fn one_budget_covers_the_whole_solve_retries_and_recovery_included() {
    let solver = Solver::default();
    for (name, s) in recovery_sketches() {
        let mut unlimited = WorkMeter::for_full_solve(u64::MAX);
        let _ = solver.solve_with_meter(&s.data, &ResolvedParams::new(), &mut unlimited);
        let total = unlimited.spent();
        assert!(total > 0, "{name}: nothing was counted");

        for budget in [total, total - 1, total / 2, total / 4, 1] {
            let mut meter = WorkMeter::for_full_solve(budget);
            let outcome = solver.solve_with_meter(&s.data, &ResolvedParams::new(), &mut meter);

            assert!(
                meter.spent() <= budget,
                "{name}: spent {} of a {budget} budget",
                meter.spent()
            );
            // Only the best-effort steps (undoing the retry offset, narrowing
            // a conflict) give way silently; the solve itself cannot
            // outspend the pool, and a pool of one unit cannot start it.
            if budget == 1 {
                assert!(
                    matches!(outcome, Err(SolveError::TooLarge { .. })),
                    "{name}: {outcome:?}"
                );
            }
        }
    }
}
