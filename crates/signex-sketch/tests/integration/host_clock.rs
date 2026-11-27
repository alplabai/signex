//! #2431 — the solver reads no clock: `elapsed_ms` is measured on the clock
//! the host passes in, around each LM run that converges, and the retry and
//! its offset-undo passes add up.

use std::cell::Cell;
use std::time::Duration;

use crate::common::Sketch;

use signex_sketch::constraint::{Constraint, ConstraintKind, DimTarget};
use signex_sketch::id::ConstraintId;
use signex_sketch::solver::Solver;
use signex_sketch::solver::residual::ResolvedParams;

/// Every read of the fake clock moves it on by this much.
const TICK_MS: u64 = 7;

thread_local! {
    static READS: Cell<u64> = const { Cell::new(0) };
}

fn ticking_clock() -> Duration {
    READS.with(|reads| {
        let n = reads.get();
        reads.set(n + 1);
        Duration::from_millis(n * TICK_MS)
    })
}

fn reads() -> u64 {
    READS.with(Cell::get)
}

fn timed_solver() -> Solver {
    READS.with(|reads| reads.set(0));
    Solver {
        clock: ticking_clock,
        ..Solver::default()
    }
}

fn constraint(kind: ConstraintKind) -> Constraint {
    Constraint {
        id: ConstraintId::new(),
        kind,
    }
}

#[test]
fn a_solve_that_converges_first_time_reports_one_lm_span() {
    let mut s = Sketch::new();
    let a = s.add_point(1.0, 2.0);
    s.data
        .constraints
        .push(constraint(ConstraintKind::Fixed { point: a }));

    let out = timed_solver()
        .solve(&s.data, &ResolvedParams::new())
        .expect("converges");

    assert_eq!(reads(), 2, "one read before and one after the LM run");
    assert_eq!(out.result.elapsed_ms, TICK_MS);
}

/// A saddle start: the first run stalls (it reads the clock once and adds
/// nothing), the perturbed retry converges and the offset undo re-solves at
/// least once. Each converged run adds one span, as before #2431.
#[test]
fn the_retry_and_its_undo_passes_add_up() {
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
        s.data
            .constraints
            .push(constraint(ConstraintKind::DistancePtPt {
                p1: from,
                p2: p,
                target: DimTarget::Literal(5.0),
            }));
    }

    let out = timed_solver()
        .solve(&s.data, &ResolvedParams::new())
        .expect("the retry converges");

    let total = reads();
    assert_eq!(total % 2, 1, "the stalled first run reads the clock once");
    let converged_runs = (total - 1) / 2;
    assert!(
        converged_runs >= 2,
        "retry plus at least one undo pass, got {converged_runs}"
    );
    assert_eq!(out.result.elapsed_ms, TICK_MS * converged_runs);
}
