//! Integration tests for the dense LU linear solver.
//!
//! Reference for hand-worked solutions: standard 3×3 systems
//! (Numerical Recipes §2.3 worked examples). All "expected" values
//! were verified by hand or by direct substitution.

use signex_sketch::solver::linalg::{LinAlgError, QrDecomposition, lu_decompose, lu_solve, solve};

const TOL: f64 = 1e-10;

/// Check that two `f64` vectors agree element-wise within `TOL`.
fn assert_vec_close(actual: &[f64], expected: &[f64]) {
    assert_eq!(
        actual.len(),
        expected.len(),
        "vector length mismatch: got {}, expected {}",
        actual.len(),
        expected.len()
    );
    for (i, (a, e)) in actual.iter().zip(expected.iter()).enumerate() {
        let diff = (a - e).abs();
        assert!(
            diff < TOL,
            "index {}: got {}, expected {} (diff {})",
            i,
            a,
            e,
            diff
        );
    }
}

/// Multiply a square matrix by a vector — used to round-trip test
/// `solve(A, b)` by checking `A x ≈ b`.
fn mat_vec(a: &[Vec<f64>], x: &[f64]) -> Vec<f64> {
    let n = a.len();
    assert_eq!(x.len(), n);
    let mut y = vec![0.0; n];
    for (i, row) in a.iter().enumerate() {
        assert_eq!(row.len(), n);
        for (j, &aij) in row.iter().enumerate() {
            y[i] += aij * x[j];
        }
    }
    y
}

// ─── 1. 2×2 trivial ─────────────────────────────────────────────

#[test]
fn solve_2x2_trivial() {
    // [[2, 1], [1, 3]] x = [4, 5]
    // Determinant = 2·3 − 1·1 = 5
    // Cramer: x1 = (4·3 − 1·5)/5 = 7/5 = 1.4
    //          x2 = (2·5 − 4·1)/5 = 6/5 = 1.2
    let a = vec![vec![2.0, 1.0], vec![1.0, 3.0]];
    let b = vec![4.0, 5.0];

    let x = solve(&a, &b).expect("non-singular 2×2 solves");
    assert_vec_close(&x, &[1.4, 1.2]);
}

// ─── 2. 3×3 well-conditioned ────────────────────────────────────

#[test]
fn solve_3x3_hand_computed() {
    // The system is constructed so that x = [1, 1, 1].
    //   row 0: 1 + 2 + 3 = 6
    //   row 1: 4 + 5 + 6 = 15
    //   row 2: 7 + 8 + 10 = 25
    // det([[1,2,3],[4,5,6],[7,8,10]]) = -3 ≠ 0, so the system is
    // non-singular and the solution is unique.
    let a = vec![
        vec![1.0, 2.0, 3.0],
        vec![4.0, 5.0, 6.0],
        vec![7.0, 8.0, 10.0],
    ];
    let b = vec![6.0, 15.0, 25.0];

    let x = solve(&a, &b).expect("non-singular 3×3 solves");
    assert_vec_close(&x, &[1.0, 1.0, 1.0]);
}

// ─── 3. Pivoting required ───────────────────────────────────────

#[test]
fn solve_2x2_zero_diagonal_forces_pivot() {
    // [[0, 1], [1, 0]] x = [1, 2]
    // The (0,0) entry is exactly zero so naive Gaussian elimination
    // would divide by zero. Partial pivoting must swap rows 0 and 1.
    // After the swap: [[1, 0], [0, 1]] x = [2, 1] ⇒ x = [2, 1].
    let a = vec![vec![0.0, 1.0], vec![1.0, 0.0]];
    let b = vec![1.0, 2.0];

    let x = solve(&a, &b).expect("pivot rescues the singular-looking diagonal");
    assert_vec_close(&x, &[2.0, 1.0]);
}

#[test]
fn solve_3x3_pivoting_required() {
    // Tiny (0,0) so the largest column-0 magnitude lives in row 1.
    // Full system is non-singular; pivoting must promote row 1 to
    // pivot duty in column 0.
    //   [[1e-15, 1, 0], [1, 1, 1], [0, 1, 2]] x = b
    // Pick x = [1, 2, 3]:
    //   row 0: 1e-15·1 + 1·2 + 0·3 = 2 + 1e-15
    //   row 1: 1·1 + 1·2 + 1·3 = 6
    //   row 2: 0·1 + 1·2 + 2·3 = 8
    let a = vec![
        vec![1e-15, 1.0, 0.0],
        vec![1.0, 1.0, 1.0],
        vec![0.0, 1.0, 2.0],
    ];
    let b = vec![2.0 + 1e-15, 6.0, 8.0];

    let x = solve(&a, &b).expect("pivoting handles tiny diagonal");
    assert_vec_close(&x, &[1.0, 2.0, 3.0]);
}

// ─── 4. Identity matrix ─────────────────────────────────────────

#[test]
fn solve_identity_returns_b() {
    let a = vec![
        vec![1.0, 0.0, 0.0, 0.0],
        vec![0.0, 1.0, 0.0, 0.0],
        vec![0.0, 0.0, 1.0, 0.0],
        vec![0.0, 0.0, 0.0, 1.0],
    ];
    let b = vec![1.5, -2.5, 0.0, 3.14159];

    let x = solve(&a, &b).expect("identity is non-singular");
    assert_vec_close(&x, &b);
}

// ─── 5. Singular matrix detected ────────────────────────────────

#[test]
fn solve_rank_deficient_2x2_is_singular() {
    // Row 1 is exactly 2× row 0 ⇒ det = 0, rank 1.
    let a = vec![vec![1.0, 2.0], vec![2.0, 4.0]];
    let b = vec![1.0, 2.0];

    let err = solve(&a, &b).expect_err("rank-deficient matrix should error");
    match err {
        LinAlgError::Singular => {}
        LinAlgError::DimensionMismatch => {
            panic!("expected Singular, got DimensionMismatch")
        }
    }
}

#[test]
fn solve_zero_matrix_is_singular() {
    // The zero matrix is the canonical singular case.
    let a = vec![vec![0.0, 0.0], vec![0.0, 0.0]];
    let b = vec![1.0, 1.0];

    let err = solve(&a, &b).expect_err("zero matrix is singular");
    assert!(matches!(err, LinAlgError::Singular));
}

// ─── 6. Dimension mismatch ──────────────────────────────────────

#[test]
fn solve_non_square_matrix_errors() {
    // 1×2 matrix is non-square.
    let a = vec![vec![1.0, 2.0]];
    let b = vec![1.0, 2.0];

    let err = solve(&a, &b).expect_err("non-square matrix must error");
    assert!(matches!(err, LinAlgError::DimensionMismatch));
}

#[test]
fn solve_b_length_mismatch_errors() {
    let a = vec![vec![1.0, 0.0], vec![0.0, 1.0]];
    let b = vec![1.0, 2.0, 3.0]; // length 3 against a 2×2

    let err = solve(&a, &b).expect_err("b length mismatch must error");
    assert!(matches!(err, LinAlgError::DimensionMismatch));
}

#[test]
fn solve_ragged_matrix_errors() {
    // Outer length 2 but row 1 has 3 columns ⇒ non-square.
    let a = vec![vec![1.0, 2.0], vec![3.0, 4.0, 5.0]];
    let b = vec![1.0, 2.0];

    let err = solve(&a, &b).expect_err("ragged rows must error");
    assert!(matches!(err, LinAlgError::DimensionMismatch));
}

// ─── 7. Round-trip: A·solve(A,b) reproduces b ───────────────────

#[test]
fn round_trip_3x3_case_1() {
    let a = vec![
        vec![4.0, 3.0, -1.0],
        vec![2.0, -1.0, 5.0],
        vec![1.0, 6.0, 2.0],
    ];
    let b = vec![6.0, 8.0, 13.0];

    let x = solve(&a, &b).expect("non-singular");
    let b_check = mat_vec(&a, &x);
    assert_vec_close(&b_check, &b);
}

#[test]
fn round_trip_3x3_case_2_negative_entries() {
    let a = vec![
        vec![-1.5, 2.0, 0.5],
        vec![3.0, -2.5, 1.0],
        vec![0.25, 1.0, -3.0],
    ];
    let b = vec![1.0, -2.0, 0.5];

    let x = solve(&a, &b).expect("non-singular");
    let b_check = mat_vec(&a, &x);
    assert_vec_close(&b_check, &b);
}

#[test]
fn round_trip_5x5_diagonally_dominant() {
    // Diagonally dominant ⇒ guaranteed non-singular and well-
    // conditioned. Constructed so the trivial answer is x =
    // [1, 2, 3, 4, 5]; b is computed by direct multiplication.
    let a = vec![
        vec![10.0, 1.0, 0.0, 2.0, 1.0],
        vec![1.0, 12.0, 1.0, 0.0, 3.0],
        vec![0.0, 1.0, 11.0, 2.0, 0.0],
        vec![2.0, 0.0, 2.0, 14.0, 1.0],
        vec![1.0, 3.0, 0.0, 1.0, 13.0],
    ];
    let x_expected = vec![1.0, 2.0, 3.0, 4.0, 5.0];
    let b = mat_vec(&a, &x_expected);

    let x = solve(&a, &b).expect("diagonally dominant is non-singular");
    assert_vec_close(&x, &x_expected);

    // Round-trip: A·x ≈ b within tolerance.
    let b_check = mat_vec(&a, &x);
    assert_vec_close(&b_check, &b);
}

// ─── 8. Direct lu_decompose/lu_solve ─────────────────────────────

#[test]
fn lu_decompose_and_solve_separately() {
    // Same well-conditioned 3×3 used above; this exercises the
    // public split between lu_decompose and lu_solve so callers
    // can re-use a factorisation across multiple right-hand sides.
    let a = vec![
        vec![1.0, 2.0, 3.0],
        vec![4.0, 5.0, 6.0],
        vec![7.0, 8.0, 10.0],
    ];

    let mut lu = a.clone();
    let perm = lu_decompose(&mut lu).expect("non-singular");

    let b1 = vec![6.0, 15.0, 25.0]; // x = [1, 1, 1]
    let x1 = lu_solve(&lu, &perm, &b1).expect("first RHS solves");
    assert_vec_close(&x1, &[1.0, 1.0, 1.0]);

    let b2 = vec![1.0, 0.0, 0.0]; // first column of A^{-1}
    let x2 = lu_solve(&lu, &perm, &b2).expect("second RHS solves");
    let b2_check = mat_vec(&a, &x2);
    assert_vec_close(&b2_check, &b2);
}

#[test]
fn lu_decompose_singular_matrix_errors() {
    let mut a = vec![vec![1.0, 2.0], vec![2.0, 4.0]];
    let err = lu_decompose(&mut a).expect_err("rank-deficient");
    assert!(matches!(err, LinAlgError::Singular));
}

#[test]
fn lu_solve_dimension_mismatch_errors() {
    let lu = vec![vec![1.0, 0.0], vec![0.0, 1.0]];
    let perm = vec![0, 1];
    let b = vec![1.0, 2.0, 3.0]; // wrong length
    let err = lu_solve(&lu, &perm, &b).expect_err("b length mismatch");
    assert!(matches!(err, LinAlgError::DimensionMismatch));
}

// ─── 9. 1×1 trivial / edge case ─────────────────────────────────

#[test]
fn solve_1x1_scalar() {
    let a = vec![vec![3.0]];
    let b = vec![6.0];
    let x = solve(&a, &b).expect("1×1 non-singular");
    assert_vec_close(&x, &[2.0]);
}

#[test]
fn solve_1x1_zero_is_singular() {
    let a = vec![vec![0.0]];
    let b = vec![1.0];
    let err = solve(&a, &b).expect_err("1×1 zero is singular");
    assert!(matches!(err, LinAlgError::Singular));
}

// ─── 10. Householder QR rank tests ──────────────────────────────────
//
// These cover the QR-based rank computation used by the DOF analysis
// in `solver::dof`. The rank of A equals the rank of R (since Q is
// orthogonal) so we only need to count the meaningful diagonal of R.
// `RANK_TOL` for these unit cases is generous — `1e-9` provides
// comfortable separation between "active" and "numerically zero"
// singular values for the textbook integer-coefficient matrices we
// use here.

const QR_TOL: f64 = 1e-9;

#[test]
fn qr_rank_full_3x3() {
    // The 3×3 identity is the canonical full-rank matrix: every
    // diagonal of R is exactly 1 (or its sign flipped by Householder),
    // so rank = 3.
    let a = vec![
        vec![1.0, 0.0, 0.0],
        vec![0.0, 1.0, 0.0],
        vec![0.0, 0.0, 1.0],
    ];

    let qr = QrDecomposition::new(&a).expect("identity QR factors cleanly");
    assert_eq!(qr.rank(QR_TOL), 3);
}

#[test]
fn qr_rank_full_4x2() {
    // 4×2 matrix with two orthogonal non-zero columns; the rest of
    // the column tail is zero. Rank should be exactly 2 (≤ min(m,n)).
    let a = vec![
        vec![1.0, 0.0],
        vec![0.0, 1.0],
        vec![0.0, 0.0],
        vec![0.0, 0.0],
    ];

    let qr = QrDecomposition::new(&a).expect("4×2 with two non-zero columns");
    assert_eq!(qr.rank(QR_TOL), 2);
}

#[test]
fn qr_rank_deficient() {
    // Row 1 is exactly 2× row 0 ⇒ rank 1 (one independent column,
    // one independent row). After Householder, R[1][1] should be
    // numerically zero so the rank count stops at 1.
    let a = vec![vec![1.0, 2.0], vec![2.0, 4.0]];

    let qr = QrDecomposition::new(&a).expect("rank-1 input still factors");
    assert_eq!(qr.rank(QR_TOL), 1);
}

#[test]
fn qr_rank_zero_matrix() {
    // The all-zero matrix has rank 0. No Householder reflection runs
    // because every sub-vector has norm zero; R is also all zeros.
    let a = vec![
        vec![0.0, 0.0, 0.0],
        vec![0.0, 0.0, 0.0],
        vec![0.0, 0.0, 0.0],
    ];

    let qr = QrDecomposition::new(&a).expect("zero matrix factors trivially");
    assert_eq!(qr.rank(QR_TOL), 0);
}
