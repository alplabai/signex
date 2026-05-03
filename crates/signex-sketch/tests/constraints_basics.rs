//! Task 2.3 — residual tests for Coincident, DistancePtPt,
//! Horizontal, Vertical, Fixed.

mod common;
use common::Sketch;

use signex_sketch::constraint::{Constraint, ConstraintKind, DimTarget};
use signex_sketch::id::ConstraintId;
use signex_sketch::solver::residual::{residual, total_residual, ResolvedParams};
use signex_sketch::solver::state::pack;

fn empty_params() -> ResolvedParams {
    ResolvedParams::new()
}

#[test]
fn coincident_residual_zero_when_coincident() {
    let mut s = Sketch::new();
    let p1 = s.add_point(1.0, 2.0);
    let p2 = s.add_point(1.0, 2.0);
    let packed = pack(&s.data);

    let c = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Coincident { p1, p2 },
    };
    let r = residual(&c, &packed.vector, &packed.index, &s.data, &empty_params()).unwrap();
    assert_eq!(r.len(), 2);
    assert!(r[0].abs() < 1e-12);
    assert!(r[1].abs() < 1e-12);
}

#[test]
fn coincident_residual_nonzero_when_apart() {
    let mut s = Sketch::new();
    let p1 = s.add_point(0.0, 0.0);
    let p2 = s.add_point(3.0, 4.0);
    let packed = pack(&s.data);

    let c = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Coincident { p1, p2 },
    };
    let r = residual(&c, &packed.vector, &packed.index, &s.data, &empty_params()).unwrap();
    assert!((r[0] - 3.0).abs() < 1e-12);
    assert!((r[1] - 4.0).abs() < 1e-12);
}

#[test]
fn distance_pt_pt_literal_zero_on_3_4_5_triangle() {
    let mut s = Sketch::new();
    let p1 = s.add_point(0.0, 0.0);
    let p2 = s.add_point(3.0, 4.0);
    let packed = pack(&s.data);

    let c = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::DistancePtPt {
            p1,
            p2,
            target: DimTarget::Literal(5.0),
        },
    };
    let r = residual(&c, &packed.vector, &packed.index, &s.data, &empty_params()).unwrap();
    assert_eq!(r.len(), 1);
    assert!(r[0].abs() < 1e-12);
}

#[test]
fn distance_pt_pt_literal_nonzero_when_off() {
    let mut s = Sketch::new();
    let p1 = s.add_point(0.0, 0.0);
    let p2 = s.add_point(3.0, 4.0);
    let packed = pack(&s.data);

    let c = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::DistancePtPt {
            p1,
            p2,
            target: DimTarget::Literal(7.0),
        },
    };
    let r = residual(&c, &packed.vector, &packed.index, &s.data, &empty_params()).unwrap();
    // distance is 5; target is 7; residual = 5 − 7 = −2
    assert!((r[0] - (-2.0)).abs() < 1e-12);
}

#[test]
fn distance_pt_pt_expr_resolves_via_params() {
    let mut s = Sketch::new();
    let p1 = s.add_point(0.0, 0.0);
    let p2 = s.add_point(3.0, 4.0);
    let packed = pack(&s.data);

    let c = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::DistancePtPt {
            p1,
            p2,
            target: DimTarget::Expr("= pad_pitch".into()),
        },
    };
    let mut params = ResolvedParams::new();
    params.insert("pad_pitch".into(), 5.0);
    let r = residual(&c, &packed.vector, &packed.index, &s.data, &params).unwrap();
    assert!(r[0].abs() < 1e-12);
}

#[test]
fn distance_pt_pt_expr_full_arithmetic() {
    // Phase 4 Task 4.6 — DimTarget::Expr now supports full
    // expression evaluation, not just bare-name lookups.
    let mut s = Sketch::new();
    let p1 = s.add_point(0.0, 0.0);
    let p2 = s.add_point(3.0, 4.0);
    let packed = pack(&s.data);

    // Distance is 5 mm. Target = 2 + 3 = 5 mm via arithmetic on
    // dimensionless literals (auto-coerced to length-mm at the
    // ResolvedParams interpretation).
    let c = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::DistancePtPt {
            p1,
            p2,
            target: DimTarget::Expr("= 2mm + 3mm".into()),
        },
    };
    let r = residual(&c, &packed.vector, &packed.index, &s.data, &ResolvedParams::new()).unwrap();
    assert!(r[0].abs() < 1e-12, "5 - (2+3) = 0, got {}", r[0]);
}

#[test]
fn distance_pt_pt_expr_param_arithmetic() {
    // pad_pitch * 5 where pad_pitch = 1 (treated as 1mm in length
    // context) should evaluate to 5mm.
    let mut s = Sketch::new();
    let p1 = s.add_point(0.0, 0.0);
    let p2 = s.add_point(3.0, 4.0);
    let packed = pack(&s.data);

    let c = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::DistancePtPt {
            p1,
            p2,
            target: DimTarget::Expr("= pad_pitch * 5".into()),
        },
    };
    let mut params = ResolvedParams::new();
    params.insert("pad_pitch".into(), 1.0);
    let r = residual(&c, &packed.vector, &packed.index, &s.data, &params).unwrap();
    // distance 5 - target 5 = 0
    assert!(r[0].abs() < 1e-12, "expected 0, got {}", r[0]);
}

#[test]
fn horizontal_residual_zero_when_horizontal() {
    let mut s = Sketch::new();
    let a = s.add_point(0.0, 1.0);
    let b = s.add_point(5.0, 1.0);
    let line = s.add_line(a, b);
    let packed = pack(&s.data);

    let c = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Horizontal { line },
    };
    let r = residual(&c, &packed.vector, &packed.index, &s.data, &empty_params()).unwrap();
    assert_eq!(r.len(), 1);
    assert!(r[0].abs() < 1e-12);
}

#[test]
fn horizontal_residual_nonzero_when_diagonal() {
    let mut s = Sketch::new();
    let a = s.add_point(0.0, 1.0);
    let b = s.add_point(5.0, 4.0);
    let line = s.add_line(a, b);
    let packed = pack(&s.data);

    let c = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Horizontal { line },
    };
    let r = residual(&c, &packed.vector, &packed.index, &s.data, &empty_params()).unwrap();
    // y2 − y1 = 4 − 1 = 3
    assert!((r[0] - 3.0).abs() < 1e-12);
}

#[test]
fn vertical_residual_zero_when_vertical() {
    let mut s = Sketch::new();
    let a = s.add_point(2.0, 0.0);
    let b = s.add_point(2.0, 9.0);
    let line = s.add_line(a, b);
    let packed = pack(&s.data);

    let c = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Vertical { line },
    };
    let r = residual(&c, &packed.vector, &packed.index, &s.data, &empty_params()).unwrap();
    assert_eq!(r.len(), 1);
    assert!(r[0].abs() < 1e-12);
}

#[test]
fn vertical_residual_nonzero_when_diagonal() {
    let mut s = Sketch::new();
    let a = s.add_point(2.0, 0.0);
    let b = s.add_point(8.0, 9.0);
    let line = s.add_line(a, b);
    let packed = pack(&s.data);

    let c = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Vertical { line },
    };
    let r = residual(&c, &packed.vector, &packed.index, &s.data, &empty_params()).unwrap();
    // x2 − x1 = 8 − 2 = 6
    assert!((r[0] - 6.0).abs() < 1e-12);
}

#[test]
fn fixed_residual_is_empty() {
    // Fixed enforced by exclusion from state vector — residual is empty.
    let mut s = Sketch::new();
    let p = s.add_point(1.0, 2.0);
    let packed = pack(&s.data);

    let c = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Fixed { point: p },
    };
    let r = residual(&c, &packed.vector, &packed.index, &s.data, &empty_params()).unwrap();
    assert!(r.is_empty());
}

#[test]
fn total_residual_concatenates_per_constraint() {
    // 1 Coincident (2) + 1 DistancePtPt (1) + 1 Horizontal (1) = 4 scalars.
    let mut s = Sketch::new();
    let p1 = s.add_point(0.0, 0.0);
    let p2 = s.add_point(3.0, 4.0);
    let p3 = s.add_point(0.0, 1.0);
    let p4 = s.add_point(5.0, 1.0);
    let line = s.add_line(p3, p4);

    s.data.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Coincident { p1, p2 },
    });
    s.data.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::DistancePtPt {
            p1,
            p2,
            target: DimTarget::Literal(5.0),
        },
    });
    s.data.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Horizontal { line },
    });

    let packed = pack(&s.data);
    let r = total_residual(&s.data, &packed.vector, &packed.index, &empty_params()).unwrap();
    assert_eq!(r.len(), 4);

    // First two scalars: Coincident — points apart by (3, 4) → r[0]=3, r[1]=4
    assert!((r[0] - 3.0).abs() < 1e-12);
    assert!((r[1] - 4.0).abs() < 1e-12);
    // Third scalar: DistancePtPt(target=5) on a 3-4-5 triangle → 0
    assert!(r[2].abs() < 1e-12);
    // Fourth scalar: Horizontal on a y=1 horizontal line → 0
    assert!(r[3].abs() < 1e-12);
}

#[test]
fn total_residual_length_matches_constraint_kind_count_sum() {
    // Quick check that residual_count() summed across constraints
    // equals total_residual().len().
    let mut s = Sketch::new();
    let p1 = s.add_point(0.0, 0.0);
    let p2 = s.add_point(1.0, 1.0);
    let p3 = s.add_point(2.0, 0.0);
    let p4 = s.add_point(3.0, 0.0);
    let line = s.add_line(p3, p4);

    s.data.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Coincident { p1, p2 }, // 2
    });
    s.data.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Horizontal { line }, // 1
    });
    s.data.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Vertical { line }, // 1
    });
    s.data.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Midpoint { point: p1, line }, // 2
    });

    let packed = pack(&s.data);
    let r = total_residual(&s.data, &packed.vector, &packed.index, &empty_params()).unwrap();
    let expected: usize = s.data.constraints.iter().map(|c| c.kind.residual_count()).sum();
    assert_eq!(r.len(), expected);
    assert_eq!(r.len(), 2 + 1 + 1 + 2);
}

#[test]
fn residual_count_matches_returned_vector_length() {
    use signex_sketch::id::SketchEntityId;
    // Quick sanity: residual_count() must match residual() output length
    // for every implemented kind in this file.
    let mut s = Sketch::new();
    let a = s.add_point(0.0, 0.0);
    let b = s.add_point(1.0, 0.0);
    let line = s.add_line(a, b);
    let packed = pack(&s.data);

    let cases: Vec<ConstraintKind> = vec![
        ConstraintKind::Coincident { p1: a, p2: b },
        ConstraintKind::DistancePtPt {
            p1: a,
            p2: b,
            target: DimTarget::Literal(1.0),
        },
        ConstraintKind::Horizontal { line },
        ConstraintKind::Vertical { line },
        ConstraintKind::Fixed { point: SketchEntityId::new() },
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
    }
}
