//! State-level tests for rejecting an over-constraining edit.

use signex_library::primitive::footprint::Footprint;
use signex_sketch::SketchData;
use signex_sketch::SketchError;
use signex_sketch::SolveError;
use signex_sketch::constraint::{Constraint, ConstraintKind, DimTarget};
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::{ConstraintId, SketchEntityId};
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};

use super::sketch_dispatch::apply_sketch_edit;
use super::sketch_mode::SketchEdit;
use super::state::FootprintEditorState;

fn distance(p1: SketchEntityId, p2: SketchEntityId, mm: f64) -> Constraint {
    Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::DistancePtPt {
            p1,
            p2,
            target: DimTarget::Literal(mm),
        },
    }
}

/// P1 fixed at the origin, P2 free, plus one satisfiable 5 mm distance.
fn solved_footprint() -> (
    Footprint,
    FootprintEditorState,
    SketchEntityId,
    SketchEntityId,
) {
    let plane = PlaneId::new();
    let (id1, id2) = (SketchEntityId::new(), SketchEntityId::new());
    let mut fp = Footprint::empty("conflict");
    fp.sketch = Some(SketchData {
        planes: vec![Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        }],
        entities: vec![
            Entity::new(id1, plane, EntityKind::Point { x: 0.0, y: 0.0 }),
            Entity::new(id2, plane, EntityKind::Point { x: 1.0, y: 0.0 }),
        ],
        constraints: vec![
            Constraint {
                id: ConstraintId::new(),
                kind: ConstraintKind::Fixed { point: id1 },
            },
            distance(id1, id2, 5.0),
        ],
        ..SketchData::default()
    });
    let mut state = FootprintEditorState::from_footprint(&fp);
    apply_sketch_edit(&mut state, &mut fp, SketchEdit::ForceRebuild)
        .expect("the starting sketch is satisfiable");
    (fp, state, id1, id2)
}

#[test]
fn a_conflicting_edit_sets_the_highlight_and_reverts_the_sketch() {
    let (mut fp, mut state, p1, p2) = solved_footprint();
    let good = fp.sketch.clone();
    let conflicting = distance(p1, p2, 9.0);
    let conflicting_id = conflicting.id;

    let result = apply_sketch_edit(&mut state, &mut fp, SketchEdit::AddConstraint(conflicting));

    let Err(SketchError::SolveFailed(SolveError::OverConstrained(ids))) = result else {
        panic!("expected an over-constraint error, got {result:?}");
    };
    assert!(ids.contains(&conflicting_id), "{ids:?}");
    assert!(state.conflict.contains_constraint(conflicting_id));
    assert_eq!(
        state.conflict.constraints().len(),
        ids.len(),
        "the highlight is exactly the conflict the solver named"
    );
    assert_eq!(fp.sketch, good, "the sketch is back at its last good state");
    assert!(
        state.last_solve.is_some(),
        "the reverted sketch is solved again, so the canvas shows the good state"
    );
    let named = state.conflict.entities(fp.sketch.as_ref().expect("sketch"));
    assert!(named.contains(&p1) && named.contains(&p2));
}

#[test]
fn the_rejected_constraints_entities_stay_highlighted_after_the_revert() {
    let (mut fp, mut state, p1, p2) = solved_footprint();
    let conflicting = distance(p1, p2, 9.0);
    let conflicting_id = conflicting.id;

    apply_sketch_edit(&mut state, &mut fp, SketchEdit::AddConstraint(conflicting))
        .expect_err("9 mm contradicts 5 mm");

    let sketch = fp.sketch.as_ref().expect("sketch");
    assert!(
        !sketch.constraints.iter().any(|c| c.id == conflicting_id),
        "precondition: the rejected constraint no longer exists"
    );
    // Resolve against a sketch with no constraints at all: only the refs
    // stored before the revert can name the entities now.
    let from_stored_refs = state.conflict.entities(&SketchData::default());
    assert!(
        from_stored_refs.contains(&p1) && from_stored_refs.contains(&p2),
        "{from_stored_refs:?}"
    );
}

#[test]
fn a_successful_solve_clears_the_highlight() {
    let (mut fp, mut state, p1, p2) = solved_footprint();
    apply_sketch_edit(
        &mut state,
        &mut fp,
        SketchEdit::AddConstraint(distance(p1, p2, 9.0)),
    )
    .expect_err("9 mm contradicts 5 mm");
    assert!(!state.conflict.is_empty(), "precondition: conflict shown");

    apply_sketch_edit(&mut state, &mut fp, SketchEdit::ForceRebuild)
        .expect("the reverted sketch solves");

    assert!(state.conflict.is_empty());
}

#[test]
fn a_constraint_on_a_zero_length_line_is_reported_to_the_user() {
    use super::sketch_dispatch::apply_sketch_edit_with_warnings;

    let _ = crate::diagnostics::init_logging();
    let (mut fp, mut state, p1, _p2) = solved_footprint();
    let sketch = fp.sketch.as_mut().expect("sketch");
    let plane = sketch.planes[0].id;
    let (a, b, line) = (
        SketchEntityId::new(),
        SketchEntityId::new(),
        SketchEntityId::new(),
    );
    sketch.entities.extend([
        Entity::new(a, plane, EntityKind::Point { x: 2.0, y: 2.0 }),
        Entity::new(b, plane, EntityKind::Point { x: 2.0, y: 2.0 }),
        Entity::new(line, plane, EntityKind::Line { start: a, end: b }),
    ]);
    let on_line = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::PointOnLine { point: p1, line },
    };

    apply_sketch_edit_with_warnings(&mut state, &mut fp, SketchEdit::AddConstraint(on_line));

    assert!(
        state
            .solve_warnings
            .iter()
            .any(|w| w.contains("zero length")),
        "{:?}",
        state.solve_warnings
    );
    assert!(
        !state.solve_warnings.iter().any(|w| w.contains("internal")),
        "{:?}",
        state.solve_warnings
    );

    // The Messages panel gets it too, naming the line (a unique id: the
    // diagnostics ring is shared by every test in the binary), and says the
    // edit was kept: a zero-length line is not reverted.
    let logged = crate::diagnostics::recent_entries().into_iter().any(|e| {
        e.message.contains(&line.to_string())
            && e.message.contains("zero length")
            && e.message.contains("the edit was kept")
    });
    assert!(logged, "the warning must reach the Messages panel");
    assert!(
        fp.sketch
            .as_ref()
            .expect("sketch")
            .entities
            .iter()
            .any(|e| e.id == line),
        "the edit is kept, not reverted"
    );
}

/// A drag re-solves on every pointer tick, and a kept zero-length line makes
/// every one of those solves fail the same way. The Properties strip shows it
/// for as long as it holds; the Messages panel hears it once.
#[test]
fn repeated_failing_solves_reach_the_messages_panel_once() {
    use super::sketch_dispatch::apply_sketch_edit_with_warnings;

    let _ = crate::diagnostics::init_logging();
    let (mut fp, mut state, p1, _p2) = solved_footprint();
    let sketch = fp.sketch.as_mut().expect("sketch");
    let plane = sketch.planes[0].id;
    let (a, b, line) = (
        SketchEntityId::new(),
        SketchEntityId::new(),
        SketchEntityId::new(),
    );
    sketch.entities.extend([
        Entity::new(a, plane, EntityKind::Point { x: 2.0, y: 2.0 }),
        Entity::new(b, plane, EntityKind::Point { x: 2.0, y: 2.0 }),
        Entity::new(line, plane, EntityKind::Line { start: a, end: b }),
    ]);
    let on_line = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::PointOnLine { point: p1, line },
    };
    apply_sketch_edit_with_warnings(&mut state, &mut fp, SketchEdit::AddConstraint(on_line));

    let line_id = line.to_string();
    let entries = crate::diagnostics::recent_entries();
    assert!(
        entries.iter().any(|e| e.message.contains(&line_id)),
        "the first failing solve logged a line naming the zero-length line"
    );
    let marker = entries.last().map_or(0, |e| e.id);
    for _ in 0..5 {
        apply_sketch_edit_with_warnings(&mut state, &mut fp, SketchEdit::ForceRebuild);
    }

    // Other tests share the ring and fail sketch solves of their own, so a
    // generic phrase such as "constraint colours" matches their entries too
    // (#1320). Count only entries that name this test's line: the failure
    // message carries the line id, which no other test's sketch can have.
    let added = crate::diagnostics::recent_entries()
        .into_iter()
        .filter(|e| e.id > marker)
        .filter(|e| e.message.contains(&line_id))
        .count();
    assert_eq!(added, 0, "five more failing solves, no new log line");
    assert!(
        state
            .solve_warnings
            .iter()
            .any(|w| w.contains("zero length")),
        "the strip still shows it: {:?}",
        state.solve_warnings
    );
}
