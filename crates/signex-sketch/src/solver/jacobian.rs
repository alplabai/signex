//! Analytic Jacobian of the total residual.
//!
//! The Jacobian J is an (m × n) matrix where m = total residual count
//! and n = state vector length; J[i][j] = dr_i/dx_j. Each constraint kind
//! supplies its own partial derivatives over only the variables it touches
//! ([`crate::solver::derivatives`]); this module concatenates them in the
//! row order of [`total_residual`] and scatters the sparse entries into the
//! dense matrix the LM normal equations and the QR rank test consume. No
//! residual is evaluated, so the cost is the number of non-zero entries,
//! not `2n` residual passes.
//!
//! [`total_residual`]: crate::solver::residual::total_residual

use crate::error::SketchError;
use crate::sketch::SketchData;
use crate::solver::derivatives::{SparseRow, arc_closure_rows, constraint_rows};
use crate::solver::residual::ResolvedParams;
use crate::solver::state::EntityIndex;

/// The Jacobian rows of `sketch` at `state` as sparse `(column, value)`
/// lists, one per entry of `total_residual` and in its order.
pub fn sparse_jacobian(
    sketch: &SketchData,
    state: &[f64],
    index: &EntityIndex,
    params: &ResolvedParams,
) -> Result<Vec<SparseRow>, SketchError> {
    let mut rows = Vec::new();
    for c in &sketch.constraints {
        rows.extend(constraint_rows(c, state, index, sketch, params)?);
    }
    rows.extend(arc_closure_rows(state, index, sketch)?);
    Ok(rows)
}

/// The (m × n) Jacobian of the total residual at `state`, row-major
/// (`j[row][col]`). Empty when the state has no variables.
pub fn jacobian(
    sketch: &SketchData,
    state: &[f64],
    index: &EntityIndex,
    params: &ResolvedParams,
) -> Result<Vec<Vec<f64>>, SketchError> {
    let n = state.len();
    if n == 0 {
        // No columns to fill, whatever m is.
        return Ok(Vec::new());
    }
    let rows = sparse_jacobian(sketch, state, index, params)?;
    let mut dense = vec![vec![0.0; n]; rows.len()];
    for (dense_row, row) in dense.iter_mut().zip(&rows) {
        for &(col, v) in row {
            dense_row[col] += v;
        }
    }
    Ok(dense)
}
