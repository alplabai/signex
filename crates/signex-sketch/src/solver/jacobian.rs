//! Numerical Jacobian via central differences.
//!
//! The Jacobian J is an (m × n) matrix where m = total residual count
//! and n = state vector length. Each entry J[i][j] is dr_i/dx_j, the
//! sensitivity of residual i to state coordinate j. Computed by
//! finite difference at a step `H = 1e-7`:
//!
//!   J[i][j] ≈ (r_i(x + h e_j) − r_i(x − h e_j)) / (2h)
//!
//! Reference: *Numerical Recipes* (Press et al., 3rd ed.) §5.7
//! (numerical derivatives — central difference is second-order
//! accurate, h = 1e-7 chosen to balance truncation vs roundoff for
//! double-precision input on the unit interval).

use crate::error::SketchError;
use crate::sketch::SketchData;
use crate::solver::residual::{ResolvedParams, total_residual};
use crate::solver::state::EntityIndex;

/// Step size for central-difference numerical differentiation.
/// Half the cube root of double-precision epsilon (~1.5e-8) is the
/// classical optimum; we round up to 1e-7 for cleaner debugging.
pub const H: f64 = 1e-7;

/// Compute the (m × n) Jacobian of the total residual at `state`.
/// `m = total_residual.len()`; `n = state.len()`. The returned matrix
/// is row-major (`j[row][col]`).
///
/// Uses central differences. Each column requires two `total_residual`
/// evaluations, so the cost is `2n` residual evaluations per call.
pub fn numerical_jacobian(
    sketch: &SketchData,
    state: &[f64],
    index: &EntityIndex,
    params: &ResolvedParams,
) -> Result<Vec<Vec<f64>>, SketchError> {
    let n = state.len();

    // MD-3: avoid the wasted baseline `total_residual` call. We only
    // need it for the row count `m`, which the first central-difference
    // evaluation already provides.
    if n == 0 {
        // No state: zero columns. m can still be nonzero (parametric
        // constraints touch fixed points only) but the resulting
        // Jacobian has no columns to fill, so an empty matrix is
        // semantically correct.
        return Ok(Vec::new());
    }

    let mut state = state.to_vec();
    let saved = state[0];
    state[0] = saved + H;
    let r_plus0 = total_residual(sketch, &state, index, params)?;
    state[0] = saved - H;
    let r_minus0 = total_residual(sketch, &state, index, params)?;
    state[0] = saved;
    let m = r_plus0.len();
    let mut j = vec![vec![0.0; n]; m];
    for row in 0..m {
        j[row][0] = (r_plus0[row] - r_minus0[row]) / (2.0 * H);
    }

    for col in 1..n {
        let saved = state[col];
        state[col] = saved + H;
        let r_plus = total_residual(sketch, &state, index, params)?;
        state[col] = saved - H;
        let r_minus = total_residual(sketch, &state, index, params)?;
        state[col] = saved;
        for row in 0..m {
            j[row][col] = (r_plus[row] - r_minus[row]) / (2.0 * H);
        }
    }

    Ok(j)
}
