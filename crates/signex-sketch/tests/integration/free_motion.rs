//! Mockup row 179: the solve counts the sketch's free degrees of freedom
//! and names the motions they are, for the Properties DOF card.

use crate::common::{self, Sketch};

use signex_sketch::constraint::{Constraint, ConstraintKind, DimTarget};
use signex_sketch::id::{ConstraintId, SketchEntityId};
use signex_sketch::solver::FullSolveOutput;
use signex_sketch::solver::dof::{DofColor, RANK_TOL};
use signex_sketch::solver::free_motion::{self, MotionKind};
use signex_sketch::solver::linalg::QrDecomposition;
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

/// The rank keeps the DOF analysis's absolute threshold: a pivot above it
/// counts however large the other entries are, as in the QR rank, so a
/// scaled Jacobian does not read one DOF more on the card than the pill.
#[test]
fn the_rank_counts_a_small_pivot_next_to_large_entries() {
    let jacobian = vec![vec![1.0e3, 0.0], vec![0.0, 5.0e-7]];
    let qr = QrDecomposition::new(&jacobian).expect("factors");
    assert_eq!(qr.rank(RANK_TOL), 2);
    assert_eq!(free_motion::rank(&jacobian, 2), 2);
    let singular = vec![vec![1.0e3, 0.0], vec![0.0, 1.0e-12]];
    assert_eq!(free_motion::rank(&singular, 2), 1);
}

/// The DOF card and the header pill share one rule: the card counts free
/// degrees of freedom exactly when the colours mark a point under-constrained.
#[test]
fn the_card_and_the_colours_agree_on_whether_anything_is_free() {
    let mut anchored = Sketch::new();
    let (p1, _) = horizontal_line(&mut anchored);
    constrain(&mut anchored, ConstraintKind::Fixed { point: p1 });
    let mut loose = Sketch::new();
    horizontal_line(&mut loose);
    for (sketch, free) in [(&anchored, false), (&loose, true)] {
        let out = solve(sketch);
        let under = out.colours.values().any(|c| *c == DofColor::Under);
        assert_eq!(out.free_motions.dof > 0, free, "{:?}", out.free_motions);
        assert_eq!(under, free, "{:?}", out.colours);
    }
}
