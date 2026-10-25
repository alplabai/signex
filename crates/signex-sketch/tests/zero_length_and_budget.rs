//! GH #685 — a zero-length line is reported as the user's geometry
//! problem, and the whole solve is bounded by one work count, not by the
//! clock.

mod common;
use common::Sketch;

use signex_sketch::constraint::{Constraint, ConstraintKind, DimTarget};
use signex_sketch::error::SolveError;
use signex_sketch::id::ConstraintId;
use signex_sketch::solver::lm::DEFAULT_WORK_BUDGET;
use signex_sketch::solver::state::point_xy;
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
        let mut unlimited = WorkMeter::new(u64::MAX);
        let _ = solver.solve_with_meter(&s.data, &ResolvedParams::new(), &mut unlimited);
        let total = unlimited.spent();
        assert!(total > 0, "{name}: nothing was counted");

        for budget in [total, total - 1, total / 2, total / 4, 1] {
            let mut meter = WorkMeter::new(budget);
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

/// A saddle start (so the solve takes the perturbed retry and the offset
/// undo), plus a point no constraint touches and a point sliding on a circle
/// round a fixed centre. The retry moves every variable by up to ~1e-3 of the
/// extent; whatever the budget, an `Ok` must not carry that offset (#765).
#[test]
fn an_ok_result_never_carries_the_retry_offset_at_any_budget() {
    let mut s = Sketch::new();
    let a = s.add_point(0.0, 0.0);
    let b = s.add_point(80.0, 0.0);
    let p = s.add_point(40.0, 0.0);
    let free = s.add_point(20.0, 20.0);
    let slider = s.add_point(30.0, 0.0);
    for point in [a, b] {
        s.data
            .constraints
            .push(constraint(ConstraintKind::Fixed { point }));
    }
    for (p1, p2, mm) in [(a, p, 50.0), (b, p, 50.0), (a, slider, 30.0)] {
        s.data
            .constraints
            .push(constraint(ConstraintKind::DistancePtPt {
                p1,
                p2,
                target: DimTarget::Literal(mm),
            }));
    }
    let solver = Solver::default();

    let mut unlimited = WorkMeter::new(u64::MAX);
    solver
        .solve_with_meter(&s.data, &ResolvedParams::new(), &mut unlimited)
        .expect("the sketch solves");
    let total = unlimited.spent();

    let mut checked_ok = 0;
    for step in 0..=100u64 {
        let budget = total / 2 + total / 2 * step / 100;
        let mut meter = WorkMeter::new(budget);
        let Ok(out) = solver.solve_with_meter(&s.data, &ResolvedParams::new(), &mut meter) else {
            continue;
        };
        checked_ok += 1;
        let at = |id| point_xy(id, &out.result.state, &out.result.index, &s.data).expect("free");
        for (name, id, (x0, y0)) in [
            ("free", free, (20.0, 20.0)),
            ("slider", slider, (30.0, 0.0)),
        ] {
            let (x, y) = at(id);
            let moved = ((x - x0).powi(2) + (y - y0).powi(2)).sqrt();
            assert!(
                moved <= 1e-6,
                "{name} moved {moved} mm at budget {budget} of {total}"
            );
        }
    }
    assert!(checked_ok > 0, "the full budget must produce an Ok result");
}

/// A satisfied sketch converges without a single step, so its size must not
/// matter: 10 by 5 mm rectangles, n = 574 and 1038, past the size one LM step
/// fits in the default budget (about n = 530).
#[test]
fn a_large_already_satisfied_sketch_solves_with_the_default_budget() {
    for rects in [72usize, 130] {
        let mut s = Sketch::new();
        let mut first = None;
        for r in 0..rects {
            let (ox, oy) = ((r % 10) as f64 * 14.0, (r / 10) as f64 * 9.0);
            let p: Vec<_> = [(0.0, 0.0), (10.0, 0.0), (10.0, 5.0), (0.0, 5.0)]
                .iter()
                .map(|(x, y)| s.add_point(ox + x, oy + y))
                .collect();
            first.get_or_insert(p[0]);
            let lines: Vec<_> = (0..4).map(|i| s.add_line(p[i], p[(i + 1) % 4])).collect();
            for kind in [
                ConstraintKind::Horizontal { line: lines[0] },
                ConstraintKind::Horizontal { line: lines[2] },
                ConstraintKind::Vertical { line: lines[1] },
                ConstraintKind::Vertical { line: lines[3] },
                ConstraintKind::DistancePtPt {
                    p1: p[0],
                    p2: p[1],
                    target: DimTarget::Literal(10.0),
                },
                ConstraintKind::DistancePtPt {
                    p1: p[1],
                    p2: p[2],
                    target: DimTarget::Literal(5.0),
                },
            ] {
                s.data.constraints.push(constraint(kind));
            }
        }
        let anchor = first.expect("a rectangle");
        s.data
            .constraints
            .push(constraint(ConstraintKind::Fixed { point: anchor }));
        let n = 8 * rects - 2;

        let out = Solver::default()
            .solve(&s.data, &ResolvedParams::new())
            .unwrap_or_else(|e| panic!("n = {n} is already satisfied: {e}"));
        assert_eq!(out.result.iterations, 0, "n = {n}");
    }
}
