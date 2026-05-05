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
/// trail produced by [`lu_decompose`]. Factor-once, solve-many API
/// shape: factor `(JᵀJ + λI)` once, then call [`LuDecomposition::solve`]
/// repeatedly against different right-hand sides without recomputing the
/// factorisation.
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

// ─────────────────────────────────────────────────────────────────────
// Householder QR decomposition (used by DOF rank analysis)
// ─────────────────────────────────────────────────────────────────────

/// Numerical-zero threshold for the QR Householder reflector. A
/// sub-vector below this magnitude is treated as a genuine zero and
/// the column is left untouched (the diagonal entry will land at zero
/// and contribute a "rank deficiency" to the count). Tighter than
/// `PIVOT_EPS` because QR is typically applied to (potentially)
/// rank-deficient inputs where the LU `Singular` branch is
/// counter-productive — DOF analysis WANTS to detect rank loss, not
/// reject it.
const QR_ZERO_EPS: f64 = 1e-300;

/// Result of a Householder QR factorisation: `R` is the upper-
/// triangular factor.
///
/// The DOF analysis only needs `rank(R)` — the rank of `A` equals the
/// rank of `R` because `Q` is orthogonal — so we deliberately do not
/// materialise `Q`. `R` is stored as a row-major `m × n` matrix; only
/// the upper triangle (`i ≤ j`) is meaningful, the lower triangle
/// holds whatever scratch the in-place factorisation left behind.
///
/// Reference: *Numerical Recipes* (Press et al., 3rd ed.) §2.10
/// ("QR Decomposition"). Algorithm derived from first principles —
/// no third-party numerical-library source consulted.
pub struct QrDecomposition {
    pub r: Vec<Vec<f64>>,
}

impl QrDecomposition {
    /// Factor an `m × n` matrix `A` using Householder reflections.
    /// Works for any shape (`m < n`, `m == n`, `m > n`).
    ///
    /// Algorithm (NR §2.10):
    ///
    /// 1. Copy `a` into a working buffer (the caller's input is
    ///    untouched).
    /// 2. For each column `k` in `0..min(m, n)`:
    ///    - Extract the sub-vector `x = a[k..m, k]`.
    ///    - Compute `α = -sign(x[0]) · |x|` so the reflector points
    ///      away from `x[0]` and avoids cancellation.
    ///    - Form `v = x − α · e_0` (i.e. `v[0] = x[0] − α`, the rest
    ///      of `v` is `x[1..]`), then normalise `v` to unit length.
    ///    - The reflector `H = I − 2vvᵀ` zeroes everything below row
    ///      `k` in column `k`. Apply it to the trailing submatrix
    ///      `a[k..m, k..n]` by, for each column `j` in `k..n`,
    ///      computing `β = 2 · v · a[k..m, j]` and updating
    ///      `a[k..m, j] -= β · v`.
    ///    - Overwrite the diagonal entry `a[k][k] = α` (the
    ///      Householder formula gives this exactly; we store it
    ///      explicitly to avoid roundoff drift).
    /// 3. The resulting upper triangle of `a` is `R`.
    ///
    /// Edge cases:
    /// - If `m == 0` or `n == 0`, the result is a degenerate
    ///   zero-rank factorisation (no reflections to apply).
    /// - If at column `k` the sub-vector `x` has Euclidean norm below
    ///   `QR_ZERO_EPS`, the column is genuinely zero in its lower
    ///   tail and we skip the reflection. `R[k][k]` will be zero (or
    ///   already-zero) and counted as a rank-deficient diagonal by
    ///   [`Self::rank`].
    pub fn new(a: &[Vec<f64>]) -> Result<Self, LinAlgError> {
        let m = a.len();
        if m == 0 {
            return Ok(Self { r: Vec::new() });
        }
        let n = a[0].len();
        for row in a {
            if row.len() != n {
                return Err(LinAlgError::DimensionMismatch);
            }
        }

        // Working buffer (clone so the caller's input is untouched).
        let mut r: Vec<Vec<f64>> = a.iter().map(|row| row.clone()).collect();

        let steps = m.min(n);
        for k in 0..steps {
            // 1. Compute |x| for the sub-vector x = r[k..m][k].
            let mut norm_sq: f64 = 0.0;
            for i in k..m {
                norm_sq += r[i][k] * r[i][k];
            }
            let norm = norm_sq.sqrt();

            if norm < QR_ZERO_EPS {
                // Column already zero in its lower tail. Nothing to
                // reflect; the diagonal entry stays whatever was
                // there (typically zero) and counts as rank-
                // deficient.
                continue;
            }

            // 2. α = -sign(x[0]) · |x|. If x[0] is exactly zero we
            //    pick the negative sign so v[0] = x[0] - α stays
            //    well-conditioned.
            let x0 = r[k][k];
            let alpha = if x0 >= 0.0 { -norm } else { norm };

            // 3. Build v = x - α e_0 (in place inside r[k..m][k]).
            r[k][k] = x0 - alpha;
            // r[k+1..m][k] already holds the rest of v.

            // 4. Compute v · v (used for β denominator).
            let mut vtv: f64 = 0.0;
            for i in k..m {
                vtv += r[i][k] * r[i][k];
            }
            if vtv < QR_ZERO_EPS {
                // Defensive: x was non-zero but v collapsed (only
                // happens if x[0] = α exactly which by our sign rule
                // is impossible for non-zero x). Skip; restore the
                // diagonal to α-equivalent.
                r[k][k] = alpha;
                continue;
            }

            // 5. Apply H = I − 2 vvᵀ / (vᵀv) to every column j in
            //    [k+1, n). For column k itself we know the result
            //    by construction: r[k][k] = α, r[k+1..m][k] = 0.
            for j in (k + 1)..n {
                let mut beta: f64 = 0.0;
                for i in k..m {
                    beta += r[i][k] * r[i][j];
                }
                let factor = 2.0 * beta / vtv;
                for i in k..m {
                    r[i][j] -= factor * r[i][k];
                }
            }

            // 6. Overwrite column k below row k with zeros and the
            //    diagonal with α. This is what the algorithm
            //    achieves analytically; we do it explicitly so
            //    rank-counting reads the clean triangular factor.
            r[k][k] = alpha;
            for i in (k + 1)..m {
                r[i][k] = 0.0;
            }
        }

        Ok(Self { r })
    }

    /// Numerical rank: count of diagonal entries `|R[i][i]| > tol`.
    /// `tol` is interpreted as an absolute threshold; for matrices
    /// of LM normal equations a value matching the LM tolerance
    /// (e.g. `1e-9`) classifies a singular value as "active".
    ///
    /// For an `m × n` matrix the diagonal runs from `(0,0)` to
    /// `(min(m,n)-1, min(m,n)-1)`. Empty matrices have rank 0.
    pub fn rank(&self, tol: f64) -> usize {
        let m = self.r.len();
        if m == 0 {
            return 0;
        }
        let n = self.r[0].len();
        let steps = m.min(n);
        let mut rank = 0;
        for i in 0..steps {
            if self.r[i][i].abs() > tol {
                rank += 1;
            }
        }
        rank
    }
}
