//! Task 2.6 — residual tests for EqualLength, EqualRadius,
//! TangentLineArc, TangentArcArc.

mod common;
use common::Sketch;

use signex_sketch::constraint::{Constraint, ConstraintKind};
use signex_sketch::id::ConstraintId;
use signex_sketch::solver::residual::{residual, ResolvedParams};
use signex_sketch::solver::state::pack;

fn empty_params() -> ResolvedParams {
    ResolvedParams::new()
}

// ───────────────────────────── EqualLength ─────────────────────────────

#[test]
fn equal_length_residual_zero_on_equal_lines() {
    let mut s = Sketch::new();
    let a = s.add_point(0.0, 0.0);
    let b = s.add_point(3.0, 4.0); // length 5
    let c = s.add_point(10.0, 0.0);
    let d = s.add_point(13.0, 4.0); // length 5
    let l1 = s.add_line(a, b);
    let l2 = s.add_line(c, d);
    let packed = pack(&s.data);

    let cstr = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::EqualLength { l1, l2 },
    };
    let r = residual(&cstr, &packed.vector, &packed.index, &s.data, &empty_params()).unwrap();
    assert_eq!(r.len(), 1);
    assert!(r[0].abs() < 1e-12);
}

#[test]
fn equal_length_residual_nonzero_when_mismatched() {
    let mut s = Sketch::new();
    let a = s.add_point(0.0, 0.0);
    let b = s.add_point(3.0, 4.0); // length 5
    let c = s.add_point(0.0, 0.0);
    let d = s.add_point(8.0, 0.0); // length 8
    let l1 = s.add_line(a, b);
    let l2 = s.add_line(c, d);
    let packed = pack(&s.data);

    let cstr = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::EqualLength { l1, l2 },
    };
    let r = residual(&cstr, &packed.vector, &packed.index, &s.data, &empty_params()).unwrap();
    // residual = len2 − len1 = 8 − 5 = 3
    assert!((r[0] - 3.0).abs() < 1e-12);
}

// ───────────────────────────── EqualRadius ─────────────────────────────

#[test]
fn equal_radius_zero_on_two_equal_circles() {
    let mut s = Sketch::new();
    let c1 = s.add_point(0.0, 0.0);
    let c2 = s.add_point(10.0, 0.0);
    let e1 = s.add_circle(c1, 4.0);
    let e2 = s.add_circle(c2, 4.0);
    let packed = pack(&s.data);

    let cstr = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::EqualRadius { e1, e2 },
    };
    let r = residual(&cstr, &packed.vector, &packed.index, &s.data, &empty_params()).unwrap();
    assert_eq!(r.len(), 1);
    assert!(r[0].abs() < 1e-12);
}

#[test]
fn equal_radius_zero_on_two_equal_arcs() {
    let mut s = Sketch::new();
    // Arc 1: centre at origin, start at (3, 0), end at (0, 3) → radius 3
    let cen1 = s.add_point(0.0, 0.0);
    let start1 = s.add_point(3.0, 0.0);
    let end1 = s.add_point(0.0, 3.0);
    let e1 = s.add_arc(cen1, start1, end1, true);
    // Arc 2: centre at (10, 0), start at (10, 3), end at (13, 0)
    //         → radius |start − centre| = |(0, 3)| = 3
    let cen2 = s.add_point(10.0, 0.0);
    let start2 = s.add_point(10.0, 3.0);
    let end2 = s.add_point(13.0, 0.0);
    let e2 = s.add_arc(cen2, start2, end2, true);
    let packed = pack(&s.data);

    let cstr = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::EqualRadius { e1, e2 },
    };
    let r = residual(&cstr, &packed.vector, &packed.index, &s.data, &empty_params()).unwrap();
    assert_eq!(r.len(), 1);
    assert!(r[0].abs() < 1e-12);
}

#[test]
fn equal_radius_zero_on_circle_and_arc_combo() {
    let mut s = Sketch::new();
    // Circle: radius 5
    let c_centre = s.add_point(0.0, 0.0);
    let circle = s.add_circle(c_centre, 5.0);
    // Arc: centre at (20, 0), start at (24, 3) → radius = 5 (3-4-5 triangle)
    let a_centre = s.add_point(20.0, 0.0);
    let a_start = s.add_point(24.0, 3.0);
    let a_end = s.add_point(20.0, 5.0);
    let arc = s.add_arc(a_centre, a_start, a_end, true);
    let packed = pack(&s.data);

    let cstr = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::EqualRadius {
            e1: circle,
            e2: arc,
        },
    };
    let r = residual(&cstr, &packed.vector, &packed.index, &s.data, &empty_params()).unwrap();
    assert_eq!(r.len(), 1);
    assert!(r[0].abs() < 1e-12);
}

#[test]
fn equal_radius_nonzero_on_mismatched_radii() {
    let mut s = Sketch::new();
    let c1 = s.add_point(0.0, 0.0);
    let c2 = s.add_point(10.0, 0.0);
    let e1 = s.add_circle(c1, 3.0);
    let e2 = s.add_circle(c2, 7.0);
    let packed = pack(&s.data);

    let cstr = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::EqualRadius { e1, e2 },
    };
    let r = residual(&cstr, &packed.vector, &packed.index, &s.data, &empty_params()).unwrap();
    // residual = r2 − r1 = 7 − 3 = 4
    assert!((r[0] - 4.0).abs() < 1e-12);
}

// ─────────────────────────── TangentLineArc ───────────────────────────

#[test]
fn tangent_line_arc_zero_on_horizontal_line_above_centre() {
    let mut s = Sketch::new();
    // Arc: centre at origin, radius 5
    let cen = s.add_point(0.0, 0.0);
    let start = s.add_point(5.0, 0.0);
    let end = s.add_point(0.0, 5.0);
    let arc = s.add_arc(cen, start, end, true);
    // Horizontal line at y = 5 → tangent from above
    let a = s.add_point(-3.0, 5.0);
    let b = s.add_point(3.0, 5.0);
    let line = s.add_line(a, b);
    let packed = pack(&s.data);

    let cstr = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::TangentLineArc { line, arc },
    };
    let r = residual(&cstr, &packed.vector, &packed.index, &s.data, &empty_params()).unwrap();
    assert_eq!(r.len(), 1);
    assert!(r[0].abs() < 1e-12, "residual = {}", r[0]);
}

#[test]
fn tangent_line_arc_zero_on_horizontal_line_below_centre() {
    let mut s = Sketch::new();
    // Arc: centre at origin, radius 5
    let cen = s.add_point(0.0, 0.0);
    let start = s.add_point(5.0, 0.0);
    let end = s.add_point(0.0, 5.0);
    let arc = s.add_arc(cen, start, end, true);
    // Horizontal line at y = −5 → tangent from below
    let a = s.add_point(-3.0, -5.0);
    let b = s.add_point(3.0, -5.0);
    let line = s.add_line(a, b);
    let packed = pack(&s.data);

    let cstr = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::TangentLineArc { line, arc },
    };
    let r = residual(&cstr, &packed.vector, &packed.index, &s.data, &empty_params()).unwrap();
    assert!(r[0].abs() < 1e-12, "residual = {}", r[0]);
}

#[test]
fn tangent_line_arc_nonzero_on_non_tangent_line() {
    let mut s = Sketch::new();
    // Arc: centre at origin, radius 5
    let cen = s.add_point(0.0, 0.0);
    let start = s.add_point(5.0, 0.0);
    let end = s.add_point(0.0, 5.0);
    let arc = s.add_arc(cen, start, end, true);
    // Horizontal line at y = 8 → 3mm too far from the arc
    let a = s.add_point(-3.0, 8.0);
    let b = s.add_point(3.0, 8.0);
    let line = s.add_line(a, b);
    let packed = pack(&s.data);

    let cstr = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::TangentLineArc { line, arc },
    };
    let r = residual(&cstr, &packed.vector, &packed.index, &s.data, &empty_params()).unwrap();
    // |perp_dist| − r = 8 − 5 = 3
    assert!((r[0] - 3.0).abs() < 1e-12, "residual = {}", r[0]);
}

// ─────────────────────────── TangentArcArc ───────────────────────────

#[test]
fn tangent_arc_arc_external_zero_when_distance_equals_sum() {
    let mut s = Sketch::new();
    // Arc 1: centre at origin, radius 2
    let cen1 = s.add_point(0.0, 0.0);
    let start1 = s.add_point(2.0, 0.0);
    let end1 = s.add_point(0.0, 2.0);
    let a1 = s.add_arc(cen1, start1, end1, true);
    // Arc 2: centre at (5, 0), radius 3 → distance 5 = 2 + 3 (external tangent)
    let cen2 = s.add_point(5.0, 0.0);
    let start2 = s.add_point(8.0, 0.0);
    let end2 = s.add_point(5.0, 3.0);
    let a2 = s.add_arc(cen2, start2, end2, true);
    let packed = pack(&s.data);

    let cstr = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::TangentArcArc {
            a1,
            a2,
            internal: false,
        },
    };
    let r = residual(&cstr, &packed.vector, &packed.index, &s.data, &empty_params()).unwrap();
    assert_eq!(r.len(), 1);
    assert!(r[0].abs() < 1e-12, "residual = {}", r[0]);
}

#[test]
fn tangent_arc_arc_internal_zero_when_distance_equals_diff() {
    let mut s = Sketch::new();
    // Arc 1: centre at origin, radius 5
    let cen1 = s.add_point(0.0, 0.0);
    let start1 = s.add_point(5.0, 0.0);
    let end1 = s.add_point(0.0, 5.0);
    let a1 = s.add_arc(cen1, start1, end1, true);
    // Arc 2: centre at (3, 0), radius 2 → distance 3 = |5 − 2| (internal tangent)
    let cen2 = s.add_point(3.0, 0.0);
    let start2 = s.add_point(5.0, 0.0);
    let end2 = s.add_point(3.0, 2.0);
    let a2 = s.add_arc(cen2, start2, end2, true);
    let packed = pack(&s.data);

    let cstr = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::TangentArcArc {
            a1,
            a2,
            internal: true,
        },
    };
    let r = residual(&cstr, &packed.vector, &packed.index, &s.data, &empty_params()).unwrap();
    assert!(r[0].abs() < 1e-12, "residual = {}", r[0]);
}

#[test]
fn tangent_arc_arc_external_nonzero_on_mismatched_setup() {
    let mut s = Sketch::new();
    // Arc 1: centre at origin, radius 2
    let cen1 = s.add_point(0.0, 0.0);
    let start1 = s.add_point(2.0, 0.0);
    let end1 = s.add_point(0.0, 2.0);
    let a1 = s.add_arc(cen1, start1, end1, true);
    // Arc 2: centre at (10, 0), radius 3 → distance 10, sum of radii 5
    let cen2 = s.add_point(10.0, 0.0);
    let start2 = s.add_point(13.0, 0.0);
    let end2 = s.add_point(10.0, 3.0);
    let a2 = s.add_arc(cen2, start2, end2, true);
    let packed = pack(&s.data);

    let cstr = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::TangentArcArc {
            a1,
            a2,
            internal: false,
        },
    };
    let r = residual(&cstr, &packed.vector, &packed.index, &s.data, &empty_params()).unwrap();
    // residual = dist − (r1 + r2) = 10 − 5 = 5
    assert!((r[0] - 5.0).abs() < 1e-12, "residual = {}", r[0]);
}

#[test]
fn tangent_arc_arc_internal_nonzero_on_mismatched_setup() {
    let mut s = Sketch::new();
    // Arc 1: centre at origin, radius 5
    let cen1 = s.add_point(0.0, 0.0);
    let start1 = s.add_point(5.0, 0.0);
    let end1 = s.add_point(0.0, 5.0);
    let a1 = s.add_arc(cen1, start1, end1, true);
    // Arc 2: centre at (1, 0), radius 2 → distance 1, |r1 − r2| = 3
    let cen2 = s.add_point(1.0, 0.0);
    let start2 = s.add_point(3.0, 0.0);
    let end2 = s.add_point(1.0, 2.0);
    let a2 = s.add_arc(cen2, start2, end2, true);
    let packed = pack(&s.data);

    let cstr = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::TangentArcArc {
            a1,
            a2,
            internal: true,
        },
    };
    let r = residual(&cstr, &packed.vector, &packed.index, &s.data, &empty_params()).unwrap();
    // residual = dist − |r1 − r2| = 1 − 3 = −2
    assert!((r[0] - (-2.0)).abs() < 1e-12, "residual = {}", r[0]);
}

// ────────────────────────── residual_count() ──────────────────────────

#[test]
fn residual_count_is_one_for_all_task_2_6_kinds() {
    use signex_sketch::id::SketchEntityId;
    let mut s = Sketch::new();
    let p1 = s.add_point(0.0, 0.0);
    let p2 = s.add_point(1.0, 0.0);
    let line = s.add_line(p1, p2);
    let cen = s.add_point(0.0, 0.0);
    let arc_start = s.add_point(1.0, 0.0);
    let arc_end = s.add_point(0.0, 1.0);
    let arc = s.add_arc(cen, arc_start, arc_end, true);
    let circle = s.add_circle(cen, 1.0);
    let _ignored: SketchEntityId = SketchEntityId::new();
    let packed = pack(&s.data);

    let cases: Vec<ConstraintKind> = vec![
        ConstraintKind::EqualLength { l1: line, l2: line },
        ConstraintKind::EqualRadius {
            e1: circle,
            e2: arc,
        },
        ConstraintKind::TangentLineArc { line, arc },
        ConstraintKind::TangentArcArc {
            a1: arc,
            a2: circle,
            internal: false,
        },
    ];

    for kind in cases {
        assert_eq!(
            kind.residual_count(),
            1,
            "residual_count != 1 for {kind:?}"
        );
        let c = Constraint {
            id: ConstraintId::new(),
            kind: kind.clone(),
        };
        let r = residual(&c, &packed.vector, &packed.index, &s.data, &empty_params()).unwrap();
        assert_eq!(
            r.len(),
            1,
            "residual() returned {} elements for {kind:?}",
            r.len()
        );
    }
}
