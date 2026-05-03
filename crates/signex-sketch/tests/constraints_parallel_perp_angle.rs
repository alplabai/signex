//! Task 2.4 — residual tests for Parallel, Perpendicular, Angle.

mod common;
use common::Sketch;

use std::f64::consts::PI;

use signex_sketch::constraint::{Constraint, ConstraintKind, DimTarget};
use signex_sketch::id::ConstraintId;
use signex_sketch::solver::residual::{residual, ResolvedParams};
use signex_sketch::solver::state::pack;

fn empty_params() -> ResolvedParams {
    ResolvedParams::new()
}

// ─────────────────────────── Parallel ───────────────────────────

#[test]
fn parallel_residual_zero_on_two_horizontal_lines() {
    let mut s = Sketch::new();
    let a1 = s.add_point(0.0, 0.0);
    let a2 = s.add_point(5.0, 0.0);
    let l1 = s.add_line(a1, a2);
    let b1 = s.add_point(1.0, 3.0);
    let b2 = s.add_point(7.0, 3.0);
    let l2 = s.add_line(b1, b2);
    let packed = pack(&s.data);

    let c = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Parallel { l1, l2 },
    };
    let r = residual(&c, &packed.vector, &packed.index, &s.data, &empty_params()).unwrap();
    assert_eq!(r.len(), 1);
    assert!(r[0].abs() < 1e-12, "expected ~0, got {}", r[0]);
}

#[test]
fn parallel_residual_zero_on_two_lines_at_30_degrees() {
    let mut s = Sketch::new();
    let theta = (30.0_f64).to_radians();
    let (cx, cy) = (theta.cos(), theta.sin());

    // line 1: from (0,0) toward angle 30°, length 4
    let a1 = s.add_point(0.0, 0.0);
    let a2 = s.add_point(4.0 * cx, 4.0 * cy);
    let l1 = s.add_line(a1, a2);

    // line 2: from (1,2) toward same angle, length 7 (different length, same direction)
    let b1 = s.add_point(1.0, 2.0);
    let b2 = s.add_point(1.0 + 7.0 * cx, 2.0 + 7.0 * cy);
    let l2 = s.add_line(b1, b2);

    let packed = pack(&s.data);
    let c = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Parallel { l1, l2 },
    };
    let r = residual(&c, &packed.vector, &packed.index, &s.data, &empty_params()).unwrap();
    assert!(r[0].abs() < 1e-10, "expected ~0, got {}", r[0]);
}

#[test]
fn parallel_residual_zero_on_antiparallel_lines() {
    // d1 = +x, d2 = −x ⇒ cross = 0 ⇒ residual zero (antiparallel counts).
    let mut s = Sketch::new();
    let a1 = s.add_point(0.0, 0.0);
    let a2 = s.add_point(5.0, 0.0);
    let l1 = s.add_line(a1, a2);
    let b1 = s.add_point(10.0, 1.0);
    let b2 = s.add_point(2.0, 1.0);
    let l2 = s.add_line(b1, b2);
    let packed = pack(&s.data);

    let c = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Parallel { l1, l2 },
    };
    let r = residual(&c, &packed.vector, &packed.index, &s.data, &empty_params()).unwrap();
    assert!(r[0].abs() < 1e-12, "expected ~0, got {}", r[0]);
}

#[test]
fn parallel_residual_nonzero_on_perpendicular_lines() {
    let mut s = Sketch::new();
    // horizontal
    let a1 = s.add_point(0.0, 0.0);
    let a2 = s.add_point(3.0, 0.0);
    let l1 = s.add_line(a1, a2);
    // vertical
    let b1 = s.add_point(0.0, 0.0);
    let b2 = s.add_point(0.0, 4.0);
    let l2 = s.add_line(b1, b2);
    let packed = pack(&s.data);

    let c = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Parallel { l1, l2 },
    };
    let r = residual(&c, &packed.vector, &packed.index, &s.data, &empty_params()).unwrap();
    // cross = 3·4 − 0·0 = 12
    assert!((r[0] - 12.0).abs() < 1e-12, "expected 12, got {}", r[0]);
}

// ───────────────────────── Perpendicular ─────────────────────────

#[test]
fn perpendicular_residual_zero_on_horizontal_and_vertical() {
    let mut s = Sketch::new();
    let a1 = s.add_point(0.0, 0.0);
    let a2 = s.add_point(3.0, 0.0);
    let l1 = s.add_line(a1, a2);
    let b1 = s.add_point(2.0, 1.0);
    let b2 = s.add_point(2.0, 6.0);
    let l2 = s.add_line(b1, b2);
    let packed = pack(&s.data);

    let c = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Perpendicular { l1, l2 },
    };
    let r = residual(&c, &packed.vector, &packed.index, &s.data, &empty_params()).unwrap();
    assert_eq!(r.len(), 1);
    assert!(r[0].abs() < 1e-12, "expected ~0, got {}", r[0]);
}

#[test]
fn perpendicular_residual_zero_on_45_and_135_degree_lines() {
    let mut s = Sketch::new();
    // 45° line: (0,0) → (1,1)
    let a1 = s.add_point(0.0, 0.0);
    let a2 = s.add_point(1.0, 1.0);
    let l1 = s.add_line(a1, a2);
    // 135° line: (0,0) → (−1,1) — perpendicular to 45°
    let b1 = s.add_point(0.0, 0.0);
    let b2 = s.add_point(-1.0, 1.0);
    let l2 = s.add_line(b1, b2);
    let packed = pack(&s.data);

    let c = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Perpendicular { l1, l2 },
    };
    let r = residual(&c, &packed.vector, &packed.index, &s.data, &empty_params()).unwrap();
    assert!(r[0].abs() < 1e-12, "expected ~0, got {}", r[0]);
}

#[test]
fn perpendicular_residual_nonzero_on_parallel_lines() {
    let mut s = Sketch::new();
    let a1 = s.add_point(0.0, 0.0);
    let a2 = s.add_point(3.0, 0.0);
    let l1 = s.add_line(a1, a2);
    let b1 = s.add_point(0.0, 5.0);
    let b2 = s.add_point(4.0, 5.0);
    let l2 = s.add_line(b1, b2);
    let packed = pack(&s.data);

    let c = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Perpendicular { l1, l2 },
    };
    let r = residual(&c, &packed.vector, &packed.index, &s.data, &empty_params()).unwrap();
    // dot = 3·4 + 0·0 = 12
    assert!((r[0] - 12.0).abs() < 1e-12, "expected 12, got {}", r[0]);
}

// ─────────────────────────── Angle ───────────────────────────

#[test]
fn angle_residual_zero_when_target_matches_geometry() {
    // l1 along +x; l2 at 60° ⇒ measured = +60° rad.
    let mut s = Sketch::new();
    let a1 = s.add_point(0.0, 0.0);
    let a2 = s.add_point(2.0, 0.0);
    let l1 = s.add_line(a1, a2);
    let theta = (60.0_f64).to_radians();
    let b1 = s.add_point(0.0, 0.0);
    let b2 = s.add_point(theta.cos(), theta.sin());
    let l2 = s.add_line(b1, b2);
    let packed = pack(&s.data);

    let c = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Angle {
            l1,
            l2,
            target: DimTarget::Literal(theta),
        },
    };
    let r = residual(&c, &packed.vector, &packed.index, &s.data, &empty_params()).unwrap();
    assert_eq!(r.len(), 1);
    assert!(r[0].abs() < 1e-12, "expected ~0, got {}", r[0]);
}

#[test]
fn angle_residual_handles_target_pi_over_2() {
    let mut s = Sketch::new();
    // l1 along +x
    let a1 = s.add_point(0.0, 0.0);
    let a2 = s.add_point(3.0, 0.0);
    let l1 = s.add_line(a1, a2);
    // l2 along +y (rotation of +π/2 from +x)
    let b1 = s.add_point(2.0, 1.0);
    let b2 = s.add_point(2.0, 5.0);
    let l2 = s.add_line(b1, b2);
    let packed = pack(&s.data);

    let c = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Angle {
            l1,
            l2,
            target: DimTarget::Literal(PI / 2.0),
        },
    };
    let r = residual(&c, &packed.vector, &packed.index, &s.data, &empty_params()).unwrap();
    assert!(r[0].abs() < 1e-12, "expected ~0, got {}", r[0]);
}

#[test]
fn angle_residual_zero_with_target_zero_matches_parallel_case() {
    // Two horizontal lines ⇒ measured angle = 0 ⇒ residual = 0 with target 0.
    // Mirrors the Parallel "two horizontal" test, confirming target=0
    // and Parallel agree on parallelism.
    let mut s = Sketch::new();
    let a1 = s.add_point(0.0, 0.0);
    let a2 = s.add_point(5.0, 0.0);
    let l1 = s.add_line(a1, a2);
    let b1 = s.add_point(1.0, 3.0);
    let b2 = s.add_point(7.0, 3.0);
    let l2 = s.add_line(b1, b2);
    let packed = pack(&s.data);

    let c = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Angle {
            l1,
            l2,
            target: DimTarget::Literal(0.0),
        },
    };
    let r = residual(&c, &packed.vector, &packed.index, &s.data, &empty_params()).unwrap();
    assert!(r[0].abs() < 1e-12, "expected ~0, got {}", r[0]);
}

#[test]
fn angle_residual_signed_negative_for_clockwise_rotation() {
    // l1 along +x, l2 at −45° ⇒ measured = −π/4. With target 0 the
    // residual should be −π/4 (negative).
    let mut s = Sketch::new();
    let a1 = s.add_point(0.0, 0.0);
    let a2 = s.add_point(3.0, 0.0);
    let l1 = s.add_line(a1, a2);
    let b1 = s.add_point(0.0, 0.0);
    let b2 = s.add_point(1.0, -1.0);
    let l2 = s.add_line(b1, b2);
    let packed = pack(&s.data);

    let c = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Angle {
            l1,
            l2,
            target: DimTarget::Literal(0.0),
        },
    };
    let r = residual(&c, &packed.vector, &packed.index, &s.data, &empty_params()).unwrap();
    assert!(
        (r[0] - (-PI / 4.0)).abs() < 1e-12,
        "expected −π/4, got {}",
        r[0]
    );
}

#[test]
fn angle_residual_wraps_across_pi_branch_cut() {
    // l1 along +x; l2 at +179° (just below π). Target = −179° (just
    // above −π). Measured − target = +358° which wraps into −2°. The
    // residual must NOT be near 358° (otherwise LM would jump
    // discontinuously). |residual| should be ≤ π.
    let mut s = Sketch::new();
    let a1 = s.add_point(0.0, 0.0);
    let a2 = s.add_point(2.0, 0.0);
    let l1 = s.add_line(a1, a2);

    let phi = (179.0_f64).to_radians();
    let b1 = s.add_point(0.0, 0.0);
    let b2 = s.add_point(phi.cos(), phi.sin());
    let l2 = s.add_line(b1, b2);

    let target = (-179.0_f64).to_radians();
    let packed = pack(&s.data);
    let c = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Angle {
            l1,
            l2,
            target: DimTarget::Literal(target),
        },
    };
    let r = residual(&c, &packed.vector, &packed.index, &s.data, &empty_params()).unwrap();

    // |residual| must lie in (−π, π].
    assert!(
        r[0].abs() <= PI + 1e-12,
        "residual {} not wrapped into (−π, π]",
        r[0]
    );
    // Wrapped value should be ≈ −2° = −0.0349 rad (358° wrapped to −2°).
    let expected = (-2.0_f64).to_radians();
    assert!(
        (r[0] - expected).abs() < 1e-9,
        "expected ~−2°, got {} rad",
        r[0]
    );
}

// ───────────────── residual_count() sanity check ─────────────────

#[test]
fn residual_count_matches_returned_vector_length() {
    let mut s = Sketch::new();
    let a = s.add_point(0.0, 0.0);
    let b = s.add_point(1.0, 0.0);
    let l1 = s.add_line(a, b);
    let c = s.add_point(0.0, 0.0);
    let d = s.add_point(0.0, 1.0);
    let l2 = s.add_line(c, d);
    let packed = pack(&s.data);

    let cases: Vec<ConstraintKind> = vec![
        ConstraintKind::Parallel { l1, l2 },
        ConstraintKind::Perpendicular { l1, l2 },
        ConstraintKind::Angle {
            l1,
            l2,
            target: DimTarget::Literal(PI / 2.0),
        },
    ];

    for kind in cases {
        let c = Constraint {
            id: ConstraintId::new(),
            kind: kind.clone(),
        };
        let r = residual(&c, &packed.vector, &packed.index, &s.data, &empty_params()).unwrap();
        assert_eq!(
            r.len(),
            kind.residual_count(),
            "residual_count mismatch for {kind:?}"
        );
        assert_eq!(r.len(), 1, "all three kinds expected to produce 1 scalar");
    }
}
