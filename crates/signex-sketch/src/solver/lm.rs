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
//! wall-clock elapsed time in milliseconds. The state vector layout
//! matches `pack(sketch).vector` — call
//! [`crate::solver::state::point_xy`] (or read `EntityIndex` directly)
//! to recover entity coordinates.
//!
//! On failure [`SolveError`] is returned with one of:
//! - `DidNotConverge` — `max_iters` exceeded without `|r| < tolerance`.
//! - `Timeout` — wall-clock budget exceeded.
//! - `OverConstrained` — singular normal-equation matrix at any λ
//!   (LM theory says this should be impossible for finite λ, but
//!   underflow can still hit the LU pivot threshold). The list is
//!   empty here: no constraint can be named at this layer.
//!
//! A `λ` stall is also reported as `DidNotConverge`; the failure's
//! `stalled` flag lets [`crate::solver::Solver`] retry it and decide
//! whether a named conflict is the cause.

use std::time::Instant;

use crate::error::SolveError;
use crate::sketch::SketchData;
use crate::solver::jacobian::numerical_jacobian;
use crate::solver::linalg::{LinAlgError, LuDecomposition};
use crate::solver::math::{add_diag, axpy, matmul_ata, matvec_t, norm_sq, norm_vec};
use crate::solver::residual::{ResolvedParams, total_residual};
use crate::solver::state::{EntityIndex, pack};

/// Initial Marquardt damping. Small enough that the first step is
/// roughly Gauss–Newton on a well-conditioned problem. Not configurable
/// via [`crate::solver::Solver`]; callers that need a different damping
/// schedule must invoke `solve_lm` directly with custom parameters.
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
/// `timeout_ms` is the wall-clock budget. `tolerance` is the linear
/// residual-norm threshold — `|r|² < tolerance²` declares convergence.
/// `max_iters` caps the iteration count before [`SolveError::DidNotConverge`]
/// is returned. The defaults baked into [`crate::solver::Solver`]
/// (`tolerance = 1e-12`, `max_iters = 100`) are appropriate for the
/// v0.13 sketch use case; callers can tighten them for unit tests or
/// for high-precision regression cases.
pub fn solve_lm(
    sketch: &SketchData,
    params: &ResolvedParams,
    timeout_ms: u64,
    tolerance: f64,
    max_iters: usize,
) -> Result<SolveResult, SolveError> {
    solve_lm_keeping_state(sketch, params, timeout_ms, tolerance, max_iters)
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
    /// more. NaN traps, timeouts and iteration-cap exits are `false`.
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
    timeout_ms: u64,
    tolerance: f64,
    max_iters: usize,
) -> Result<SolveResult, Box<LmFailure>> {
    run_lm(sketch, params, timeout_ms, tolerance, max_iters, false)
}

/// [`solve_lm_keeping_state`] from a slightly moved start. A start on a
/// symmetric saddle has a zero gradient and stalls although the sketch is
/// solvable; every free variable is offset by a small fixed vector (no
/// RNG) so the retry is reproducible.
pub fn solve_lm_perturbed(
    sketch: &SketchData,
    params: &ResolvedParams,
    timeout_ms: u64,
    tolerance: f64,
    max_iters: usize,
) -> Result<SolveResult, Box<LmFailure>> {
    run_lm(sketch, params, timeout_ms, tolerance, max_iters, true)
}

/// A Jacobian entry below this counts as zero: no residual depends on the
/// variable. Real constraint derivatives are O(1); finite-difference noise
/// is orders of magnitude smaller.
const COLUMN_ZERO_TOL: f64 = 1e-6;

/// Undo the retry's start offset on variables nothing constrains. A
/// perturbed retry moves every variable by ~[`START_OFFSET_REL`] of the
/// sketch extent; the ones no residual depends on would otherwise keep that
/// offset and drift free geometry. Variables with a non-zero Jacobian column
/// keep the retry's values, so the result still satisfies every residual.
pub fn undo_offset_on_unconstrained(
    sketch: &SketchData,
    params: &ResolvedParams,
    mut result: SolveResult,
) -> SolveResult {
    let Ok(j) = numerical_jacobian(sketch, &result.state, &result.index, params) else {
        return result;
    };
    let original = pack(sketch).vector;
    for (k, value) in result.state.iter_mut().enumerate() {
        let depends = j.iter().any(|row| row[k].abs() > COLUMN_ZERO_TOL);
        if !depends {
            *value = original[k];
        }
    }
    result
}

fn run_lm(
    sketch: &SketchData,
    params: &ResolvedParams,
    timeout_ms: u64,
    tolerance: f64,
    max_iters: usize,
    perturb: bool,
) -> Result<SolveResult, Box<LmFailure>> {
    let started = Instant::now();
    let packed = pack(sketch);
    let mut state = packed.vector.clone();
    if perturb {
        let extent = state.iter().fold(1.0_f64, |m, v| m.max(v.abs()));
        for (i, v) in state.iter_mut().enumerate() {
            // Distinct, non-zero offsets per variable: 0.5 .. 1.5.
            *v += START_OFFSET_REL * extent * (0.5 + (i % 7) as f64 / 6.0);
        }
    }
    let outcome = iterate_lm(
        sketch,
        params,
        &packed.index,
        &mut state,
        started,
        timeout_ms,
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

/// The LM loop. Advances `state` in place and returns
/// `(iterations, |r|)` on convergence.
#[allow(
    clippy::too_many_arguments,
    reason = "internal loop; the arguments are the solve's own knobs plus the in-out state"
)]
fn iterate_lm(
    sketch: &SketchData,
    params: &ResolvedParams,
    index: &EntityIndex,
    state: &mut Vec<f64>,
    started: Instant,
    timeout_ms: u64,
    tolerance: f64,
    max_iters: usize,
) -> Result<(usize, f64), LmExit> {
    // Edge case: sketch with no free state vars — nothing to solve.
    if state.is_empty() {
        return Ok((0, 0.0));
    }

    // Internally the LM step compares the squared residual norm
    // (saves a `sqrt` per iteration). `tolerance²` is the cutoff so
    // the public-API contract is a linear-norm threshold.
    let tol_sq = tolerance * tolerance;
    let mut lambda = LAMBDA_INIT;
    let mut iterations = 0usize;

    let mut r = total_residual(sketch, state, index, params)
        .map_err(|e| SolveError::Internal(format!("initial residual: {e}")))?;
    let mut residual_norm_sq = norm_sq(&r);

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

        let elapsed_ms = started.elapsed().as_millis() as u64;
        if elapsed_ms > timeout_ms {
            return Err(SolveError::Timeout {
                elapsed_ms,
                budget_ms: timeout_ms,
            }
            .into());
        }

        let j = numerical_jacobian(sketch, state, index, params)
            .map_err(|e| SolveError::Internal(format!("jacobian: {e}")))?;

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
