//! GH #1053 — when the first offset-undo pass after a perturbed retry
//! fails, the retry's start offset is still in the geometry. That must come
//! back as an error, never as an `Ok` solution that has quietly slid a
//! partly constrained point along its circle.

mod common;
use common::Sketch;

use signex_sketch::SketchData;
use signex_sketch::constraint::{Constraint, ConstraintKind, DimTarget};
use signex_sketch::error::SolveError;
use signex_sketch::id::{ConstraintId, SketchEntityId};
use signex_sketch::solver::lm::{SolveResult, solve_lm_keeping_state, undo_retry_offset};
use signex_sketch::solver::residual::ResolvedParams;
use signex_sketch::solver::work::WorkMeter;

fn constraint(kind: ConstraintKind) -> Constraint {
    Constraint {
        id: ConstraintId::new(),
        kind,
    }
}

/// `a` fixed at the origin, `q` 99 mm up and held 100 mm from `a`: a
/// partly constrained point that may slide around the circle.
fn point_on_circle() -> (SketchData, SketchEntityId) {
    let mut s = Sketch::new();
    let a = s.add_point(0.0, 0.0);
    let q = s.add_point(0.0, 99.0);
    s.data
        .constraints
        .push(constraint(ConstraintKind::Fixed { point: a }));
    s.data
        .constraints
        .push(constraint(ConstraintKind::DistancePtPt {
            p1: a,
            p2: q,
            target: DimTarget::Literal(100.0),
        }));
    (s.data, q)
}

fn solved(data: &SketchData) -> SolveResult {
    let solver = common::solver();
    let mut meter = WorkMeter::new(u64::MAX);
    solve_lm_keeping_state(
        data,
        &ResolvedParams::new(),
        &mut meter,
        solver.tolerance,
        solver.max_iters,
    )
    .expect("the circle sketch converges")
}

fn undo(
    data: &SketchData,
    result: SolveResult,
    max_iters: usize,
) -> Result<SolveResult, SolveError> {
    let mut meter = WorkMeter::new(u64::MAX);
    undo_retry_offset(
        data,
        &ResolvedParams::new(),
        &mut meter,
        common::solver().tolerance,
        max_iters,
        result,
    )
}

/// Control: with a working re-solve the undo pass succeeds.
#[test]
fn a_working_undo_pass_returns_ok() {
    let (data, _) = point_on_circle();
    let result = solved(&data);
    let max_iters = common::solver().max_iters;
    undo(&data, result, max_iters).expect("the undo pass converges");
}

/// The re-solve cannot converge (no iterations allowed): the pass fails and
/// the result must be an error, not the retry's state with the offset in it.
#[test]
fn a_re_solve_that_does_not_converge_is_an_error() {
    let (data, _) = point_on_circle();
    let result = solved(&data);
    let err = undo(&data, result, 0).expect_err("the re-solve cannot converge");
    assert!(
        matches!(err, SolveError::RetryOffsetNotUndone(_)),
        "{err:?}"
    );
}

/// The constraint rows cannot be evaluated at the retry's state (a
/// zero-length line): the first pass fails, so the result is an error.
#[test]
fn an_unevaluable_state_is_an_error() {
    let mut s = Sketch::new();
    let a = s.add_point(0.0, 0.0);
    let b = s.add_point(4.0, 0.0);
    let p = s.add_point(3.0, 2.0);
    let line = s.add_line(a, b);
    s.data
        .constraints
        .push(constraint(ConstraintKind::PointOnLine { point: p, line }));
    let mut result = solved(&s.data);
    result.state.iter_mut().for_each(|v| *v = 1.0);
    let max_iters = common::solver().max_iters;
    let err = undo(&s.data, result, max_iters).expect_err("a zero-length line has no direction");
    assert!(
        matches!(err, SolveError::RetryOffsetNotUndone(_)),
        "{err:?}"
    );
}

/// The Jacobian is non-finite, so the row-space projection has no answer:
/// the first pass fails, so the result is an error.
#[test]
fn a_projection_that_has_no_answer_is_an_error() {
    let (data, _) = point_on_circle();
    let mut result = solved(&data);
    result.state[0] = f64::NAN;
    let max_iters = common::solver().max_iters;
    let err = undo(&data, result, max_iters).expect_err("a NaN state cannot be projected");
    assert!(
        matches!(err, SolveError::RetryOffsetNotUndone(_)),
        "{err:?}"
    );
}

/// With no constraint at all every variable is free; the retry's offset is
/// put back and that is a success, not an error.
#[test]
fn an_unconstrained_sketch_is_restored_not_rejected() {
    let mut s = Sketch::new();
    s.add_point(1.5, -2.5);
    let mut result = solved(&s.data);
    result.state.iter_mut().for_each(|v| *v += 1e-3);
    let max_iters = common::solver().max_iters;
    let out = undo(&s.data, result, max_iters).expect("nothing to undo against");
    assert_eq!(out.state, vec![1.5, -2.5]);
}
