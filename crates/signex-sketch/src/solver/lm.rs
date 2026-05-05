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
//!   underflow can still hit the LU pivot threshold; we surface it
//!   so the caller can show a useful error rather than retrying
//!   forever).

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
    let started = Instant::now();
    let packed = pack(sketch);
    let mut state = packed.vector;

    // Edge case: sketch with no free state vars — nothing to solve.
    if state.is_empty() {
        return Ok(SolveResult {
            state,
            index: packed.index,
            iterations: 0,
            final_residual_norm: 0.0,
            elapsed_ms: started.elapsed().as_millis() as u64,
        });
    }

    // Internally the LM step compares the squared residual norm
    // (saves a `sqrt` per iteration). `tolerance²` is the cutoff so
    // the public-API contract is a linear-norm threshold.
    let tol_sq = tolerance * tolerance;
    let mut lambda = LAMBDA_INIT;
    let mut iterations = 0usize;

    let mut r = total_residual(sketch, &state, &packed.index, params)
        .map_err(|e| SolveError::Internal(format!("initial residual: {e}")))?;
    let mut residual_norm_sq = norm_sq(&r);

    while iterations < max_iters {
        if residual_norm_sq < tol_sq {
            break;
        }

        // CRIT-6: divergence guard. Once `lambda` saturates we cannot
        // make progress without overflowing `(JᵀJ + λI)`.
        if lambda > LAMBDA_MAX {
            return Err(SolveError::DidNotConverge {
                iters: iterations,
                final_residual_norm: norm_vec(&r),
            });
        }

        let elapsed_ms = started.elapsed().as_millis() as u64;
        if elapsed_ms > timeout_ms {
            return Err(SolveError::Timeout {
                elapsed_ms,
                budget_ms: timeout_ms,
            });
        }

        let j = numerical_jacobian(sketch, &state, &packed.index, params)
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
                // over-constrained. We don't have a single constraint
                // id to attribute this to, so callers should match on
                // OverConstrained without relying on the inner id.
                return Err(SolveError::OverConstrained(crate::id::ConstraintId(
                    uuid::Uuid::nil(),
                )));
            }
            Err(LinAlgError::DimensionMismatch) => {
                // LO-12: code bug, not a user error.
                return Err(SolveError::Internal(
                    "LU dimension mismatch — please report".into(),
                ));
            }
        };

        let delta = match lu.solve(&neg_jt_r) {
            Ok(d) => d,
            Err(e) => {
                return Err(SolveError::Internal(format!("LU back-substitution: {e:?}")));
            }
        };

        // Tentative step.
        let mut trial = state.clone();
        axpy(1.0, &delta, &mut trial);

        let trial_r = match total_residual(sketch, &trial, &packed.index, params) {
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
            });
        }

        if trial_norm_sq < residual_norm_sq {
            state = trial;
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
        });
    }

    // CRIT-6: defence-in-depth — never return a NaN-poisoned result.
    if !residual_norm_sq.is_finite() {
        return Err(SolveError::DidNotConverge {
            iters: iterations,
            final_residual_norm: norm_vec(&r),
        });
    }

    Ok(SolveResult {
        state,
        index: packed.index,
        iterations,
        final_residual_norm: norm_vec(&r),
        elapsed_ms: started.elapsed().as_millis() as u64,
    })
}
