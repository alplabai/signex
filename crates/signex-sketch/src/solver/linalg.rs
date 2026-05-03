//! Dense LU linear solver with partial pivoting for small systems
//! (n < 200). Designed for the LM step's `(J^T J + λI) Δx = b`.
//!
//! Reference: *Numerical Recipes* (Press et al., 3rd ed.) §2.3
//! ("LU Decomposition and Its Applications"). The algorithm is
//! standard textbook 1965-era numerical analysis: Gaussian
//! elimination with row pivots, packed into a single matrix per
//! the conventional LAPACK convention where L's unit diagonal is
//! implicit and U occupies the upper triangle.
//!
//! Cleanroom: no third-party numerical library source consulted.

/// Tolerance below which a pivot is considered numerically zero.
///
/// `1e-300` sits just above the f64 denormal range (~2.2e-308), so
/// any pivot smaller than this has effectively underflowed to a
/// useless magnitude. The LM iteration upstream regularises the
/// normal equations with `+ λI`, so genuinely ill-conditioned but
/// non-zero pivots are kept; only true zero pivots produce a
/// `Singular` error.
const PIVOT_EPS: f64 = 1e-300;

/// Errors returned by the dense LU solver.
#[derive(Debug)]
pub enum LinAlgError {
    /// Matrix is non-square or `b` length mismatches the matrix.
    DimensionMismatch,
    /// LU decomposition encountered a zero pivot — matrix is
    /// numerically singular at the requested precision.
    Singular,
}

/// Solve `A x = b` via partial-pivot LU.
///
/// `A` is borrowed and cloned internally (the caller's matrix is
/// untouched); `b` is borrowed and cloned. Returns `x` as a fresh
/// `Vec<f64>` of length `n = A.len()`.
///
/// Errors:
/// - [`LinAlgError::DimensionMismatch`] if `A` is non-square or
///   `b.len() != A.len()`.
/// - [`LinAlgError::Singular`] if a zero pivot is encountered.
pub fn solve(a: &[Vec<f64>], b: &[f64]) -> Result<Vec<f64>, LinAlgError> {
    let n = a.len();
    if b.len() != n {
        return Err(LinAlgError::DimensionMismatch);
    }
    for row in a {
        if row.len() != n {
            return Err(LinAlgError::DimensionMismatch);
        }
    }

    let mut lu: Vec<Vec<f64>> = a.iter().map(|row| row.clone()).collect();
    let perm = lu_decompose(&mut lu)?;
    lu_solve(&lu, &perm, b)
}

/// Compute the LU decomposition in place with partial (row) pivoting.
///
/// On success the input matrix is overwritten with the packed LU
/// form: the strict lower triangle holds L (with an implicit unit
/// diagonal), and the upper triangle including the diagonal holds
/// U. The returned `perm` vector records the row pivots; entry
/// `perm[k] = r` means "during column k, rows k and r were
/// swapped". The same permutation must be applied to the right-
/// hand side before forward substitution — see [`lu_solve`].
///
/// Algorithm (NR §2.3):
/// 1. For each column `k` in `0..n`:
///    - **Pivot:** find row `r` in `[k, n)` with the largest
///      `|a[r][k]|`. If `|a[r][k]| < PIVOT_EPS`, return
///      [`LinAlgError::Singular`]. Swap rows `k` and `r` and
///      record `perm[k] = r`.
///    - **Eliminate:** for every row `i > k`,
///        `a[i][k] /= a[k][k]`            // store L's i-th
///                                        // multiplier in place
///        `a[i][j] -= a[i][k] * a[k][j]`  // for each j > k:
///                                        // update U's submatrix
pub fn lu_decompose(a: &mut [Vec<f64>]) -> Result<Vec<usize>, LinAlgError> {
    let n = a.len();
    for row in a.iter() {
        if row.len() != n {
            return Err(LinAlgError::DimensionMismatch);
        }
    }

    let mut perm: Vec<usize> = (0..n).collect();

    for k in 0..n {
        // Pivot: find the row in [k, n) whose column-k entry has
        // the largest absolute value.
        let mut pivot_row = k;
        let mut pivot_mag = a[k][k].abs();
        for r in (k + 1)..n {
            let m = a[r][k].abs();
            if m > pivot_mag {
                pivot_mag = m;
                pivot_row = r;
            }
        }

        if pivot_mag < PIVOT_EPS {
            return Err(LinAlgError::Singular);
        }

        if pivot_row != k {
            a.swap(k, pivot_row);
        }
        perm[k] = pivot_row;

        // Eliminate column k below the pivot.
        let pivot = a[k][k];
        for i in (k + 1)..n {
            let factor = a[i][k] / pivot;
            a[i][k] = factor;
            for j in (k + 1)..n {
                let akj = a[k][j];
                a[i][j] -= factor * akj;
            }
        }
    }

    Ok(perm)
}

/// Bundled LU factorisation: the packed `LU` matrix and the row-pivot
/// trail produced by [`lu_decompose`]. API patterned after nalgebra's
/// `LU` struct (Apache-2.0; we adopt the API shape, not the
/// implementation): factor once, then call [`LuDecomposition::solve`]
/// repeatedly with different right-hand sides.
///
/// LM uses this in the inner loop: factor `(JᵀJ + λI)` once per
/// iteration and solve against `−Jᵀr` without recomputing the
/// factorisation if the step is rejected and `λ` updates.
pub struct LuDecomposition {
    pub lu: Vec<Vec<f64>>,
    pub perm: Vec<usize>,
}

impl LuDecomposition {
    /// Factor a square matrix into its packed LU form. The input is
    /// borrowed and cloned internally so callers can reuse `a`.
    pub fn new(a: &[Vec<f64>]) -> Result<Self, LinAlgError> {
        let mut lu: Vec<Vec<f64>> = a.iter().map(|row| row.clone()).collect();
        let perm = lu_decompose(&mut lu)?;
        Ok(Self { lu, perm })
    }

    /// Solve `A x = b` against the cached factorisation.
    pub fn solve(&self, b: &[f64]) -> Result<Vec<f64>, LinAlgError> {
        lu_solve(&self.lu, &self.perm, b)
    }
}

/// Forward + back substitution given an LU-decomposed matrix and a
/// pivot permutation produced by [`lu_decompose`].
///
/// Steps:
/// 1. Apply the recorded row pivots to `b` in the same order they
///    were applied during decomposition.
/// 2. Forward-substitute through L (unit diagonal) to obtain `y`
///    such that `L y = P b`.
/// 3. Back-substitute through U to obtain `x` such that `U x = y`.
pub fn lu_solve(lu: &[Vec<f64>], perm: &[usize], b: &[f64]) -> Result<Vec<f64>, LinAlgError> {
    let n = lu.len();
    if b.len() != n || perm.len() != n {
        return Err(LinAlgError::DimensionMismatch);
    }
    for row in lu {
        if row.len() != n {
            return Err(LinAlgError::DimensionMismatch);
        }
    }

    // Apply the pivot trail to b in the same order as decomposition.
    let mut x = b.to_vec();
    for k in 0..n {
        let r = perm[k];
        if r != k {
            x.swap(k, r);
        }
    }

    // Forward substitution: L has an implicit unit diagonal stored
    // in the strict lower triangle of `lu`. Solve `L y = P b` in
    // place (x already holds P b on entry).
    for i in 0..n {
        let mut sum = x[i];
        for j in 0..i {
            sum -= lu[i][j] * x[j];
        }
        x[i] = sum;
    }

    // Back substitution: U occupies the upper triangle including
    // the diagonal. Solve `U x = y` in place (x holds y on entry).
    for i in (0..n).rev() {
        let mut sum = x[i];
        for j in (i + 1)..n {
            sum -= lu[i][j] * x[j];
        }
        let pivot = lu[i][i];
        if pivot.abs() < PIVOT_EPS {
            // Defensive: lu_decompose already rejects this, but
            // guard against a hand-constructed LU being passed in.
            return Err(LinAlgError::Singular);
        }
        x[i] = sum / pivot;
    }

    Ok(x)
}
