//! Self-contained benchmark for the in-house dense LU solver in
//! `signex_sketch::solver::linalg`. No external benchmarking crate
//! is used — timings are taken with `std::time::Instant` so the
//! Apache-clean Signex codebase stays free of dev-dependencies for
//! micro-benchmarking.
//!
//! Run:
//!
//! ```sh
//! cargo run -p signex-sketch --example bench_linalg --release
//! ```
//!
//! What it measures
//! ----------------
//! For a small spread of system sizes (10, 25, 50, 100, 200 unknowns)
//! we time:
//!
//! 1. `lu_decompose` — the partial-pivot LU factorisation (the
//!    dominant cost; O(n³)).
//! 2. `lu_solve`     — forward + back substitution given an existing
//!    LU + permutation (O(n²)).
//! 3. `solve`        — end-to-end `solve(A, b)` (decompose + solve
//!    every call; the LM iteration's amortised cost per step).
//!
//! Each size is benchmarked over `iters` runs of the same problem
//! (regenerated once, reused) to amortise allocator and timer noise.
//! The reported number is the per-iteration mean.
//!
//! What we expect
//! --------------
//! For `n ≤ 200` unknowns (the v0.13 sketcher's worst-case constraint
//! count), end-to-end solve should be sub-millisecond on a 2024-class
//! laptop. The LM iteration's 50 ms timeout comfortably covers a full
//! Newton–Marquardt loop (50–100 iterations × sub-ms solve each).
//!
//! What we do NOT claim
//! --------------------
//! These numbers are not vs-state-of-the-art benchmarks. nalgebra and
//! faer (the Apache-2.0/MIT pure-Rust LA libraries) ship SIMD-tuned
//! dense LU that will be 2–5× faster on these sizes. We choose
//! roll-our-own to keep the Apache-clean Signex codebase
//! dependency-free; the bench exists to verify the performance is
//! adequate for the v0.13 use case, not to compete with hand-tuned
//! BLAS implementations.

use signex_sketch::solver::linalg::{lu_decompose, lu_solve, solve};
use std::time::Instant;

/// Build a well-conditioned `n × n` matrix that exercises pivoting:
/// strongly diagonally dominant with off-diagonals decaying away
/// from the diagonal. The system is solvable to machine precision.
fn make_matrix(n: usize) -> (Vec<Vec<f64>>, Vec<f64>) {
    let mut a = vec![vec![0.0; n]; n];
    let mut b = vec![0.0; n];
    for i in 0..n {
        for j in 0..n {
            a[i][j] = if i == j {
                4.0 + (i as f64) * 0.01
            } else {
                1.0 / (1.0 + ((i as f64) - (j as f64)).abs())
            };
        }
        b[i] = (i as f64) * 0.5 + 1.0;
    }
    (a, b)
}

/// Run one bench: time `f` over `iters` runs and return the mean
/// duration in nanoseconds.
fn bench<F: FnMut()>(iters: usize, mut f: F) -> f64 {
    // Single warm-up to prime caches.
    f();
    let start = Instant::now();
    for _ in 0..iters {
        f();
    }
    let elapsed = start.elapsed();
    elapsed.as_nanos() as f64 / iters as f64
}

fn main() {
    println!(
        "{:>4}  {:>12}  {:>12}  {:>12}  {:>14}",
        "n", "decompose ns", "lu_solve ns", "solve ns", "solve GFLOPS*"
    );
    println!("{}", "-".repeat(64));

    // Smaller iters at larger n so the bench finishes in seconds.
    let sizes = [(10, 5_000), (25, 2_000), (50, 1_000), (100, 200), (200, 50)];

    for (n, iters) in sizes {
        let (a, b) = make_matrix(n);

        // Warm allocator.
        let _ = solve(&a, &b).unwrap();

        // lu_decompose alone.
        let mut scratch = a.clone();
        let decompose_ns = bench(iters, || {
            for i in 0..n {
                scratch[i].copy_from_slice(&a[i]);
            }
            let _ = lu_decompose(&mut scratch).unwrap();
        });

        // lu_solve alone (one decomposition reused).
        let mut lu = a.clone();
        let perm = lu_decompose(&mut lu).unwrap();
        let lu_solve_ns = bench(iters, || {
            let _ = lu_solve(&lu, &perm, &b).unwrap();
        });

        // Full solve (decompose + back-sub each iter).
        let solve_ns = bench(iters, || {
            let _ = solve(&a, &b).unwrap();
        });

        // Approximate FLOPS for solve = (2/3)·n³ (decompose) + 2·n² (subs).
        let flops = (2.0 / 3.0) * (n as f64).powi(3) + 2.0 * (n as f64).powi(2);
        let gflops = flops / solve_ns;

        println!(
            "{:>4}  {:>12.0}  {:>12.0}  {:>12.0}  {:>14.3}",
            n, decompose_ns, lu_solve_ns, solve_ns, gflops
        );
    }

    println!();
    println!("* solve GFLOPS computed as ((2/3)·n³ + 2·n²) / solve_ns.");
    println!("  The constraint-solver's LM step solves one (J^T J + λI) Δx = b");
    println!("  system per iteration; n is the number of free state variables.");
    println!("  v0.13 footprints expect n ≤ ~80 (e.g. QFP-100 with 200 pads, half");
    println!("  of which are anchored by a Linear array source point) — so the");
    println!("  100-unknown row above is the realistic upper bound.");
}
