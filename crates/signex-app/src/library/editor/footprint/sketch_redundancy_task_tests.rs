//! The redundancy pass runs off the UI thread, and a stale answer is
//! dropped (#1685).

use signex_library::{Footprint, FootprintFile};
use signex_sketch::SketchData;
use signex_sketch::constraint::{Constraint, ConstraintKind};
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::{ConstraintId, SketchEntityId};
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};

use super::sketch_redundancy_task::{
    dispatch_redundancy, run_redundancy, snapshot_redundancy_request,
};
use super::updates::apply_footprint_primitive_edit;
use crate::library::messages::FootprintEditorMsg;

/// An editor whose sketch states `Horizontal` on one line twice; the second
/// one is redundant. Returns it with that constraint's id.
fn editor_with_a_redundant_constraint() -> (
    crate::app::FootprintDocument,
    std::path::PathBuf,
    ConstraintId,
) {
    let path = std::path::PathBuf::from("redundant.snxfpt");
    let plane = PlaneId::new();
    let (a, b) = (SketchEntityId::new(), SketchEntityId::new());
    let line = SketchEntityId::new();
    let first = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Horizontal { line },
    };
    let second = Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Horizontal { line },
    };
    let second_id = second.id;
    let mut fp = Footprint::empty("redundant");
    fp.sketch = Some(SketchData {
        planes: vec![Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        }],
        entities: vec![
            Entity::new(a, plane, EntityKind::Point { x: 0.0, y: 0.0 }),
            Entity::new(b, plane, EntityKind::Point { x: 4.0, y: 0.5 }),
            Entity::new(line, plane, EntityKind::Line { start: a, end: b }),
        ],
        constraints: vec![first, second],
        ..SketchData::default()
    });
    let editor =
        crate::app::FootprintDocument::new(path.clone(), FootprintFile::from_footprint(fp));
    (editor, path, second_id)
}

/// An edit that is not a drag tick, so `update` solves it inline.
fn inline_edit() -> FootprintEditorMsg {
    FootprintEditorMsg::SketchPlacePoint {
        x_mm: 9.0,
        y_mm: 9.0,
    }
}

#[test]
fn an_inline_solve_leaves_the_redundancy_pass_to_a_task() {
    let (mut editor, path, redundant) = editor_with_a_redundant_constraint();

    apply_footprint_primitive_edit(&mut editor, inline_edit());

    let solve = editor.state.last_solve.as_ref().expect("the edit solved");
    assert!(
        solve.redundant_constraints.is_empty(),
        "update must not have run the redundancy pass"
    );
    assert!(editor.state.sketch_solve.is_redundancy_owed());
    let _task = dispatch_redundancy(&mut editor, &path);
    assert!(
        editor.state.sketch_solve.is_redundancy_running(),
        "the pass is handed to a task"
    );

    let answer = run_redundancy(snapshot_redundancy_request(&editor).expect("a solved sketch"));
    apply_footprint_primitive_edit(&mut editor, answer);

    let solve = editor.state.last_solve.as_ref().expect("still solved");
    assert_eq!(solve.redundant_constraints, vec![redundant]);
    assert!(!editor.state.sketch_solve.is_redundancy_owed());
    assert!(!editor.state.sketch_solve.is_redundancy_running());
}

#[test]
fn a_redundancy_answer_for_an_older_sketch_is_dropped() {
    let (mut editor, path, _redundant) = editor_with_a_redundant_constraint();
    apply_footprint_primitive_edit(&mut editor, inline_edit());
    let _task = dispatch_redundancy(&mut editor, &path);
    let stale = run_redundancy(snapshot_redundancy_request(&editor).expect("a solved sketch"));

    // The sketch changes while the pass runs.
    apply_footprint_primitive_edit(&mut editor, inline_edit());
    apply_footprint_primitive_edit(&mut editor, stale);

    let solve = editor.state.last_solve.as_ref().expect("solved");
    assert!(
        solve.redundant_constraints.is_empty(),
        "the stale answer must not be applied"
    );
    assert!(
        editor.state.sketch_solve.is_redundancy_owed(),
        "the newer solve still owes its own pass"
    );
    assert!(
        !editor.state.sketch_solve.is_redundancy_running(),
        "the landed answer frees the slot"
    );
}

#[test]
fn an_undo_while_the_pass_runs_does_not_start_one_on_the_old_solve() {
    let (mut editor, path, _redundant) = editor_with_a_redundant_constraint();
    apply_footprint_primitive_edit(&mut editor, inline_edit());
    let _task = dispatch_redundancy(&mut editor, &path);
    let stale = run_redundancy(snapshot_redundancy_request(&editor).expect("a solved sketch"));

    assert!(editor.undo(), "the edit is an undo step");
    apply_footprint_primitive_edit(&mut editor, stale);
    let _task = dispatch_redundancy(&mut editor, &path);

    assert!(
        !editor.state.sketch_solve.is_redundancy_owed(),
        "the restored sketch has no solve of its own to owe a pass for"
    );
    assert!(
        !editor.state.sketch_solve.is_redundancy_running(),
        "no pass may pair the restored sketch with the solved state from before the undo"
    );
}

#[test]
fn a_redundancy_pass_that_did_not_complete_marks_nothing_redundant() {
    let (mut editor, path, _redundant) = editor_with_a_redundant_constraint();
    apply_footprint_primitive_edit(&mut editor, inline_edit());
    let _task = dispatch_redundancy(&mut editor, &path);
    let generation = editor.state.sketch_solve.generation();
    let failed = FootprintEditorMsg::RedundancyChecked {
        instance: editor.state.sketch_solve.instance(),
        generation,
        footprint_idx: editor.active_idx,
        report: Err("the redundancy check did not complete: panicked".into()),
    };

    apply_footprint_primitive_edit(&mut editor, failed);

    let solve = editor.state.last_solve.as_ref().expect("still solved");
    assert!(
        solve.redundancy_pending,
        "a failed pass is not an answer: the solve still shows its check as pending"
    );
    assert!(!editor.state.sketch_solve.is_redundancy_running());
    assert!(!editor.state.sketch_solve.is_redundancy_owed());
}
