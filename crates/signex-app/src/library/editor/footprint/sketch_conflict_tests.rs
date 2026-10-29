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

/// Reject a fresh 9 mm distance (a new constraint id, as the UI mints one per
/// click) and report it. Returns how many Messages entries naming the
/// pre-existing constraint appeared.
fn reject_and_report(
    state: &mut FootprintEditorState,
    fp: &mut Footprint,
    p1: SketchEntityId,
    p2: SketchEntityId,
    mm: f64,
    needle: &str,
) -> usize {
    let before = crate::diagnostics::logged_count(needle);
    let result = apply_sketch_edit(state, fp, SketchEdit::AddConstraint(distance(p1, p2, mm)));
    let error = result.expect_err("the conflicting edit is rejected");
    super::sketch_dispatch::report_solve_failure(state, &error);
    crate::diagnostics::logged_count(needle) - before
}

/// The revert re-solves the last good sketch, and that clean solve must not
/// forget what was logged: the same rejected click made again (a new
/// constraint id each time) is the same conflict, and the Messages panel
/// hears about it once.
#[test]
fn a_rejected_edit_repeated_logs_its_conflict_once() {
    let _ = crate::diagnostics::init_logging();
    let (mut fp, mut state, p1, p2) = solved_footprint();
    // The pre-existing 5 mm distance is named in every attempt's message.
    let kept = fp.sketch.as_ref().expect("sketch").constraints[1].id;
    let needle = kept.to_string();

    assert_eq!(
        reject_and_report(&mut state, &mut fp, p1, p2, 9.0, &needle),
        1
    );
    for _ in 0..2 {
        assert_eq!(
            reject_and_report(&mut state, &mut fp, p1, p2, 9.0, &needle),
            0
        );
    }
}

/// A different conflict is news: another kind of constraint on the same
/// points logs again.
#[test]
fn a_different_rejected_conflict_is_logged() {
    let _ = crate::diagnostics::init_logging();
    let (mut fp, mut state, p1, p2) = solved_footprint();
    let kept = fp.sketch.as_ref().expect("sketch").constraints[1].id;
    let needle = kept.to_string();
    assert_eq!(
        reject_and_report(&mut state, &mut fp, p1, p2, 9.0, &needle),
        1
    );

    let coincident = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Coincident { p1, p2 },
    };
    let before = crate::diagnostics::logged_count(&needle);
    let error = apply_sketch_edit(&mut state, &mut fp, SketchEdit::AddConstraint(coincident))
        .expect_err("joining the points 5 mm apart is rejected");
    super::sketch_dispatch::report_solve_failure(&mut state, &error);
    assert_eq!(crate::diagnostics::logged_count(&needle) - before, 1);
}

/// A bake warning that holds across rejections (here a cutout on a circle)
/// was logged with the sketch; rejecting an edit must not log it again.
#[test]
fn a_standing_bake_warning_is_not_relogged_by_a_rejected_edit() {
    use signex_sketch::attr::BoardCutoutAttr;

    let _ = crate::diagnostics::init_logging();
    let (mut fp, mut state, p1, p2) = solved_footprint();
    let circle = SketchEntityId::new();
    let plane = fp.sketch.as_ref().expect("sketch").planes[0].id;
    let mut entity = Entity::new(
        circle,
        plane,
        EntityKind::Circle {
            center: p1,
            radius: 1.0,
        },
    );
    entity.board_cutout = Some(BoardCutoutAttr {
        edge_radius_expr: None,
        through: true,
    });
    fp.sketch.as_mut().expect("sketch").entities.push(entity);

    let needle = format!("entity {circle}: BoardCutoutAttr requires");
    let before = crate::diagnostics::logged_count(&needle);
    apply_sketch_edit(&mut state, &mut fp, SketchEdit::ForceRebuild).expect("solves");
    assert_eq!(
        crate::diagnostics::logged_count(&needle) - before,
        1,
        "logged with the sketch"
    );

    let kept = fp.sketch.as_ref().expect("sketch").constraints[1]
        .id
        .to_string();
    for _ in 0..2 {
        reject_and_report(&mut state, &mut fp, p1, p2, 9.0, &kept);
    }
    assert_eq!(
        crate::diagnostics::logged_count(&needle) - before,
        1,
        "rejections leave the standing warning alone"
    );
}
