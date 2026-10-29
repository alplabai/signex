//! GH #1649 — a constraint that restates earlier ones is named, a consistent
//! independent set is not, and a conflict keeps its existing reporting.

use crate::common;
use common::Sketch;

use signex_sketch::constraint::{Constraint, ConstraintKind, DimTarget};
use signex_sketch::error::SolveError;
use signex_sketch::id::ConstraintId;
use signex_sketch::solver::residual::ResolvedParams;

fn constraint(kind: ConstraintKind) -> Constraint {
    Constraint {
        id: ConstraintId::new(),
        kind,
    }
}

#[test]
fn horizontal_applied_twice_flags_the_second() {
    let mut s = Sketch::new();
    let p1 = s.add_point(0.0, 0.0);
    let p2 = s.add_point(4.0, 0.5);
    let line = s.add_line(p1, p2);
    let first = constraint(ConstraintKind::Horizontal { line });
    let second = constraint(ConstraintKind::Horizontal { line });
    let second_id = second.id;
    s.data.constraints.extend([first, second]);

    let out = common::solver()
        .solve(&s.data, &ResolvedParams::new())
        .expect("consistent");
    assert_eq!(out.redundant_constraints, vec![second_id]);
    assert!(out.over_constraints.is_empty());
}

#[test]
fn an_independent_set_flags_nothing() {
    let mut s = Sketch::new();
    let p1 = s.add_point(0.0, 0.0);
    let p2 = s.add_point(4.0, 0.5);
    let line = s.add_line(p1, p2);
    s.data.constraints.extend([
        constraint(ConstraintKind::Fixed { point: p1 }),
        constraint(ConstraintKind::Horizontal { line }),
        constraint(ConstraintKind::DistancePtPt {
            p1,
            p2,
            target: DimTarget::Literal(4.0),
        }),
    ]);

    let out = common::solver()
        .solve(&s.data, &ResolvedParams::new())
        .expect("consistent");
    assert!(out.redundant_constraints.is_empty());
}

#[test]
fn a_conflict_is_still_reported_as_over_constrained() {
    let mut s = Sketch::new();
    let p1 = s.add_point(0.0, 0.0);
    let p2 = s.add_point(1.0, 0.0);
    let dist = |mm| {
        constraint(ConstraintKind::DistancePtPt {
            p1,
            p2,
            target: DimTarget::Literal(mm),
        })
    };
    s.data.constraints.extend([
        constraint(ConstraintKind::Fixed { point: p1 }),
        dist(5.0),
        dist(10.0),
    ]);

    let err = common::solver()
        .solve(&s.data, &ResolvedParams::new())
        .expect_err("5 mm and 10 mm conflict");
    assert!(matches!(err, SolveError::OverConstrained(_)), "{err:?}");
}

#[test]
fn coincident_after_point_on_line_is_not_flagged() {
    // PointOnLine(P, L) then Coincident(P, L.start): the second pins P to
    // the start point, which PointOnLine alone does not, so it is not free.
    let mut s = Sketch::new();
    let a = s.add_point(0.0, 0.0);
    let b = s.add_point(4.0, 1.0);
    let p = s.add_point(2.0, 0.5);
    let line = s.add_line(a, b);
    s.data.constraints.extend([
        constraint(ConstraintKind::Fixed { point: a }),
        constraint(ConstraintKind::Fixed { point: b }),
        constraint(ConstraintKind::PointOnLine { point: p, line }),
        constraint(ConstraintKind::Coincident { p1: p, p2: a }),
    ]);

    let out = common::solver()
        .solve(&s.data, &ResolvedParams::new())
        .expect("consistent");
    assert!(
        out.redundant_constraints.is_empty(),
        "{:?}",
        out.redundant_constraints
    );
}

#[test]
fn a_coincident_with_one_independent_axis_is_not_flagged() {
    // A fixed; Horizontal(A,P) and Horizontal(A,Q) fix P.y and Q.y. Then
    // Coincident(P,Q): its y-row is implied, its x-row is not, so deleting
    // it would free P.x against Q.x.
    let mut s = Sketch::new();
    let a = s.add_point(0.0, 0.0);
    let p = s.add_point(2.0, 0.1);
    let q = s.add_point(3.0, -0.1);
    let (lp, lq) = (s.add_line(a, p), s.add_line(a, q));
    s.data.constraints.extend([
        constraint(ConstraintKind::Fixed { point: a }),
        constraint(ConstraintKind::Horizontal { line: lp }),
        constraint(ConstraintKind::Horizontal { line: lq }),
        constraint(ConstraintKind::Coincident { p1: p, p2: q }),
    ]);

    let out = common::solver()
        .solve(&s.data, &ResolvedParams::new())
        .expect("consistent");
    assert!(
        out.redundant_constraints.is_empty(),
        "{:?}",
        out.redundant_constraints
    );
}
