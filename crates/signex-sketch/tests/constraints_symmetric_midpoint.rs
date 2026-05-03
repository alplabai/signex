//! Task 2.7 — residual tests for SymmetricAboutLine,
//! SymmetricAboutPoint, and Midpoint.

mod common;
use common::Sketch;

use signex_sketch::constraint::{Constraint, ConstraintKind};
use signex_sketch::id::ConstraintId;
use signex_sketch::solver::residual::{residual, ResolvedParams};
use signex_sketch::solver::state::pack;

fn empty_params() -> ResolvedParams {
    ResolvedParams::new()
}

// ─── SymmetricAboutLine ──────────────────────────────────────────────

#[test]
fn symmetric_about_line_zero_mirrored_across_x_axis() {
    // Line from (0,0) to (1,0) is the X axis.
    // p1=(2,3), p2=(2,-3) are mirror images across it.
    let mut s = Sketch::new();
    let a = s.add_point(0.0, 0.0);
    let b = s.add_point(1.0, 0.0);
    let line = s.add_line(a, b);
    let p1 = s.add_point(2.0, 3.0);
    let p2 = s.add_point(2.0, -3.0);
    let packed = pack(&s.data);

    let c = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::SymmetricAboutLine { p1, p2, line },
    };
    let r = residual(&c, &packed.vector, &packed.index, &s.data, &empty_params()).unwrap();
    assert_eq!(r.len(), 2);
    assert!(r[0].abs() < 1e-12, "midpoint-on-line residual nonzero: {}", r[0]);
    assert!(r[1].abs() < 1e-12, "perp-to-line residual nonzero: {}", r[1]);
}

#[test]
fn symmetric_about_line_nonzero_when_midpoint_off_line() {
    // Line is X-axis. p1=(2,3), p2=(2,-1). Midpoint (2,1) is NOT on
    // X-axis, but p2 − p1 = (0,−4) IS perpendicular to (1,0).
    // First residual should be nonzero, second should be zero.
    let mut s = Sketch::new();
    let a = s.add_point(0.0, 0.0);
    let b = s.add_point(1.0, 0.0);
    let line = s.add_line(a, b);
    let p1 = s.add_point(2.0, 3.0);
    let p2 = s.add_point(2.0, -1.0);
    let packed = pack(&s.data);

    let c = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::SymmetricAboutLine { p1, p2, line },
    };
    let r = residual(&c, &packed.vector, &packed.index, &s.data, &empty_params()).unwrap();
    // signed perpendicular distance: cross / |d| = (2*0 − 1*1) / 1 = −1
    assert!((r[0] - (-1.0)).abs() < 1e-12, "got r[0] = {}", r[0]);
    assert!(r[1].abs() < 1e-12, "got r[1] = {}", r[1]);
}

#[test]
fn symmetric_about_line_nonzero_when_segment_not_perpendicular() {
    // Line is X-axis. p1=(0,1), p2=(2,-1). Midpoint (1,0) IS on the
    // X-axis, but p2 − p1 = (2,−2) is NOT perpendicular to (1,0).
    // First residual should be zero, second should be 2.
    let mut s = Sketch::new();
    let a = s.add_point(0.0, 0.0);
    let b = s.add_point(1.0, 0.0);
    let line = s.add_line(a, b);
    let p1 = s.add_point(0.0, 1.0);
    let p2 = s.add_point(2.0, -1.0);
    let packed = pack(&s.data);

    let c = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::SymmetricAboutLine { p1, p2, line },
    };
    let r = residual(&c, &packed.vector, &packed.index, &s.data, &empty_params()).unwrap();
    assert!(r[0].abs() < 1e-12, "got r[0] = {}", r[0]);
    // dot = (2 − 0) * 1 + (−1 − 1) * 0 = 2
    assert!((r[1] - 2.0).abs() < 1e-12, "got r[1] = {}", r[1]);
}

#[test]
fn symmetric_about_line_zero_for_45_degree_line() {
    // Line from (0,0) to (1,1), p1=(0,1), p2=(1,0) — mirror images
    // across the y = x line.
    let mut s = Sketch::new();
    let a = s.add_point(0.0, 0.0);
    let b = s.add_point(1.0, 1.0);
    let line = s.add_line(a, b);
    let p1 = s.add_point(0.0, 1.0);
    let p2 = s.add_point(1.0, 0.0);
    let packed = pack(&s.data);

    let c = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::SymmetricAboutLine { p1, p2, line },
    };
    let r = residual(&c, &packed.vector, &packed.index, &s.data, &empty_params()).unwrap();
    assert_eq!(r.len(), 2);
    assert!(r[0].abs() < 1e-12, "midpoint-on-line: got {}", r[0]);
    assert!(r[1].abs() < 1e-12, "perp-to-line: got {}", r[1]);
}

// ─── SymmetricAboutPoint ─────────────────────────────────────────────

#[test]
fn symmetric_about_point_zero_when_centre_is_midpoint() {
    // p1=(1,2), p2=(5,8) — midpoint is (3,5); centre at (3,5).
    let mut s = Sketch::new();
    let p1 = s.add_point(1.0, 2.0);
    let p2 = s.add_point(5.0, 8.0);
    let center = s.add_point(3.0, 5.0);
    let packed = pack(&s.data);

    let c = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::SymmetricAboutPoint { p1, p2, center },
    };
    let r = residual(&c, &packed.vector, &packed.index, &s.data, &empty_params()).unwrap();
    assert_eq!(r.len(), 2);
    assert!(r[0].abs() < 1e-12, "got r[0] = {}", r[0]);
    assert!(r[1].abs() < 1e-12, "got r[1] = {}", r[1]);
}

#[test]
fn symmetric_about_point_nonzero_when_centre_off() {
    // p1=(1,2), p2=(5,8) — true midpoint is (3,5); place centre at
    // (2,4). Residual = ((1+5)/2 − 2, (2+8)/2 − 4) = (1, 1).
    let mut s = Sketch::new();
    let p1 = s.add_point(1.0, 2.0);
    let p2 = s.add_point(5.0, 8.0);
    let center = s.add_point(2.0, 4.0);
    let packed = pack(&s.data);

    let c = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::SymmetricAboutPoint { p1, p2, center },
    };
    let r = residual(&c, &packed.vector, &packed.index, &s.data, &empty_params()).unwrap();
    assert!((r[0] - 1.0).abs() < 1e-12, "got r[0] = {}", r[0]);
    assert!((r[1] - 1.0).abs() < 1e-12, "got r[1] = {}", r[1]);
}

// ─── Midpoint ────────────────────────────────────────────────────────

#[test]
fn midpoint_zero_when_point_at_line_midpoint() {
    // Line A=(0,0), B=(4,6). Midpoint is (2,3). Point at (2,3).
    let mut s = Sketch::new();
    let a = s.add_point(0.0, 0.0);
    let b = s.add_point(4.0, 6.0);
    let line = s.add_line(a, b);
    let point = s.add_point(2.0, 3.0);
    let packed = pack(&s.data);

    let c = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Midpoint { point, line },
    };
    let r = residual(&c, &packed.vector, &packed.index, &s.data, &empty_params()).unwrap();
    assert_eq!(r.len(), 2);
    assert!(r[0].abs() < 1e-12, "got r[0] = {}", r[0]);
    assert!(r[1].abs() < 1e-12, "got r[1] = {}", r[1]);
}

#[test]
fn midpoint_nonzero_when_point_at_endpoint() {
    // Line A=(0,0), B=(4,6). Midpoint is (2,3). Point at A=(0,0).
    // Residual = (0 − 2, 0 − 3) = (−2, −3).
    let mut s = Sketch::new();
    let a = s.add_point(0.0, 0.0);
    let b = s.add_point(4.0, 6.0);
    let line = s.add_line(a, b);
    let point = s.add_point(0.0, 0.0);
    let packed = pack(&s.data);

    let c = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Midpoint { point, line },
    };
    let r = residual(&c, &packed.vector, &packed.index, &s.data, &empty_params()).unwrap();
    assert!((r[0] - (-2.0)).abs() < 1e-12, "got r[0] = {}", r[0]);
    assert!((r[1] - (-3.0)).abs() < 1e-12, "got r[1] = {}", r[1]);
}

// ─── Sanity: residual_count() matches output length ─────────────────

#[test]
fn residual_count_matches_returned_vector_length() {
    let mut s = Sketch::new();
    let a = s.add_point(0.0, 0.0);
    let b = s.add_point(1.0, 0.0);
    let line = s.add_line(a, b);
    let p1 = s.add_point(2.0, 3.0);
    let p2 = s.add_point(2.0, -3.0);
    let center = s.add_point(2.0, 0.0);
    let on_mid = s.add_point(0.5, 0.0);
    let packed = pack(&s.data);

    let cases: Vec<ConstraintKind> = vec![
        ConstraintKind::SymmetricAboutLine { p1, p2, line },
        ConstraintKind::SymmetricAboutPoint { p1, p2, center },
        ConstraintKind::Midpoint { point: on_mid, line },
    ];

    for kind in cases {
        // residual_count() should match the residual vector length
        // for every Task 2.7 kind.
        assert_eq!(kind.residual_count(), 2, "count mismatch for {kind:?}");
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
    }
}
