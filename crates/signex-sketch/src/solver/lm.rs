//! Levenberg–Marquardt iteration for the constraint solver.
//!
//! Solves the least-squares problem
//!
//!   minimise  ½ ‖r(x)‖²
//!
//! by iterating the damped Newton update
//!
//!   (JᵀJ + λI) Δx = −Jᵀr
//!   x ← x + Δx       (if step reduces |r|²)
//!   λ ← λ / 10       (good step — be more Gauss–Newton next time)
//!   λ ← λ · 10       (bad step — be more steepest-descent next time)
//!
//! Reference: *Numerical Recipes* (Press et al., 3rd ed.) §15.5
//! ("Nonlinear Models — Levenberg-Marquardt Method"). Implementation
//! is composed entirely from the in-house primitives in
//! [`crate::solver::math`] and the in-house dense LU in
//! [`crate::solver::linalg`]; no external numerical library source
//! consulted.
//!
//! What `solve_lm` returns
//! -----------------------
//! On success the [`SolveResult`] carries the final state vector,
//! the iteration count, the final residual norm `|r|`, and the
//! elapsed time in milliseconds (informational: it never steers the solve). The state vector layout
//! matches `pack(sketch).vector` — call
//! [`crate::solver::state::point_xy`] (or read `EntityIndex` directly)
//! to recover entity coordinates.
//!
//! On failure [`SolveError`] is returned with one of:
//! - `DidNotConverge` — `max_iters` exceeded without `|r| < tolerance`.
//! - `BudgetExceeded` — the work budget (see [`DEFAULT_WORK_BUDGET`]) ran
//!   out. The budget counts work units (see [`crate::solver::work`]), so
//!   it depends only on the sketch: the same sketch stops at the same
//!   iteration on any machine.
//! - `TooLarge` — a single step needs more work than the whole budget, so
//!   nothing was attempted.
//! - `DegenerateLine` — a constraint needs the direction of a zero-length
//!   line.
//! - `OverConstrained` — singular normal-equation matrix at any λ
//!   (LM theory says this should be impossible for finite λ, but
//!   underflow can still hit the LU pivot threshold). The list is
//!   empty here: no constraint can be named at this layer.
//!
//! A `λ` stall is also reported as `DidNotConverge`; the failure's
//! `stalled` flag lets [`crate::solver::Solver`] retry it and decide
//! whether a named conflict is the cause.

use std::time::Instant;

use crate::error::{SketchError, SolveError};
use crate::sketch::SketchData;
use crate::solver::jacobian::jacobian;
use crate::solver::linalg::{self, LinAlgError, LuDecomposition};
use crate::solver::math::{add_diag, axpy, matmul_ata, matvec_t, norm_sq, norm_vec};
use crate::solver::residual::{ResolvedParams, total_residual};
use crate::solver::state::{EntityIndex, pack};
use crate::solver::work::{WorkMeter, iteration_cost, jacobian_cost, row_space_cost};

/// Default work budget of one whole [`crate::solver::Solver::solve`], in work
/// units (see [`crate::solver::work`] for what one unit is and what each step
/// costs). Every LM run, the perturbed retry, the offset undo and the conflict
/// recovery draw on this one pool.
///
/// Measured in `--release` on an Apple-silicon Mac, one unit of LM work is
/// 0.8 to 1.0 ns, so a solve that exhausts the pool takes roughly 90 to
/// 150 ms (a conflicting 16-rectangle grid, n = 126: 123 to 152 ms across
/// runs and constructions; a jittered fully
/// constrained 64-rectangle grid, n = 384: 122 ms) and then fails with
/// `BudgetExceeded`. The solve runs synchronously on every drag frame, so
/// that is the longest a failing frame can be held. Sketches that solve, with
/// their LM work in units (wall time of the whole solve on that Mac): saddle
/// retry, n = 2: 894 (0.1 ms); 40-point chain, n = 78: 55.3 M (50 ms);
/// 16-rectangle grid, n = 126: 14.2 M (16 ms); 32-rectangle grid,
/// n = 254: 115.5 M (109 ms, of which the DOF analysis is 14 ms). The grids
/// are 10 by 5 mm rectangles, 0.15 mm off, six constraints each, one fixed
/// corner. Everything the former 50 ms wall-clock limit solved still solves.
///
/// The DOF analysis of a converged sketch is not in the pool (failing a
/// converged sketch would drop its geometry): see
/// [`crate::solver::work::WorkMeter::analysis_spent`]. It grows as `m·n²` and
/// is the dominant cost of a large, already-converged sketch: 92 ms at
/// n = 574 when under-constrained, 400 to 520 ms at n = 510 when fully
/// constrained. A converged sketch is never refused for its size; a sketch
/// that still needs LM steps and whose single step, `(2n + 2)·m + m·n² + n³`
/// work units, costs more than the whole pool is reported as
/// [`crate::error::SolveError::TooLarge`]: at the default budget that is
/// from n = 397 when m = n, or n = 437 when m = n / 2.
pub const DEFAULT_WORK_BUDGET: u64 = 125_000_000;

/// Initial Marquardt damping. Small enough that the first step is
/// roughly Gauss–Newton on a well-conditioned problem. Not configurable
/// via [`crate::solver::Solver`] or `solve_lm`; the damping schedule
/// is fixed.
pub const LAMBDA_INIT: f64 = 1e-3;

/// CRIT-6: upper bound on Marquardt damping. After ~300 consecutive
/// rejected steps `lambda` would otherwise grow to `1e297`; (JᵀJ + λI)
/// then overflows to Inf, the LU pivot check passes, and `lu_solve`
/// produces NaN that silently poisons the state vector. We bail out
/// before that happens with `DidNotConverge`.
pub const LAMBDA_MAX: f64 = 1e16;

/// Output of a single solve. The state vector layout matches
/// `pack(sketch).vector` — pair it with the [`EntityIndex`] returned
/// by `pack(...)` to recover individual entity coordinates.
#[derive(Debug, Clone)]
pub struct SolveResult {
    pub state: Vec<f64>,
    pub index: EntityIndex,
    pub iterations: usize,
    pub final_residual_norm: f64,
    pub elapsed_ms: u64,
}

/// Levenberg–Marquardt solve.
///
/// `work_budget` caps the work of the run in work units. `tolerance` is the linear
/// residual-norm threshold — `|r|² < tolerance²` declares convergence.
/// `max_iters` caps the iteration count before [`SolveError::DidNotConverge`]
/// is returned. The defaults baked into [`crate::solver::Solver`]
/// (`tolerance = 1e-12`, `max_iters = 100`) are appropriate for the
/// v0.13 sketch use case; callers can tighten them for unit tests or
/// for high-precision regression cases.
pub fn solve_lm(
    sketch: &SketchData,
    params: &ResolvedParams,
    work_budget: u64,
    tolerance: f64,
    max_iters: usize,
) -> Result<SolveResult, SolveError> {
    let mut meter = WorkMeter::new(work_budget);
    solve_lm_keeping_state(sketch, params, &mut meter, tolerance, max_iters)
        .map_err(|failure| failure.error)
}

/// A failed solve together with the state LM stopped at, so the caller
/// can inspect the residuals the iteration could not remove.
#[derive(Debug)]
pub struct LmFailure {
    pub error: SolveError,
    pub state: Vec<f64>,
    pub index: EntityIndex,
    /// `true` only when damping saturated: no step reduced `|r|` any
    /// more. NaN traps, budget exhaustion and iteration-cap exits are `false`.
    pub stalled: bool,
}

/// Why the LM loop stopped without converging.
enum LmExit {
    Stalled { iters: usize, norm: f64 },
    Failed(SolveError),
}

impl From<SolveError> for LmExit {
    fn from(e: SolveError) -> Self {
        Self::Failed(e)
    }
}

/// Size of the deterministic start offset, relative to the sketch extent.
const START_OFFSET_REL: f64 = 1e-3;

/// [`solve_lm`], but a failure carries the last accepted state.
pub fn solve_lm_keeping_state(
    sketch: &SketchData,
    params: &ResolvedParams,
    meter: &mut WorkMeter,
    tolerance: f64,
    max_iters: usize,
) -> Result<SolveResult, Box<LmFailure>> {
    run_lm(sketch, params, meter, tolerance, max_iters, Start::Seed)
}

/// [`solve_lm_keeping_state`] from a slightly moved start. A start on a
/// symmetric saddle has a zero gradient and stalls although the sketch is
/// solvable; every free variable is offset by a small fixed vector (no
/// RNG) so the retry is reproducible.
pub fn solve_lm_perturbed(
    sketch: &SketchData,
    params: &ResolvedParams,
    meter: &mut WorkMeter,
    tolerance: f64,
    max_iters: usize,
) -> Result<SolveResult, Box<LmFailure>> {
    run_lm(
        sketch,
        params,
        meter,
        tolerance,
        max_iters,
        Start::Perturbed,
    )
}

/// Where an LM run starts.
enum Start {
    /// The sketch's stored values.
    Seed,
    /// The stored values moved by the deterministic retry offset.
    Perturbed,
    /// An explicit state vector (the re-solve after an offset retry).
    From(Vec<f64>),
}

/// Project-and-re-solve passes after a retry. One pass is exact only to
/// first order where the constrained directions themselves move (a floppy
/// chain off an anchor); two or three bring the leftover offset to rounding
/// noise.
const UNDO_PASSES: usize = 3;

/// A pass that moves the state by less than this, relative to the sketch
/// extent, has nothing left to undo.
const UNDO_DONE_REL: f64 = 1e-12;

/// Take the retry's start offset back out of a converged solution (#765).
///
/// A perturbed retry moves every variable by ~[`START_OFFSET_REL`] of the
/// sketch extent. The constraints pin only some directions; along the rest
/// (a free point, or a point on a single distance circle) the offset would
/// stay and drift geometry the user never touched. So the offset is
/// projected onto the row space of the Jacobian, the directions the
/// constraints actually pin, which keeps the solution closest to the
/// stored values, and the sketch is re-solved from there. That repeats up
/// to [`UNDO_PASSES`] times, because one projection is exact only to first
/// order.
///
/// Every step draws on the shared `meter`. If the first pass cannot be paid
/// for (a [`SolveError::BudgetExceeded`] or [`SolveError::TooLarge`]), that
/// error is returned. If the first pass fails for any other reason (the
/// residual rows or the Jacobian cannot be evaluated, the row-space
/// projection has no answer, the re-solve does not converge), the result is
/// [`SolveError::RetryOffsetNotUndone`]: the retry's offset is still in the
/// geometry and, along a direction the constraints leave free, would drift
/// it, so it must not be handed out as a solution (#1053). A later pass that
/// fails after the first has succeeded is not an error: the offset is already
/// gone to first order, and the last good state is kept.
///
/// The one exception is a sketch with no constraint rows at all. Nothing pins
/// any variable, so the offset is simply put back: the stored values are the
/// answer ([`restore_unconstrained`]).
pub fn undo_retry_offset(
    sketch: &SketchData,
    params: &ResolvedParams,
    meter: &mut WorkMeter,
    tolerance: f64,
    max_iters: usize,
    result: SolveResult,
) -> Result<SolveResult, SolveError> {
    let original = pack(sketch).vector;
    let extent = original.iter().fold(1.0_f64, |m, v| m.max(v.abs()));
    let mut best = result;
    let mut polished = false;
    for _ in 0..UNDO_PASSES {
        // A failing pass ends the loop: after a completed pass the last good
        // state stands, before one it is an error.
        let not_undone = |cause: &str| SolveError::RetryOffsetNotUndone(cause.to_string());
        let Some(rows) = row_count(sketch, &best, params) else {
            if polished {
                break;
            }
            return Err(not_undone(
                "a constraint cannot be evaluated at the solved state",
            ));
        };
        if rows == 0 && !polished {
            return Ok(restore_unconstrained(best, &original));
        }
        let n = best.state.len();
        let projection_cost = jacobian_cost(n, rows).saturating_add(row_space_cost(n, rows));
        if let Err(e) = meter.charge(projection_cost) {
            if polished {
                break;
            }
            return Err(e);
        }
        let Ok(j) = jacobian(sketch, &best.state, &best.index, params) else {
            if polished {
                break;
            }
            return Err(not_undone("the Jacobian cannot be evaluated"));
        };
        let Some(start) = row_space_point(&j, &original, &best.state) else {
            if polished {
                break;
            }
            return Err(not_undone(
                "the offset cannot be projected onto the constraints",
            ));
        };
        let moved = start
            .iter()
            .zip(&best.state)
            .fold(0.0_f64, |m, (a, b)| m.max((a - b).abs()));
        let next = match run_lm(
            sketch,
            params,
            meter,
            tolerance,
            max_iters,
            Start::From(start),
        ) {
            Ok(next) => next,
            Err(failure) => {
                if polished {
                    break;
                }
                return Err(match failure.error {
                    e @ (SolveError::BudgetExceeded { .. } | SolveError::TooLarge { .. }) => e,
                    e => not_undone(&format!("the re-solve failed: {e}")),
                });
            }
        };
        best = SolveResult {
            iterations: best.iterations + next.iterations,
            elapsed_ms: best.elapsed_ms + next.elapsed_ms,
            ..next
        };
        polished = true;
        if moved <= UNDO_DONE_REL * extent {
            break;
        }
    }
    Ok(best)
}

/// Residual rows of `sketch` at a solved state, or `None` if a residual
/// cannot be evaluated.
fn row_count(sketch: &SketchData, at: &SolveResult, params: &ResolvedParams) -> Option<usize> {
    total_residual(sketch, &at.state, &at.index, params)
        .ok()
        .map(|r| r.len())
}

/// The sketch has no constraint rows, so no variable is pinned: put back
/// `original` on all of them. The residual is empty, so the reported norm
/// stays the state's own.
fn restore_unconstrained(mut result: SolveResult, original: &[f64]) -> SolveResult {
    result.state = original.to_vec();
    result
}

/// `original` plus only the part of `solved - original` that lies in the
/// row space of `j`: `original + Jᵀ (J Jᵀ + εI)⁻¹ J (solved - original)`.
/// The small ridge `ε` keeps redundant constraints (dependent rows) from
/// making `J Jᵀ` singular. With no constraints the row space is empty and
/// the result is `original` itself.
fn row_space_point(j: &[Vec<f64>], original: &[f64], solved: &[f64]) -> Option<Vec<f64>> {
    if j.is_empty() {
        return Some(original.to_vec());
    }
    let delta: Vec<f64> = solved.iter().zip(original).map(|(s, o)| s - o).collect();
    let m = j.len();
    let mut jjt = vec![vec![0.0; m]; m];
    for (a, row_a) in j.iter().enumerate() {
        for (b, row_b) in j.iter().enumerate() {
            jjt[a][b] = row_a.iter().zip(row_b).map(|(x, y)| x * y).sum();
        }
    }
    let scale = (0..m).fold(1.0_f64, |acc, k| acc.max(jjt[k][k]));
    for (k, row) in jjt.iter_mut().enumerate() {
        row[k] += 1e-12 * scale;
    }
    let j_delta: Vec<f64> = j
        .iter()
        .map(|row| row.iter().zip(&delta).map(|(x, d)| x * d).sum())
        .collect();
    let w = linalg::solve(&jjt, &j_delta).ok()?;
    let step = matvec_t(j, &w);
    let point: Vec<f64> = original.iter().zip(&step).map(|(o, s)| o + s).collect();
    point.iter().all(|v| v.is_finite()).then_some(point)
}

fn run_lm(
    sketch: &SketchData,
    params: &ResolvedParams,
    meter: &mut WorkMeter,
    tolerance: f64,
    max_iters: usize,
    start: Start,
) -> Result<SolveResult, Box<LmFailure>> {
    let started = Instant::now();
    let dangling = sketch.dangling_refs();
    if !dangling.is_empty() {
        // The caller reports the error; this stays at debug so a stuck
        // sketch is not logged twice.
        tracing::debug!(
            target: "signex::sketch",
            missing = dangling.len(),
            "sketch references entities that do not exist; solve refused"
        );
        return Err(Box::new(LmFailure {
            error: SolveError::DanglingReference(dangling),
            state: Vec::new(),
            index: EntityIndex::default(),
            stalled: false,
        }));
    }
    let packed = pack(sketch);
    let mut state = match start {
        Start::Seed => packed.vector.clone(),
        Start::Perturbed => {
            let mut state = packed.vector.clone();
            let extent = state.iter().fold(1.0_f64, |m, v| m.max(v.abs()));
            for (i, v) in state.iter_mut().enumerate() {
                // Distinct, non-zero offsets per variable: 0.5 .. 1.5.
                *v += START_OFFSET_REL * extent * (0.5 + (i % 7) as f64 / 6.0);
            }
            state
        }
        Start::From(state) => {
            debug_assert_eq!(state.len(), packed.vector.len(), "re-solve start length");
            if state.len() == packed.vector.len() {
                state
            } else {
                packed.vector.clone()
            }
        }
    };
    let outcome = iterate_lm(
        sketch,
        params,
        &packed.index,
        &mut state,
        meter,
        tolerance,
        max_iters,
    );
    match outcome {
        Ok((iterations, final_residual_norm)) => Ok(SolveResult {
            state,
            index: packed.index,
            iterations,
            final_residual_norm,
            elapsed_ms: started.elapsed().as_millis() as u64,
        }),
        Err(exit) => {
            let (error, stalled) = match exit {
                LmExit::Stalled { iters, norm } => (
                    SolveError::DidNotConverge {
                        iters,
                        final_residual_norm: norm,
                    },
                    true,
                ),
                LmExit::Failed(error) => (error, false),
            };
            Err(Box::new(LmFailure {
                error,
                state,
                index: packed.index,
                stalled,
            }))
        }
    }
}

/// A residual that cannot be evaluated: a zero-length line is the user's to
/// fix and says so; anything else is the solver's own fault.
pub(crate) fn residual_failure(stage: &str, error: SketchError) -> SolveError {
    match error {
        SketchError::DegenerateLine(line) => SolveError::DegenerateLine(line),
        other => SolveError::Internal(format!("{stage}: {other}")),
    }
}

/// The LM loop. Advances `state` in place and returns
/// `(iterations, |r|)` on convergence.
fn iterate_lm(
    sketch: &SketchData,
    params: &ResolvedParams,
    index: &EntityIndex,
    state: &mut Vec<f64>,
    meter: &mut WorkMeter,
    tolerance: f64,
    max_iters: usize,
) -> Result<(usize, f64), LmExit> {
    // The initial residual comes first, even with nothing to solve for: a
    // constraint on only fixed geometry can still be unevaluable (a
    // zero-length line) and that must be reported, not skipped.
    let mut r = total_residual(sketch, state, index, params)
        .map_err(|e| residual_failure("initial residual", e))?;

    // Edge case: sketch with no free state vars — nothing to solve.
    if state.is_empty() {
        return Ok((0, norm_vec(&r)));
    }
    // Already within tolerance: converged, whatever the sketch's size. No
    // step is taken, so nothing is charged (a satisfied sketch must never be
    // refused for being large).
    let tol_sq = tolerance * tolerance;
    if norm_sq(&r) < tol_sq {
        return Ok((0, norm_vec(&r)));
    }
    let (vars, rows) = (state.len(), r.len());
    meter.charge(rows as u64)?;

    // Internally the LM step compares the squared residual norm
    // (saves a `sqrt` per iteration). `tolerance²` is the cutoff so
    // the public-API contract is a linear-norm threshold.
    let mut lambda = LAMBDA_INIT;
    let mut iterations = 0usize;

    let mut residual_norm_sq = norm_sq(&r);

    // Work of one iteration, see [`crate::solver::work`].
    let step_cost = iteration_cost(vars, rows);

    while iterations < max_iters {
        if residual_norm_sq < tol_sq {
            break;
        }

        // CRIT-6: divergence guard. Once `lambda` saturates we cannot
        // make progress without overflowing `(JᵀJ + λI)`.
        if lambda > LAMBDA_MAX {
            return Err(LmExit::Stalled {
                iters: iterations,
                norm: norm_vec(&r),
            });
        }

        meter.charge(step_cost)?;

        let j =
            jacobian(sketch, state, index, params).map_err(|e| residual_failure("jacobian", e))?;

        // Edge case: no constraints (m=0). Nothing to drive; converged.
        if j.is_empty() {
            break;
        }

        // Form the damped normal equations: (JᵀJ + λI) Δx = −Jᵀr.
        let mut a = matmul_ata(&j);
        add_diag(&mut a, lambda);
        // MD-5: in-place negate avoids a fresh `Vec` allocation per
        // iteration. `matvec_t` already builds the vector; flip signs
        // there instead of `.iter().map(|&v| -v).collect()`.
        let mut neg_jt_r = matvec_t(&j, &r);
        for v in &mut neg_jt_r {
            *v = -*v;
        }

        let lu = match LuDecomposition::new(&a) {
            Ok(lu) => lu,
            Err(LinAlgError::Singular) => {
                // λ already damps the diagonal; if even with damping
                // we hit a zero pivot, the geometry is genuinely
                // over-constrained. No constraint can be named here;
                // `Solver::solve` attributes stalls it can see.
                return Err(SolveError::OverConstrained(Vec::new()).into());
            }
            Err(LinAlgError::DimensionMismatch) => {
                // LO-12: code bug, not a user error.
                return Err(
                    SolveError::Internal("LU dimension mismatch — please report".into()).into(),
                );
            }
        };

        let delta = match lu.solve(&neg_jt_r) {
            Ok(d) => d,
            Err(e) => {
                return Err(SolveError::Internal(format!("LU back-substitution: {e:?}")).into());
            }
        };

        // Tentative step.
        let mut trial = state.clone();
        axpy(1.0, &delta, &mut trial);

        let trial_r = match total_residual(sketch, &trial, index, params) {
            Ok(rr) => rr,
            Err(_) => {
                lambda *= 10.0;
                iterations += 1;
                continue;
            }
        };
        let trial_norm_sq = norm_sq(&trial_r);

        // CRIT-6: NaN trap. If lambda saturates, the LU solve can yield
        // NaN deltas; trial_norm_sq becomes NaN; `NaN < residual_norm_sq`
        // is always false so we'd loop forever rejecting the step. Bail
        // out with DidNotConverge before max_iters runs out.
        if !trial_norm_sq.is_finite() {
            return Err(SolveError::DidNotConverge {
                iters: iterations,
                final_residual_norm: norm_vec(&r),
            }
            .into());
        }

        if trial_norm_sq < residual_norm_sq {
            *state = trial;
            r = trial_r;
            residual_norm_sq = trial_norm_sq;
            lambda /= 10.0;
        } else {
            lambda *= 10.0;
        }
        iterations += 1;
    }

    if residual_norm_sq >= tol_sq && iterations >= max_iters {
        return Err(SolveError::DidNotConverge {
            iters: iterations,
            final_residual_norm: norm_vec(&r),
        }
        .into());
    }

    // CRIT-6: defence-in-depth — never return a NaN-poisoned result.
    if !residual_norm_sq.is_finite() {
        return Err(SolveError::DidNotConverge {
            iters: iterations,
            final_residual_norm: norm_vec(&r),
        }
        .into());
    }

    Ok((iterations, norm_vec(&r)))
}
