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
//! - `DidNotConverge` — `MAX_ITERS` exceeded without `|r|² < ε`.
//! - `Timeout` — wall-clock budget exceeded.
//! - `OverConstrained` — singular normal-equation matrix at any λ
//!   (LM theory says this should be impossible for finite λ, but
//!   underflow can still hit the LU pivot threshold; we surface it
//!   so the caller can show a useful error rather than retrying
//!   forever).

use std::time::Instant;

use crate::error::SolveError;
use crate::id::ConstraintId;
use crate::sketch::SketchData;
use crate::solver::linalg::{LinAlgError, LuDecomposition};
use crate::solver::math::{add_diag, axpy, matmul_ata, matvec_t, norm_sq, norm_vec};
use crate::solver::residual::{total_residual, ResolvedParams};
use crate::solver::state::{pack, EntityIndex};
use crate::solver::jacobian::numerical_jacobian;

/// Convergence tolerance — `|r|² < TOL_SQ` declares success.
pub const TOL_SQ: f64 = 1e-24;
/// Hard iteration cap — `MAX_ITERS` exceeded surfaces `DidNotConverge`.
pub const MAX_ITERS: usize = 100;
/// Initial Marquardt damping. Small enough that the first step is
/// roughly Gauss–Newton on a well-conditioned problem.
pub const LAMBDA_INIT: f64 = 1e-3;

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

/// Levenberg–Marquardt solve. Times out after `timeout_ms`
/// wall-clock milliseconds.
pub fn solve_lm(
    sketch: &SketchData,
    params: &ResolvedParams,
    timeout_ms: u64,
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

    let mut lambda = LAMBDA_INIT;
    let mut iterations = 0usize;

    let mut r = total_residual(sketch, &state, &packed.index, params)
        .map_err(|_| SolveError::OverConstrained(ConstraintId(uuid::Uuid::nil())))?;
    let mut residual_norm_sq = norm_sq(&r);

    while iterations < MAX_ITERS {
        if residual_norm_sq < TOL_SQ {
            break;
        }

        let elapsed_ms = started.elapsed().as_millis() as u64;
        if elapsed_ms > timeout_ms {
            return Err(SolveError::Timeout {
                elapsed_ms,
                budget_ms: timeout_ms,
            });
        }

        let j = numerical_jacobian(sketch, &state, &packed.index, params)
            .map_err(|_| SolveError::OverConstrained(ConstraintId(uuid::Uuid::nil())))?;

        // Edge case: no constraints (m=0). Nothing to drive; converged.
        if j.is_empty() {
            break;
        }

        // Form the damped normal equations: (JᵀJ + λI) Δx = −Jᵀr.
        let mut a = matmul_ata(&j);
        add_diag(&mut a, lambda);
        let neg_jt_r: Vec<f64> = matvec_t(&j, &r).iter().map(|&v| -v).collect();

        let lu = match LuDecomposition::new(&a) {
            Ok(lu) => lu,
            Err(LinAlgError::Singular) => {
                // λ already damps the diagonal; if even with damping
                // we hit a zero pivot, the geometry is genuinely
                // over-constrained.
                return Err(SolveError::OverConstrained(ConstraintId(uuid::Uuid::nil())));
            }
            Err(LinAlgError::DimensionMismatch) => {
                // The math module enforces shapes by construction;
                // hitting this means a code bug, not a user error.
                return Err(SolveError::OverConstrained(ConstraintId(uuid::Uuid::nil())));
            }
        };

        let delta = match lu.solve(&neg_jt_r) {
            Ok(d) => d,
            Err(_) => {
                return Err(SolveError::OverConstrained(ConstraintId(uuid::Uuid::nil())));
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

    if residual_norm_sq >= TOL_SQ && iterations >= MAX_ITERS {
        return Err(SolveError::DidNotConverge { iters: iterations });
    }

    Ok(SolveResult {
        state,
        index: packed.index,
        iterations,
        final_residual_norm: norm_vec(&r),
        elapsed_ms: started.elapsed().as_millis() as u64,
    })
}
