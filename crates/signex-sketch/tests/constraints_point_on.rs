//! Task 2.5 — residual tests for PointOnLine, PointOnArc, DistancePtLine.

mod common;
use common::Sketch;

use signex_sketch::constraint::{Constraint, ConstraintKind, DimTarget};
use signex_sketch::id::ConstraintId;
use signex_sketch::solver::residual::{ResolvedParams, residual};
use signex_sketch::solver::state::pack;

const EPS: f64 = 1e-12;

fn empty_params() -> ResolvedParams {
    ResolvedParams::new()
}

// ─── PointOnLine ────────────────────────────────────────────────────

#[test]
fn point_on_line_residual_zero_when_on_line() {
    // Line A=(0,0) → B=(4,0); P=(2,0) sits on the line.
    let mut s = Sketch::new();
    let a = s.add_point(0.0, 0.0);
    let b = s.add_point(4.0, 0.0);
    let line = s.add_line(a, b);
    let p = s.add_point(2.0, 0.0);
    let packed = pack(&s.data);

    let c = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::PointOnLine { point: p, line },
    };
    let r = residual(&c, &packed.vector, &packed.index, &s.data, &empty_params()).unwrap();
    assert_eq!(r.len(), 1);
    assert!(r[0].abs() < EPS, "expected 0, got {}", r[0]);
}

#[test]
fn point_on_line_residual_signed_above_line() {
    // Line A=(0,0) → B=(4,0); P=(2,3) sits 3 above the x-axis.
    // Cross product = (x0-x1)·(y2-y1) − (y0-y1)·(x2-x1)
    //               = 2·0 − 3·4 = −12; length = 4; residual = −3.
    let mut s = Sketch::new();
    let a = s.add_point(0.0, 0.0);
    let b = s.add_point(4.0, 0.0);
    let line = s.add_line(a, b);
    let p = s.add_point(2.0, 3.0);
    let packed = pack(&s.data);

    let c = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::PointOnLine { point: p, line },
    };
    let r = residual(&c, &packed.vector, &packed.index, &s.data, &empty_params()).unwrap();
    assert!((r[0] - (-3.0)).abs() < EPS, "expected -3, got {}", r[0]);
}

#[test]
fn point_on_line_residual_signed_below_line() {
    // Line A=(0,0) → B=(4,0); P=(2,-2) sits 2 below the x-axis.
    // Cross = 2·0 − (−2)·4 = +8; length = 4; residual = +2.
    let mut s = Sketch::new();
    let a = s.add_point(0.0, 0.0);
    let b = s.add_point(4.0, 0.0);
    let line = s.add_line(a, b);
    let p = s.add_point(2.0, -2.0);
    let packed = pack(&s.data);

    let c = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::PointOnLine { point: p, line },
    };
    let r = residual(&c, &packed.vector, &packed.index, &s.data, &empty_params()).unwrap();
    assert!((r[0] - 2.0).abs() < EPS, "expected +2, got {}", r[0]);
}

#[test]
fn point_on_line_residual_zero_at_endpoint() {
    // Endpoints lie on the (infinite) line by definition.
    let mut s = Sketch::new();
    let a = s.add_point(1.0, 1.0);
    let b = s.add_point(7.0, 9.0);
    let line = s.add_line(a, b);
    let packed = pack(&s.data);

    let c = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::PointOnLine { point: a, line },
    };
    let r = residual(&c, &packed.vector, &packed.index, &s.data, &empty_params()).unwrap();
    assert!(
        r[0].abs() < EPS,
        "endpoint residual should be 0, got {}",
        r[0]
    );
}

#[test]
fn point_on_line_diagonal_normalises_by_length() {
    // Line A=(0,0) → B=(3,4); length = 5. Take P=(0,5).
    // Cross = (0-0)·(4-0) − (5-0)·(3-0) = 0 − 15 = −15; residual = −15/5 = −3.
    let mut s = Sketch::new();
    let a = s.add_point(0.0, 0.0);
    let b = s.add_point(3.0, 4.0);
    let line = s.add_line(a, b);
    let p = s.add_point(0.0, 5.0);
    let packed = pack(&s.data);

    let c = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::PointOnLine { point: p, line },
    };
    let r = residual(&c, &packed.vector, &packed.index, &s.data, &empty_params()).unwrap();
    assert!((r[0] - (-3.0)).abs() < EPS, "expected -3, got {}", r[0]);
}

#[test]
fn point_on_line_degenerate_zero_length_line_errors() {
    // A line whose endpoints coincide has no defined direction;
    // implementation reports EntityNotFound for the line.
    use signex_sketch::error::SketchError;
    let mut s = Sketch::new();
    let a = s.add_point(2.0, 2.0);
    let b = s.add_point(2.0, 2.0);
    let line = s.add_line(a, b);
    let p = s.add_point(0.0, 0.0);
    let packed = pack(&s.data);

    let c = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::PointOnLine { point: p, line },
    };
    let err = residual(&c, &packed.vector, &packed.index, &s.data, &empty_params()).unwrap_err();
    match err {
        SketchError::EntityNotFound(id) => assert_eq!(id, line),
        other => panic!("expected EntityNotFound(line), got {other:?}"),
    }
}

// ─── DistancePtLine ─────────────────────────────────────────────────

#[test]
fn distance_pt_line_zero_when_target_matches() {
    // Line A=(0,0) → B=(4,0); P=(0,-3) sits 3 below the line.
    // Signed perp distance with cross-product convention = +3.
    // target = 3 ⇒ residual = 3 − 3 = 0.
    let mut s = Sketch::new();
    let a = s.add_point(0.0, 0.0);
    let b = s.add_point(4.0, 0.0);
    let line = s.add_line(a, b);
    let p = s.add_point(0.0, -3.0);
    let packed = pack(&s.data);

    let c = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::DistancePtLine {
            point: p,
            line,
            target: DimTarget::Literal(3.0),
        },
    };
    let r = residual(&c, &packed.vector, &packed.index, &s.data, &empty_params()).unwrap();
    assert_eq!(r.len(), 1);
    assert!(r[0].abs() < EPS, "expected 0, got {}", r[0]);
}

#[test]
fn distance_pt_line_left_side_positive_target_disagrees_in_sign() {
    // Line A=(0,0) → B=(4,0); P=(0,3) sits ABOVE the line.
    // Signed perp distance = −3 (cross product convention puts the
    // left-hand side of A→B at negative).
    // target = +3 ⇒ residual = −3 − 3 = −6.
    let mut s = Sketch::new();
    let a = s.add_point(0.0, 0.0);
    let b = s.add_point(4.0, 0.0);
    let line = s.add_line(a, b);
    let p = s.add_point(0.0, 3.0);
    let packed = pack(&s.data);

    let c = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::DistancePtLine {
            point: p,
            line,
            target: DimTarget::Literal(3.0),
        },
    };
    let r = residual(&c, &packed.vector, &packed.index, &s.data, &empty_params()).unwrap();
    assert!((r[0] - (-6.0)).abs() < EPS, "expected -6, got {}", r[0]);
}

#[test]
fn distance_pt_line_negative_target_satisfies_left_side() {
    // Same P=(0,3) above the line; target = −3 ⇒ residual = −3 − (−3) = 0.
    // Confirms the sign is symmetric: a negative target asks the solver
    // to put P on the left-hand side of A→B.
    let mut s = Sketch::new();
    let a = s.add_point(0.0, 0.0);
    let b = s.add_point(4.0, 0.0);
    let line = s.add_line(a, b);
    let p = s.add_point(0.0, 3.0);
    let packed = pack(&s.data);

    let c = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::DistancePtLine {
            point: p,
            line,
            target: DimTarget::Literal(-3.0),
        },
    };
    let r = residual(&c, &packed.vector, &packed.index, &s.data, &empty_params()).unwrap();
    assert!(r[0].abs() < EPS, "expected 0, got {}", r[0]);
}

#[test]
fn distance_pt_line_target_zero_reduces_to_point_on_line() {
    // With target = 0, DistancePtLine and PointOnLine must produce the
    // same residual. Use a non-trivial geometry to exercise both code
    // paths (line A=(0,0) → B=(3,4), P=(0,5)).
    let mut s = Sketch::new();
    let a = s.add_point(0.0, 0.0);
    let b = s.add_point(3.0, 4.0);
    let line = s.add_line(a, b);
    let p = s.add_point(0.0, 5.0);
    let packed = pack(&s.data);

    let c_pol = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::PointOnLine { point: p, line },
    };
    let c_dpl = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::DistancePtLine {
            point: p,
            line,
            target: DimTarget::Literal(0.0),
        },
    };
    let r_pol = residual(
        &c_pol,
        &packed.vector,
        &packed.index,
        &s.data,
        &empty_params(),
    )
    .unwrap();
    let r_dpl = residual(
        &c_dpl,
        &packed.vector,
        &packed.index,
        &s.data,
        &empty_params(),
    )
    .unwrap();

    assert_eq!(r_pol.len(), 1);
    assert_eq!(r_dpl.len(), 1);
    assert!(
        (r_pol[0] - r_dpl[0]).abs() < EPS,
        "PointOnLine ({}) and DistancePtLine target=0 ({}) must match",
        r_pol[0],
        r_dpl[0]
    );
}

// ─── PointOnArc ─────────────────────────────────────────────────────

#[test]
fn point_on_arc_residual_zero_on_3_4_5_triangle() {
    // Arc: center=(0,0), start=(5,0), end=(0,5), CCW. Radius = 5.
    // P = (3,4) — distance to centre = 5 ⇒ residual = 0.
    let mut s = Sketch::new();
    let center = s.add_point(0.0, 0.0);
    let start = s.add_point(5.0, 0.0);
    let end = s.add_point(0.0, 5.0);
    let arc = s.add_arc(center, start, end, true);
    let p = s.add_point(3.0, 4.0);
    let packed = pack(&s.data);

    let c = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::PointOnArc { point: p, arc },
    };
    let r = residual(&c, &packed.vector, &packed.index, &s.data, &empty_params()).unwrap();
    assert_eq!(r.len(), 1);
    assert!(r[0].abs() < EPS, "expected 0, got {}", r[0]);
}

#[test]
fn point_on_arc_residual_zero_at_arc_start() {
    // The start Point of an arc lies on its underlying circle by
    // construction (radius is defined as |start − center|).
    let mut s = Sketch::new();
    let center = s.add_point(1.0, 2.0);
    let start = s.add_point(6.0, 2.0); // radius = 5
    let end = s.add_point(1.0, 7.0);
    let arc = s.add_arc(center, start, end, true);
    let packed = pack(&s.data);

    let c = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::PointOnArc { point: start, arc },
    };
    let r = residual(&c, &packed.vector, &packed.index, &s.data, &empty_params()).unwrap();
    assert!(
        r[0].abs() < EPS,
        "start should be on its own circle, got {}",
        r[0]
    );
}

#[test]
fn point_on_arc_residual_negative_when_inside_circle() {
    // Center=(0,0), start=(5,0) ⇒ radius = 5. P=(0,3) is inside.
    // Distance to centre = 3; residual = 3 − 5 = −2.
    let mut s = Sketch::new();
    let center = s.add_point(0.0, 0.0);
    let start = s.add_point(5.0, 0.0);
    let end = s.add_point(0.0, 5.0);
    let arc = s.add_arc(center, start, end, true);
    let p = s.add_point(0.0, 3.0);
    let packed = pack(&s.data);

    let c = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::PointOnArc { point: p, arc },
    };
    let r = residual(&c, &packed.vector, &packed.index, &s.data, &empty_params()).unwrap();
    assert!((r[0] - (-2.0)).abs() < EPS, "expected -2, got {}", r[0]);
}

#[test]
fn point_on_arc_residual_positive_when_outside_circle() {
    // Center=(0,0), start=(5,0) ⇒ radius = 5. P=(0,7) is outside.
    // Distance to centre = 7; residual = 7 − 5 = +2.
    let mut s = Sketch::new();
    let center = s.add_point(0.0, 0.0);
    let start = s.add_point(5.0, 0.0);
    let end = s.add_point(0.0, 5.0);
    let arc = s.add_arc(center, start, end, true);
    let p = s.add_point(0.0, 7.0);
    let packed = pack(&s.data);

    let c = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::PointOnArc { point: p, arc },
    };
    let r = residual(&c, &packed.vector, &packed.index, &s.data, &empty_params()).unwrap();
    assert!((r[0] - 2.0).abs() < EPS, "expected +2, got {}", r[0]);
}

#[test]
fn point_on_arc_residual_uses_start_for_radius_not_end() {
    // Arc has center=(0,0), start=(5,0) (radius from start = 5),
    // end=(0,8) (which would be radius 8 if end were used). Verify
    // the residual treats radius as |start−center|, not |end−center|,
    // by placing P on the start-radius circle.
    let mut s = Sketch::new();
    let center = s.add_point(0.0, 0.0);
    let start = s.add_point(5.0, 0.0); // start radius = 5
    let end = s.add_point(0.0, 8.0); // end radius = 8 (not used)
    let arc = s.add_arc(center, start, end, true);
    let p = s.add_point(3.0, 4.0); // distance to centre = 5
    let packed = pack(&s.data);

    let c = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::PointOnArc { point: p, arc },
    };
    let r = residual(&c, &packed.vector, &packed.index, &s.data, &empty_params()).unwrap();
    assert!(
        r[0].abs() < EPS,
        "P should be on the start-radius circle, got {}",
        r[0]
    );
}

// ─── DistancePtCircle (v0.23) ───────────────────────────────────────

#[test]
fn distance_pt_circle_residual_zero_when_on_circle() {
    // Circle centred at origin with radius 5; P=(5,0) sits on the
    // circle, target=0 → residual = 0.
    let mut s = Sketch::new();
    let centre = s.add_point(0.0, 0.0);
    let circle = s.add_circle(centre, 5.0);
    let p = s.add_point(5.0, 0.0);
    let packed = pack(&s.data);

    let c = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::DistancePtCircle {
            point: p,
            circle,
            target: DimTarget::Literal(0.0),
        },
    };
    let r = residual(&c, &packed.vector, &packed.index, &s.data, &empty_params()).unwrap();
    assert_eq!(r.len(), 1);
    assert!(r[0].abs() < EPS, "expected 0, got {}", r[0]);
}

#[test]
fn distance_pt_circle_residual_positive_outside() {
    // Circle radius 5; P=(8,0) is 8 from centre → distance to
    // boundary = 3. With target=2, residual = 8 − 5 − 2 = 1.
    let mut s = Sketch::new();
    let centre = s.add_point(0.0, 0.0);
    let circle = s.add_circle(centre, 5.0);
    let p = s.add_point(8.0, 0.0);
    let packed = pack(&s.data);

    let c = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::DistancePtCircle {
            point: p,
            circle,
            target: DimTarget::Literal(2.0),
        },
    };
    let r = residual(&c, &packed.vector, &packed.index, &s.data, &empty_params()).unwrap();
    assert!((r[0] - 1.0).abs() < EPS, "expected 1, got {}", r[0]);
}

#[test]
fn distance_pt_circle_residual_negative_inside() {
    // Circle radius 5; P=(3,0) is 3 from centre → distance to
    // boundary = -2 (inside). With target=-3, residual = 3 - 5 - (-3) = 1.
    let mut s = Sketch::new();
    let centre = s.add_point(0.0, 0.0);
    let circle = s.add_circle(centre, 5.0);
    let p = s.add_point(3.0, 0.0);
    let packed = pack(&s.data);

    let c = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::DistancePtCircle {
            point: p,
            circle,
            target: DimTarget::Literal(-3.0),
        },
    };
    let r = residual(&c, &packed.vector, &packed.index, &s.data, &empty_params()).unwrap();
    assert!((r[0] - 1.0).abs() < EPS, "expected 1, got {}", r[0]);
}

#[test]
fn distance_pt_circle_works_on_arc() {
    // Arc centred at origin with start=(5,0). Underlying radius = 5.
    // P=(7,0) is 7 from centre → distance to boundary = 2. target=2
    // → residual = 7 − 5 − 2 = 0.
    let mut s = Sketch::new();
    let centre = s.add_point(0.0, 0.0);
    let start = s.add_point(5.0, 0.0);
    let end = s.add_point(0.0, 5.0);
    let arc = s.add_arc(centre, start, end, true);
    let p = s.add_point(7.0, 0.0);
    let packed = pack(&s.data);

    let c = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::DistancePtCircle {
            point: p,
            circle: arc,
            target: DimTarget::Literal(2.0),
        },
    };
    let r = residual(&c, &packed.vector, &packed.index, &s.data, &empty_params()).unwrap();
    assert!(r[0].abs() < EPS, "expected 0, got {}", r[0]);
}

// ─── residual_count() sanity ────────────────────────────────────────

#[test]
fn point_on_residual_count_is_one_per_kind() {
    let mut s = Sketch::new();
    let a = s.add_point(0.0, 0.0);
    let b = s.add_point(4.0, 0.0);
    let line = s.add_line(a, b);
    let center = s.add_point(0.0, 0.0);
    let start = s.add_point(5.0, 0.0);
    let end = s.add_point(0.0, 5.0);
    let arc = s.add_arc(center, start, end, true);
    let p = s.add_point(2.0, 0.0);
    let packed = pack(&s.data);

    let cases: Vec<ConstraintKind> = vec![
        ConstraintKind::PointOnLine { point: p, line },
        ConstraintKind::PointOnArc { point: p, arc },
        ConstraintKind::DistancePtLine {
            point: p,
            line,
            target: DimTarget::Literal(1.5),
        },
    ];

    for kind in cases {
        assert_eq!(
            kind.residual_count(),
            1,
            "{kind:?} should contribute 1 scalar residual"
        );
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
