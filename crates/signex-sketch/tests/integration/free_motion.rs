//! Mockup row 179: the solve counts the sketch's free degrees of freedom
//! and names the motions they are, for the Properties DOF card.

use crate::common::{self, Sketch};

use signex_sketch::constraint::{Constraint, ConstraintKind, DimTarget};
use signex_sketch::id::{ConstraintId, SketchEntityId};
use signex_sketch::solver::FullSolveOutput;
use signex_sketch::solver::free_motion::MotionKind;
use signex_sketch::solver::residual::ResolvedParams;

fn constrain(s: &mut Sketch, kind: ConstraintKind) {
    s.data.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind,
    });
}

fn solve(s: &Sketch) -> FullSolveOutput {
    common::solver()
        .solve(&s.data, &ResolvedParams::new())
        .expect("solves")
}

/// A line from `(0, 0)` to `(5, 0)` held horizontal, at 5 mm.
fn horizontal_line(s: &mut Sketch) -> (SketchEntityId, SketchEntityId) {
    let p1 = s.add_point(0.0, 0.0);
    let p2 = s.add_point(5.0, 0.0);
    let line = s.add_line(p1, p2);
    constrain(s, ConstraintKind::Horizontal { line });
    constrain(
        s,
        ConstraintKind::DistancePtPt {
            p1,
            p2,
            target: DimTarget::Literal(5.0),
        },
    );
    (p1, p2)
}

#[test]
fn an_anchored_dimensioned_line_has_no_free_motion() {
    let mut s = Sketch::new();
    let (p1, _) = horizontal_line(&mut s);
    constrain(&mut s, ConstraintKind::Fixed { point: p1 });
    let out = solve(&s);
    assert_eq!(out.free_motions.dof, 0);
    assert!(
        out.free_motions.motions.is_empty(),
        "{:?}",
        out.free_motions
    );
}

#[test]
fn a_dimensioned_line_without_an_anchor_moves_in_any_direction_as_one() {
    let mut s = Sketch::new();
    let (p1, p2) = horizontal_line(&mut s);
    let out = solve(&s);
    assert_eq!(out.free_motions.dof, 2);
    let [motion] = out.free_motions.motions.as_slice() else {
        panic!("one motion: {:?}", out.free_motions);
    };
    assert_eq!(motion.kind, MotionKind::AnyDirection);
    assert_eq!(motion.dof, 2);
    let mut moved = motion.entities.clone();
    moved.sort_by_key(|id| id.0);
    let mut both = vec![p1, p2];
    both.sort_by_key(|id| id.0);
    assert_eq!(moved, both);
}

#[test]
fn an_anchored_horizontal_line_without_a_length_slides_its_end_along_x() {
    let mut s = Sketch::new();
    let p1 = s.add_point(0.0, 0.0);
    let p2 = s.add_point(5.0, 0.0);
    let line = s.add_line(p1, p2);
    constrain(&mut s, ConstraintKind::Fixed { point: p1 });
    constrain(&mut s, ConstraintKind::Horizontal { line });
    let out = solve(&s);
    assert_eq!(out.free_motions.dof, 1);
    let [motion] = out.free_motions.motions.as_slice() else {
        panic!("one motion: {:?}", out.free_motions);
    };
    assert_eq!(motion.entities, vec![p2]);
    let MotionKind::Slide { dx, dy } = motion.kind else {
        panic!("a slide: {motion:?}");
    };
    assert!(
        (dx.abs() - 1.0).abs() < 1e-9 && dy.abs() < 1e-9,
        "{dx}, {dy}"
    );
}

#[test]
fn a_point_nothing_constrains_is_free_in_any_direction() {
    let mut s = Sketch::new();
    let p1 = s.add_point(0.0, 0.0);
    let (q1, _) = horizontal_line(&mut s);
    constrain(&mut s, ConstraintKind::Fixed { point: q1 });
    let out = solve(&s);
    assert_eq!(out.free_motions.dof, 2);
    let [motion] = out.free_motions.motions.as_slice() else {
        panic!("one motion: {:?}", out.free_motions);
    };
    assert_eq!(motion.entities, vec![p1]);
    assert_eq!(motion.kind, MotionKind::AnyDirection);
}
