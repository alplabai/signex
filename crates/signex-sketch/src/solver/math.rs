//! Self-contained 2D vector + dense linear-algebra primitives used
//! across the solver. Everything is stdlib-only `f64`; no external
//! numerics crate is taken on.
//!
//! What lives here
//! ---------------
//! - **2D vector primitives** (`Vec2`, `dot`, `cross`, `norm`,
//!   `distance`, `wrap_to_pi`) — compose the Phase 2 residual
//!   functions in `solver/residuals/*.rs` from a common vocabulary so
//!   any algebraic update is local to one file.
//! - **Dense vector / matrix primitives** (`norm_sq`, `axpy`,
//!   `matvec`, `matmul_ata`, `add_diag`) — the building blocks of
//!   the LM normal-equation update `(JᵀJ + λI) Δx = −Jᵀr`.
//!
//! The LU factorisation lives in [`crate::solver::linalg`]; this
//! module deliberately only carries the *primitives* so each piece
//! is independently testable + benchmarkable.
//!
//! References:
//! - Hearn & Baker, *Computer Graphics with OpenGL*, ch. 5 (2D
//!   vector geometry).
//! - Press et al., *Numerical Recipes* (3rd ed.), §2.1 (vector and
//!   matrix conventions), §2.3 (Gaussian elimination — see
//!   `crate::solver::linalg`), §15.5 (Levenberg–Marquardt — see
//!   `crate::solver::lm`).
//!
//! No third-party numerical library source has been consulted; every
//! formula is derived from first-principles linear algebra.

// ─────────────────────────────────────────────────────────────────────
// 2D vector primitives
// ─────────────────────────────────────────────────────────────────────

/// 2D vector / point in plane-local coordinates (mm).
pub type Vec2 = (f64, f64);

/// Component-wise subtraction `a − b`.
#[inline]
pub fn sub(a: Vec2, b: Vec2) -> Vec2 {
    (a.0 - b.0, a.1 - b.1)
}

/// Component-wise addition `a + b`.
#[inline]
pub fn add(a: Vec2, b: Vec2) -> Vec2 {
    (a.0 + b.0, a.1 + b.1)
}

/// Scalar multiply `a · v`.
#[inline]
pub fn scale(s: f64, v: Vec2) -> Vec2 {
    (s * v.0, s * v.1)
}

/// Dot product `a · b = a.x·b.x + a.y·b.y`.
#[inline]
pub fn dot(a: Vec2, b: Vec2) -> f64 {
    a.0 * b.0 + a.1 * b.1
}

/// 2D scalar cross product `a × b = a.x·b.y − a.y·b.x`. Returns the
/// signed magnitude of the 3D cross's z-component, useful as a
/// "side of line" test (positive = `b` is left of `a`).
#[inline]
pub fn cross(a: Vec2, b: Vec2) -> f64 {
    a.0 * b.1 - a.1 * b.0
}

/// Euclidean norm `|v| = sqrt(v.x² + v.y²)`. Uses [`f64::hypot`]
/// for numerical stability on extreme-magnitude inputs.
#[inline]
pub fn norm(v: Vec2) -> f64 {
    v.0.hypot(v.1)
}

/// Squared Euclidean norm `|v|² = v.x² + v.y²`. Avoids the `sqrt`
/// when only relative magnitudes matter (e.g. distance comparisons,
/// LM convergence test on `|r|²`).
#[inline]
pub fn norm_sq_2(v: Vec2) -> f64 {
    v.0 * v.0 + v.1 * v.1
}

/// Euclidean distance between two points.
#[inline]
pub fn distance(a: Vec2, b: Vec2) -> f64 {
    norm(sub(b, a))
}

/// Wrap an angle into the principal range `(−π, π]`.
///
/// Computed as `θ − 2π · round(θ / 2π)` and then nudged so a value
/// at exactly `+π` stays `+π` (the half-open interval lives on the
/// negative side). This keeps the Angle constraint's residual
/// continuous across a sketch that crosses the ±π branch cut so the
/// LM driver sees a well-formed derivative instead of a 2π jump.
#[inline]
pub fn wrap_to_pi(theta: f64) -> f64 {
    use std::f64::consts::PI;
    let two_pi = 2.0 * PI;
    let mut t = theta - two_pi * (theta / two_pi).round();
    if t <= -PI {
        t += two_pi;
    } else if t > PI {
        t -= two_pi;
    }
    t
}

// ─────────────────────────────────────────────────────────────────────
// Dense vector + matrix primitives (used by LM)
// ─────────────────────────────────────────────────────────────────────

/// `|x|²` for a flat vector. The LM convergence test compares this
/// to a tolerance.
#[inline]
pub fn norm_sq(x: &[f64]) -> f64 {
    x.iter().map(|&xi| xi * xi).sum()
}

/// `|x|` for a flat vector.
#[inline]
pub fn norm_vec(x: &[f64]) -> f64 {
    norm_sq(x).sqrt()
}

/// In-place AXPY: `y[i] += α · x[i]` for all `i`. Panics on length
/// mismatch.
pub fn axpy(alpha: f64, x: &[f64], y: &mut [f64]) {
    assert_eq!(x.len(), y.len(), "axpy: length mismatch");
    for i in 0..x.len() {
        y[i] += alpha * x[i];
    }
}

/// `y = A · x` for an `m × n` matrix `A` (row-major, `Vec<Vec<f64>>`).
/// Returns a fresh `Vec<f64>` of length `m`. Panics if any row of
/// `A` has length ≠ `x.len()`.
pub fn matvec(a: &[Vec<f64>], x: &[f64]) -> Vec<f64> {
    let n = x.len();
    let mut y = Vec::with_capacity(a.len());
    for row in a {
        assert_eq!(row.len(), n, "matvec: row width mismatch");
        let mut sum = 0.0;
        for j in 0..n {
            sum += row[j] * x[j];
        }
        y.push(sum);
    }
    y
}

/// `y = Aᵀ · x` for an `m × n` matrix `A` (row-major). Returns a
/// fresh `Vec<f64>` of length `n`. Panics if `x.len() != m` or any
/// row width differs.
pub fn matvec_t(a: &[Vec<f64>], x: &[f64]) -> Vec<f64> {
    let m = a.len();
    assert_eq!(x.len(), m, "matvec_t: vector length must equal matrix rows");
    if m == 0 {
        return Vec::new();
    }
    let n = a[0].len();
    let mut y = vec![0.0; n];
    for i in 0..m {
        assert_eq!(a[i].len(), n, "matvec_t: row width mismatch");
        let xi = x[i];
        for j in 0..n {
            y[j] += a[i][j] * xi;
        }
    }
    y
}

/// `Aᵀ · A` for an `m × n` matrix `A`. Returns the `n × n` Gram
/// matrix as `Vec<Vec<f64>>` (row-major). The result is symmetric;
/// only the upper triangle is computed and the lower triangle is
/// mirrored to avoid redundant arithmetic.
///
/// LM's normal equations need this on every iteration: `(AᵀA + λI)
/// Δx = −Aᵀr`.
pub fn matmul_ata(a: &[Vec<f64>]) -> Vec<Vec<f64>> {
    let m = a.len();
    if m == 0 {
        return Vec::new();
    }
    let n = a[0].len();
    let mut gram = vec![vec![0.0; n]; n];
    for j in 0..n {
        for k in j..n {
            let mut sum = 0.0;
            for i in 0..m {
                debug_assert_eq!(a[i].len(), n, "matmul_ata: ragged matrix");
                sum += a[i][j] * a[i][k];
            }
            gram[j][k] = sum;
            if j != k {
                gram[k][j] = sum;
            }
        }
    }
    gram
}

/// In-place diagonal add: `A[i][i] += λ` for all `i`. Used by LM to
/// damp the normal-equation matrix.
pub fn add_diag(a: &mut [Vec<f64>], lambda: f64) {
    for i in 0..a.len() {
        a[i][i] += lambda;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPS: f64 = 1e-12;

    fn approx_eq(a: f64, b: f64, eps: f64) -> bool {
        (a - b).abs() <= eps + eps * b.abs()
    }

    // ─── 2D vector primitives ───

    #[test]
    fn sub_add_scale_compose() {
        let a = (3.0, 4.0);
        let b = (1.0, 1.0);
        assert_eq!(sub(a, b), (2.0, 3.0));
        assert_eq!(add(a, b), (4.0, 5.0));
        assert_eq!(scale(2.0, a), (6.0, 8.0));
    }

    #[test]
    fn dot_cross_basics() {
        let a = (1.0, 0.0);
        let b = (0.0, 1.0);
        assert!(approx_eq(dot(a, b), 0.0, EPS));
        assert!(approx_eq(cross(a, b), 1.0, EPS));
        assert!(approx_eq(cross(b, a), -1.0, EPS));
        assert!(approx_eq(dot(a, a), 1.0, EPS));
    }

    #[test]
    fn norm_distance_basics() {
        let a = (3.0, 4.0);
        let b = (0.0, 0.0);
        assert!(approx_eq(norm(a), 5.0, EPS));
        assert!(approx_eq(norm_sq_2(a), 25.0, EPS));
        assert!(approx_eq(distance(a, b), 5.0, EPS));
        assert!(approx_eq(distance(a, a), 0.0, EPS));
    }

    #[test]
    fn wrap_to_pi_basic_cases() {
        use std::f64::consts::PI;
        assert!(approx_eq(wrap_to_pi(0.0), 0.0, EPS));
        assert!(approx_eq(wrap_to_pi(PI), PI, EPS));
        assert!(approx_eq(wrap_to_pi(-PI), PI, EPS)); // half-open: -π lifts to +π
        assert!(approx_eq(wrap_to_pi(2.0 * PI), 0.0, EPS));
        assert!(approx_eq(wrap_to_pi(3.0 * PI), PI, EPS));
        // Branch-cut crossing: a 2π jump collapses to 0.
        let a = PI - 0.01;
        let b = -PI + 0.01;
        assert!(
            approx_eq(wrap_to_pi(b - a), -PI + 0.02 + 2.0 * PI - 0.0, 1e-3)
                || (wrap_to_pi(b - a)).abs() < 0.02 + EPS
        );
    }

    // ─── Dense vector / matrix primitives ───

    #[test]
    fn norm_sq_and_norm_vec() {
        let v = vec![3.0, 4.0];
        assert!(approx_eq(norm_sq(&v), 25.0, EPS));
        assert!(approx_eq(norm_vec(&v), 5.0, EPS));
        let empty: Vec<f64> = vec![];
        assert_eq!(norm_sq(&empty), 0.0);
    }

    #[test]
    fn axpy_in_place() {
        let mut y = vec![1.0, 2.0, 3.0];
        let x = vec![10.0, 20.0, 30.0];
        axpy(0.5, &x, &mut y);
        assert_eq!(y, vec![6.0, 12.0, 18.0]);
    }

    #[test]
    fn matvec_2x3() {
        // A = [[1,2,3],[4,5,6]]
        // x = [1,1,1] → A·x = [6, 15]
        let a = vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]];
        let x = vec![1.0, 1.0, 1.0];
        let y = matvec(&a, &x);
        assert_eq!(y, vec![6.0, 15.0]);
    }

    #[test]
    fn matvec_t_3x2() {
        // A^T · x where A = [[1,2,3],[4,5,6]] (2×3)
        // A^T is 3×2, A^T·x where x = [1, 1] → [1+4, 2+5, 3+6] = [5, 7, 9]
        let a = vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]];
        let x = vec![1.0, 1.0];
        let y = matvec_t(&a, &x);
        assert_eq!(y, vec![5.0, 7.0, 9.0]);
    }

    #[test]
    fn matmul_ata_2x2() {
        // A = [[1,0],[0,1],[1,1]] (3×2)
        // A^T·A = [[2,1],[1,2]]
        let a = vec![vec![1.0, 0.0], vec![0.0, 1.0], vec![1.0, 1.0]];
        let g = matmul_ata(&a);
        assert_eq!(g, vec![vec![2.0, 1.0], vec![1.0, 2.0]]);
    }

    #[test]
    fn matmul_ata_is_symmetric() {
        // A 4×3 with arbitrary values; result must be symmetric.
        let a = vec![
            vec![1.0, 2.0, 3.0],
            vec![4.0, 5.0, 6.0],
            vec![-1.0, 0.0, 2.0],
            vec![3.0, -2.0, 1.0],
        ];
        let g = matmul_ata(&a);
        for i in 0..3 {
            for j in 0..3 {
                assert!(approx_eq(g[i][j], g[j][i], EPS));
            }
        }
    }

    #[test]
    fn add_diag_basic() {
        let mut a = vec![vec![1.0, 2.0], vec![3.0, 4.0]];
        add_diag(&mut a, 0.5);
        assert_eq!(a, vec![vec![1.5, 2.0], vec![3.0, 4.5]]);
    }

    #[test]
    #[should_panic(expected = "axpy: length mismatch")]
    fn axpy_length_mismatch_panics() {
        let mut y = vec![1.0];
        let x = vec![1.0, 2.0];
        axpy(1.0, &x, &mut y);
    }
}
