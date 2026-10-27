//! Deleting an entity must never leave a dangling reference, and a
//! sketch that already has one must fail the solve with a typed error.

use crate::common;
use common::Sketch;

use signex_sketch::SolveError;
use signex_sketch::array::{Array, ArrayId, ArrayKind, NumberingScheme};
use signex_sketch::constraint::{Constraint, ConstraintKind};
use signex_sketch::id::ConstraintId;
use signex_sketch::solver::residual::ResolvedParams;

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

    common::solver()
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

    let err = common::solver()
        .solve(&s.data, &ResolvedParams::new())
        .unwrap_err();
    assert!(
        matches!(&err, SolveError::DanglingReference(m) if m == &vec![a]),
        "{err:?}"
    );

    // Deleting anything recovers the sketch: the orphaned line goes too.
    let removed = s.data.remove_entity_cascade(b);
    assert!(removed.repaired_entities.contains(&line));
    assert!(s.data.dangling_refs().is_empty());
    common::solver()
        .solve(&s.data, &ResolvedParams::new())
        .expect("healed sketch solves");
}

fn array(kind: ArrayKind) -> Array {
    Array {
        id: ArrayId::new(),
        kind,
        numbering: NumberingScheme::default(),
    }
}

fn linear(source: signex_sketch::id::SketchEntityId) -> Array {
    array(ArrayKind::Linear {
        source,
        count_expr: "3".into(),
        dx_expr: "1mm".into(),
        dy_expr: "0mm".into(),
    })
}

#[test]
fn deleting_a_linear_array_source_removes_the_array() {
    let mut s = Sketch::new();
    let src = s.add_point(0.0, 0.0);
    let other = s.add_point(5.0, 5.0);
    let arr = linear(src);
    let id = arr.id;
    s.data.arrays.push(arr);
    s.data.arrays.push(linear(other));

    let removed = s.data.remove_entity_cascade(src);

    assert_eq!(removed.arrays, vec![id]);
    assert_eq!(s.data.arrays.len(), 1);
    assert!(s.data.dangling_refs().is_empty());
}

#[test]
fn deleting_a_polar_array_centre_removes_the_array() {
    let mut s = Sketch::new();
    let src = s.add_point(1.0, 0.0);
    let centre = s.add_point(0.0, 0.0);
    s.data.arrays.push(array(ArrayKind::Polar {
        source: src,
        center: centre,
        count_expr: "4".into(),
        sweep_angle_expr: "360deg".into(),
        depopulation: None,
    }));

    let removed = s.data.remove_entity_cascade(centre);

    assert_eq!(removed.arrays.len(), 1);
    assert!(s.data.arrays.is_empty());
    assert!(s.data.dangling_refs().is_empty());
}

#[test]
fn an_array_pointing_at_a_missing_entity_is_a_dangling_reference() {
    let mut s = Sketch::new();
    let ghost = signex_sketch::id::SketchEntityId::new();
    s.data.arrays.push(linear(ghost));

    assert_eq!(s.data.dangling_refs(), vec![ghost]);
    let err = common::solver()
        .solve(&s.data, &ResolvedParams::new())
        .unwrap_err();
    assert!(matches!(err, SolveError::DanglingReference(_)), "{err:?}");
}
