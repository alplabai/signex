//! Solver faults and in-flight bookkeeping of the off-thread solve (#1047).

use signex_library::{Footprint, FootprintFile};
use signex_sketch::SketchData;
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};

use super::sketch_solve_task::{SolveOutcome, dispatch_pending, run_solve_with, snapshot_request};
use super::updates::apply_footprint_primitive_edit;
use crate::library::messages::FootprintEditorMsg;
use signex_sketch::SolveError;

/// An editor with one free point, and a tick that moves it.
fn point_editor(
    stem: &str,
) -> (
    crate::app::FootprintDocument,
    std::path::PathBuf,
    FootprintEditorMsg,
) {
    let path = std::path::PathBuf::from(format!("{stem}.snxfpt"));
    let plane = PlaneId::new();
    let point = SketchEntityId::new();
    let mut fp = Footprint::empty(stem);
    fp.sketch = Some(SketchData {
        planes: vec![Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        }],
        entities: vec![Entity::new(
            point,
            plane,
            EntityKind::Point { x: 0.0, y: 0.0 },
        )],
        ..SketchData::default()
    });
    let editor =
        crate::app::FootprintDocument::new(path.clone(), FootprintFile::from_footprint(fp));
    let tick = FootprintEditorMsg::SketchMovePoint {
        id: point,
        dx: 1.0,
        dy: 0.0,
    };
    (editor, path, tick)
}

#[test]
fn a_superseded_solve_keeps_the_slot_until_its_stale_answer_lands() {
    let (mut editor, path, tick) = point_editor("keep-slot");
    apply_footprint_primitive_edit(&mut editor, tick.clone());
    let _task = dispatch_pending(&mut editor, &path);
    let running = run_solve_with(snapshot_request(&editor).expect("a sketch"), |_, _| {
        SolveOutcome::SolveFailed(SolveError::Internal("stale".into()))
    });

    // An undo (or any inline solve) makes the running solve stale, but the
    // task still occupies the blocking pool.
    editor.state.sketch_solve.supersede();
    assert!(
        editor.state.sketch_solve.is_in_flight(),
        "the slot is held until the stale answer arrives"
    );
    apply_footprint_primitive_edit(&mut editor, tick);
    let _none = dispatch_pending(&mut editor, &path);
    assert!(
        editor.state.sketch_solve.is_solve_owed(),
        "the new tick waits for the slot instead of starting a second solve"
    );

    apply_footprint_primitive_edit(&mut editor, running);
    assert!(
        !editor.state.sketch_solve.is_in_flight(),
        "the stale answer frees the slot"
    );
    let _follow_up = dispatch_pending(&mut editor, &path);
    assert!(
        editor.state.sketch_solve.is_in_flight(),
        "the wanted follow-up starts on the newest sketch"
    );
}

#[test]
fn a_panicking_solve_becomes_a_reported_failure_and_frees_the_slot() {
    let path = std::path::PathBuf::from("panic.snxfpt");
    let plane = PlaneId::new();
    let point = SketchEntityId::new();
    let mut fp = Footprint::empty("panic");
    fp.sketch = Some(SketchData {
        planes: vec![Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        }],
        entities: vec![Entity::new(
            point,
            plane,
            EntityKind::Point { x: 0.0, y: 0.0 },
        )],
        ..SketchData::default()
    });
    let mut editor =
        crate::app::FootprintDocument::new(path.clone(), FootprintFile::from_footprint(fp));
    let tick = || FootprintEditorMsg::SketchMovePoint {
        id: point,
        dx: 1.0,
        dy: 0.0,
    };

    apply_footprint_primitive_edit(&mut editor, tick());
    let _task = dispatch_pending(&mut editor, &path);
    assert!(editor.state.sketch_solve.is_in_flight());

    let request = snapshot_request(&editor).expect("a sketch");
    let message = run_solve_with(request, |_, _| panic!("injected solver fault"));
    apply_footprint_primitive_edit(&mut editor, message);

    assert!(
        !editor.state.sketch_solve.is_in_flight(),
        "the slot must be free for the next solve"
    );
    assert!(
        editor
            .state
            .solve_warnings
            .iter()
            .any(|w| w.contains("solver panicked") && w.contains("injected solver fault")),
        "the failure reaches the Properties strip: {:?}",
        editor.state.solve_warnings
    );

    apply_footprint_primitive_edit(&mut editor, tick());
    let _again = dispatch_pending(&mut editor, &path);
    assert!(
        editor.state.sketch_solve.is_in_flight(),
        "a later drag tick starts a new solve"
    );
}
