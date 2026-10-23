//! Deleting an entity must never leave a dangling reference, and a
//! sketch that already has one must fail the solve with a typed error.

mod common;
use common::Sketch;

use signex_sketch::constraint::{Constraint, ConstraintKind};
use signex_sketch::id::ConstraintId;
use signex_sketch::solver::residual::ResolvedParams;
use signex_sketch::{SolveError, Solver};

fn constrain(s: &mut Sketch, kind: ConstraintKind) -> ConstraintId {
    let id = ConstraintId::new();
    s.data.constraints.push(Constraint { id, kind });
    id
}

#[test]
fn deleting_a_point_removes_its_line_arc_and_constraints_and_still_solves() {
    let mut s = Sketch::new();
    let a = s.add_point(0.0, 0.0);
    let b = s.add_point(4.0, 0.1);
    let c = s.add_point(0.0, 3.0);
    let d = s.add_point(2.0, 1.0);
    let keep = s.add_point(9.0, 9.0);
    let line = s.add_line(a, b);
    let arc = s.add_arc(c, a, d, true);
    let keep_line = s.add_line(c, d);
    constrain(&mut s, ConstraintKind::Horizontal { line });
    constrain(&mut s, ConstraintKind::PointOnLine { point: d, line });
    constrain(&mut s, ConstraintKind::Fixed { point: a });
    let kept = constrain(&mut s, ConstraintKind::Fixed { point: keep });
    let kept_h = constrain(&mut s, ConstraintKind::Vertical { line: keep_line });

    let removed = s.data.remove_entity_cascade(a);

    assert!(removed.entities.contains(&a));
    assert!(removed.entities.contains(&line));
    assert!(removed.entities.contains(&arc));
    assert_eq!(removed.constraints.len(), 3);
    assert!(s.data.dangling_refs().is_empty());
    let left: Vec<_> = s.data.constraints.iter().map(|c| c.id).collect();
    assert_eq!(left, vec![kept, kept_h]);
    assert!(s.data.entities.iter().any(|e| e.id == keep_line));

    Solver::default()
        .solve(&s.data, &ResolvedParams::new())
        .expect("the remaining sketch solves");
}

#[test]
fn deleting_a_line_keeps_its_endpoints() {
    let mut s = Sketch::new();
    let a = s.add_point(0.0, 0.0);
    let b = s.add_point(1.0, 0.0);
    let line = s.add_line(a, b);
    constrain(&mut s, ConstraintKind::Horizontal { line });

    let removed = s.data.remove_entity_cascade(line);

    assert_eq!(removed.entities, vec![line]);
    assert_eq!(s.data.entities.len(), 2);
    assert!(s.data.constraints.is_empty());
}

#[test]
fn a_sketch_with_a_dangling_ref_returns_a_typed_error_then_heals() {
    let mut s = Sketch::new();
    let a = s.add_point(0.0, 0.0);
    let b = s.add_point(1.0, 0.0);
    let line = s.add_line(a, b);
    // Remove the point behind the sketch's back, as a corrupt file would.
    s.data.entities.retain(|e| e.id != a);
    assert_eq!(s.data.dangling_refs(), vec![a]);

    let err = Solver::default()
        .solve(&s.data, &ResolvedParams::new())
        .unwrap_err();
    assert!(
        matches!(&err, SolveError::DanglingReference(m) if m == &vec![a]),
        "{err:?}"
    );

    // Deleting anything recovers the sketch: the orphaned line goes too.
    let removed = s.data.remove_entity_cascade(b);
    assert!(removed.entities.contains(&line));
    assert!(s.data.dangling_refs().is_empty());
    Solver::default()
        .solve(&s.data, &ResolvedParams::new())
        .expect("healed sketch solves");
}
