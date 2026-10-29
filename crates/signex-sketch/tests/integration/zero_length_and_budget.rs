//! GH #685 — a zero-length line is reported as the user's geometry
//! problem, and the whole solve is bounded by one work count, not by the
//! clock.

use crate::common;
use common::Sketch;

use signex_sketch::SketchData;
use signex_sketch::constraint::{Constraint, ConstraintKind, DimTarget};
use signex_sketch::error::SolveError;
use signex_sketch::id::ConstraintId;
use signex_sketch::solver::FullSolveOutput;
use signex_sketch::solver::lm::{DEFAULT_WORK_BUDGET, solve_lm_keeping_state, solve_lm_perturbed};
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

/// What a solve produced, as one comparable string: every number as bits,
/// and its warnings.
fn outcome(result: &Result<FullSolveOutput, SolveError>) -> String {
    match result {
        Ok(out) => format!(
            "ok {} {} {:?} {:?}",
            out.result.iterations,
            out.result.final_residual_norm.to_bits(),
            out.result
                .state
                .iter()
                .map(|v| v.to_bits())
                .collect::<Vec<_>>(),
            // A pool one unit short skips the redundancy pass and says so.
            out.warnings
        ),
        Err(e) => format!("err {e:?}"),
    }
}

fn solve_with_budget(
    solver: &Solver,
    s: &Sketch,
    budget: u64,
) -> (Result<FullSolveOutput, SolveError>, u64) {
    let mut meter = WorkMeter::new(budget);
    let result = solver.solve_with_meter(&s.data, &ResolvedParams::new(), &mut meter);
    (result, meter.spent())
}

/// Work of one plain or perturbed LM run on its own meter.
fn lone_run_work(sketch: &SketchData, perturbed: bool) -> u64 {
    let solver = Solver::default();
    let mut meter = WorkMeter::new(u64::MAX);
    let params = ResolvedParams::new();
    let _ = if perturbed {
        solve_lm_perturbed(
            sketch,
            &params,
            &mut meter,
            solver.tolerance,
            solver.max_iters,
        )
    } else {
        solve_lm_keeping_state(
            sketch,
            &params,
            &mut meter,
            solver.tolerance,
            solver.max_iters,
        )
    };
    meter.spent()
}

/// The pool holds exactly what the whole solve needs: with `total` units the
/// result is the unlimited one, with one unit less it is not. This pins the
/// accounting itself. It does not notice a step that moved to a pool of its
/// own (that work just drops out of `total`); the two tests below do.
#[test]
fn the_pool_holds_exactly_the_work_of_the_whole_solve() {
    let solver = Solver::default();
    for (name, s) in recovery_sketches() {
        let (unlimited, total) = solve_with_budget(&solver, &s, u64::MAX);
        assert!(total > 0, "{name}: nothing was counted");

        let (at_total, spent) = solve_with_budget(&solver, &s, total);
        assert_eq!(outcome(&at_total), outcome(&unlimited), "{name}");
        assert_eq!(spent, total, "{name}");

        let (short, _) = solve_with_budget(&solver, &s, total - 1);
        assert_ne!(
            outcome(&short),
            outcome(&unlimited),
            "{name}: one unit short of {total} must change the outcome"
        );
    }
}

/// The retry of a stalled first run is work of the same solve: the total is
/// at least the first run plus the retry, each measured on its own meter, and
/// a pool one unit short of both cannot finish.
#[test]
fn the_retry_and_recovery_are_charged_to_the_solves_pool() {
    let solver = Solver::default();
    for (name, s) in recovery_sketches() {
        let first = lone_run_work(&s.data, false);
        let retry = lone_run_work(&s.data, true);
        let (_, total) = solve_with_budget(&solver, &s, u64::MAX);
        assert!(
            total >= first + retry,
            "{name}: total {total} is less than the first run {first} plus the retry {retry}"
        );

        let (short, _) = solve_with_budget(&solver, &s, first + retry - 1);
        assert!(
            matches!(short, Err(SolveError::BudgetExceeded { .. })),
            "{name}: {short:?}"
        );
    }
}

/// A conflict is narrowed with runs that draw on the same pool: the
/// unlimited conflict outcome costs more than detecting it, and a pool that
/// ends mid-narrowing still names every constraint of the conflict.
#[test]
fn conflict_narrowing_draws_on_the_same_pool() {
    let solver = Solver::default();
    let (_, s) = recovery_sketches().remove(1);
    let (unlimited, total) = solve_with_budget(&solver, &s, u64::MAX);
    let Err(SolveError::OverConstrained(narrowed)) = unlimited else {
        panic!("expected a conflict, got {unlimited:?}");
    };

    let detection = lone_run_work(&s.data, false) + lone_run_work(&s.data, true);
    let mut without = s.data.clone();
    without.constraints.retain(|c| !narrowed.contains(&c.id));
    let narrowing = lone_run_work(&without, false);
    assert!(
        total >= detection + narrowing,
        "total {total}: detection {detection} plus a run without the culprit {narrowing}"
    );

    for budget in (0..=total).step_by(3) {
        let (outcome, _) = solve_with_budget(&solver, &s, budget);
        if let Err(SolveError::OverConstrained(ids)) = outcome {
            assert!(
                narrowed.iter().all(|id| ids.contains(id)),
                "budget {budget}: {ids:?} leaves out part of {narrowed:?}"
            );
        }
    }
}

/// Redundant rows (m > n): a saddle start whose retry succeeds, but whose
/// undo projection (`2nm + m²n + m³`) costs more than the whole pool while
/// the cheaper `restore_unconstrained` charge (`2nm`) still fits. The retry
/// offset must not be returned as an `Ok` then (#765).
#[test]
fn an_undo_projection_larger_than_the_pool_is_too_large_not_ok() {
    let mut s = Sketch::new();
    let a = s.add_point(0.0, 0.0);
    let b = s.add_point(8.0, 0.0);
    let p = s.add_point(4.0, 0.0);
    for point in [a, b] {
        s.data
            .constraints
            .push(constraint(ConstraintKind::Fixed { point }));
    }
    for from in [a, b] {
        for _ in 0..100 {
            s.data
                .constraints
                .push(constraint(ConstraintKind::DistancePtPt {
                    p1: from,
                    p2: p,
                    target: DimTarget::Literal(5.0),
                }));
        }
    }
    // The fixed points are not variables and add no rows: n = 2, m = 200.
    let (n, m) = (2u64, 200u64);
    let projection = 2 * n * m + m * m * n + m * m * m;
    let budget = 100_000;
    assert!(projection > budget, "precondition: {projection}");
    let solver = Solver::default();

    let (unlimited, total) = solve_with_budget(&solver, &s, u64::MAX);
    assert!(unlimited.is_ok(), "precondition: {unlimited:?}");
    assert!(total > projection, "precondition: the undo is in the total");

    let (result, _) = solve_with_budget(&solver, &s, budget);
    assert!(
        matches!(result, Err(SolveError::TooLarge { needed, .. }) if needed == projection),
        "{:?}",
        result.map(|o| o.result.iterations)
    );
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
/// fits in the default budget (one step costs `(2n + 2)·m + m·n² + n³`, which
/// passes 125,000,000 from n = 397 when m = n).
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
