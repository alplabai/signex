//! #1047 — the footprint sketch solve runs off the UI thread.
//!
//! A drag tick moves the geometry at once and owes a solve; the answer comes
//! back as `SketchSolved` and is applied only if no edit has happened since.
//! The runtime's `Task` is replaced by running the solve inline and feeding
//! the answer back, so nothing here sleeps or races.

#![expect(
    clippy::let_underscore_must_use,
    reason = "integration tests discard Results with `let _ = ...` routinely; this is test scaffolding, not a production `iced::Task` being dropped, and `[lints]` in Cargo.toml is package-scoped so each integration-test crate root needs its own"
)]

use signex_app::app::{FootprintEditorState, Message, Signex, TabInfo, TabKind};
use signex_app::library::editor::footprint::sketch_dispatch::apply_sketch_edit_with_warnings;
use signex_app::library::editor::footprint::sketch_mode::SketchEdit;
use signex_app::library::editor::footprint::sketch_solve_task::{run_solve, snapshot_request};
use signex_app::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
use signex_library::{Footprint, FootprintFile};
use signex_sketch::SketchData;
use signex_sketch::constraint::{Constraint, ConstraintKind, DimTarget};
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::{ConstraintId, SketchEntityId};
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
use std::path::{Path, PathBuf};

fn dispatch(app: &mut Signex, path: &Path, msg: FootprintEditorMsg) {
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.to_path_buf(),
        msg: PrimitiveEdit::Footprint(msg),
    }));
}

fn editor<'a>(app: &'a Signex, path: &Path) -> &'a FootprintEditorState {
    app.document_state.footprint_editors.get(path).unwrap()
}

/// The answer to the sketch as it stands now, as the message a `Task`
/// would deliver.
fn solve_now(app: &Signex, path: &Path) -> FootprintEditorMsg {
    run_solve(snapshot_request(editor(app, path)).expect("the editor has a sketch"))
}

/// Where the last applied solve put `id`.
fn solved_x(e: &FootprintEditorState, id: SketchEntityId) -> f64 {
    let solve = e.state.last_solve.as_ref().expect("a solve was applied");
    signex_sketch::solver::state::point_xy(
        id,
        &solve.result.state,
        &solve.result.index,
        e.primitive().sketch.as_ref().unwrap(),
    )
    .expect("solved point")
    .0
}

fn point(sketch: &SketchData, id: SketchEntityId) -> (f64, f64) {
    sketch
        .entities
        .iter()
        .find_map(|e| match e.kind {
            EntityKind::Point { x, y } if e.id == id => Some((x, y)),
            _ => None,
        })
        .expect("point exists")
}

struct Fixture {
    app: Signex,
    path: PathBuf,
    p2: SketchEntityId,
}

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

fn fixed(point: SketchEntityId) -> Constraint {
    Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Fixed { point },
    }
}

/// P1 and P2 fixed 8 mm apart, a free P3 exactly 5 mm from each:
/// a satisfiable triangle. Dragging P2 far away leaves P3 nowhere to go, so
/// the drag over-constrains the sketch.
fn fixture(stem: &str) -> Fixture {
    let path = PathBuf::from(format!("{stem}.snxfpt"));
    let plane = PlaneId::new();
    let (p1, p2, p3) = (
        SketchEntityId::new(),
        SketchEntityId::new(),
        SketchEntityId::new(),
    );
    let mut fp = Footprint::empty(stem);
    fp.sketch = Some(SketchData {
        planes: vec![Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        }],
        entities: vec![
            Entity::new(p1, plane, EntityKind::Point { x: 0.0, y: 0.0 }),
            Entity::new(p2, plane, EntityKind::Point { x: 8.0, y: 0.0 }),
            Entity::new(p3, plane, EntityKind::Point { x: 4.0, y: 3.0 }),
        ],
        constraints: vec![
            fixed(p1),
            fixed(p2),
            distance(p1, p3, 5.0),
            distance(p2, p3, 5.0),
        ],
        ..SketchData::default()
    });
    let mut state = FootprintEditorState::new(path.clone(), FootprintFile::from_footprint(fp));
    let (st, primitive) = state.parts_mut();
    apply_sketch_edit_with_warnings(st, primitive, SketchEdit::ForceRebuild);
    assert!(
        state.state.last_solve.is_some(),
        "the starting sketch solves"
    );
    state.dirty = false;

    let (mut app, _task) = Signex::new();
    app.document_state
        .footprint_editors
        .insert(path.clone(), state);
    app.document_state.tabs.push(TabInfo {
        title: stem.into(),
        path: path.clone(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: TabKind::FootprintEditor(path.clone()),
    });
    app.document_state.active_tab = 0;
    Fixture { app, path, p2 }
}

fn drag(id: SketchEntityId, dx: f64) -> FootprintEditorMsg {
    FootprintEditorMsg::SketchMovePoint { id, dx, dy: 0.0 }
}

#[test]
fn a_drag_tick_moves_the_geometry_at_once_and_owes_a_solve() {
    let Fixture { mut app, path, p2 } = fixture("tick-owes");
    let solved_before = editor(&app, &path).state.last_solve.clone();

    dispatch(&mut app, &path, drag(p2, 0.5));

    let e = editor(&app, &path);
    assert_eq!(
        point(e.primitive().sketch.as_ref().unwrap(), p2),
        (8.5, 0.0)
    );
    assert!(e.state.sketch_solve.is_in_flight(), "the solve was started");
    assert_eq!(
        e.state.last_solve.as_ref().map(|s| s.jacobian.len()),
        solved_before.as_ref().map(|s| s.jacobian.len()),
        "the tick itself did not run the solver: the last applied solve is still shown"
    );
}

#[test]
fn a_stale_result_after_a_newer_edit_is_dropped() {
    let Fixture { mut app, path, p2 } = fixture("stale");
    dispatch(&mut app, &path, drag(p2, 0.5));
    let stale = solve_now(&app, &path);
    dispatch(&mut app, &path, drag(p2, 0.5));
    let pads_before = editor(&app, &path).primitive().pads.clone();
    let warnings_before = editor(&app, &path).state.solve_warnings.clone();
    let history_before = editor(&app, &path).history.len();

    dispatch(&mut app, &path, stale);

    let e = editor(&app, &path);
    assert_eq!(
        e.primitive().pads,
        pads_before,
        "a stale answer bakes nothing"
    );
    assert_eq!(e.state.solve_warnings, warnings_before);
    assert_eq!(e.history.len(), history_before, "and records nothing");
    assert!(
        e.state.sketch_solve.is_solve_owed(),
        "the newer sketch is still owed its solve"
    );
}

#[test]
fn a_drag_applies_only_the_newest_result() {
    let Fixture { mut app, path, p2 } = fixture("newest");
    let mut stale = Vec::new();
    for _ in 0..3 {
        dispatch(&mut app, &path, drag(p2, 0.25));
        stale.push(solve_now(&app, &path));
    }
    let newest = stale.pop().expect("three ticks");
    for old in stale {
        dispatch(&mut app, &path, old);
        assert!(
            editor(&app, &path).state.sketch_solve.is_solve_owed(),
            "an older answer must not settle the newer sketch"
        );
    }

    dispatch(&mut app, &path, newest);

    let e = editor(&app, &path);
    assert!(!e.state.sketch_solve.is_solve_owed());
    assert!(
        (solved_x(e, p2) - 8.75).abs() < 1e-6,
        "the applied solve is the one for all three ticks"
    );
}

#[test]
fn undo_history_and_dirty_flag_match_the_inline_path() {
    // The inline path: one undo step per message and the dirty flag set.
    let Fixture { mut app, path, p2 } = fixture("undo-equiv");
    let original = editor(&app, &path).primitive().sketch.clone();
    assert_eq!(editor(&app, &path).history.len(), 0);
    assert!(!editor(&app, &path).dirty);

    for _ in 0..2 {
        dispatch(&mut app, &path, drag(p2, 0.5));
        let answer = solve_now(&app, &path);
        dispatch(&mut app, &path, answer);
    }

    let e = editor(&app, &path);
    assert_eq!(
        e.history.len(),
        2,
        "applying a result adds no step of its own"
    );
    assert!(e.dirty);

    // The reference: the same two edits through the inline dispatcher.
    let Fixture {
        app: mut ref_app,
        path: ref_path,
        p2: ref_p2,
    } = fixture("undo-equiv-ref");
    let ref_editor = ref_app
        .document_state
        .footprint_editors
        .get_mut(&ref_path)
        .unwrap();
    for _ in 0..2 {
        let (st, primitive) = ref_editor.parts_mut();
        apply_sketch_edit_with_warnings(
            st,
            primitive,
            SketchEdit::MovePoint {
                id: ref_p2,
                dx: 0.5,
                dy: 0.0,
            },
        );
    }
    let moved = |sk: &Option<SketchData>, id| point(sk.as_ref().unwrap(), id);
    assert_eq!(
        moved(&e.primitive().sketch, p2),
        moved(&ref_editor.primitive().sketch, ref_p2),
        "the dragged sketch equals the inline one"
    );

    // Undo walks back through the same steps.
    let editor_mut = app.document_state.footprint_editors.get_mut(&path).unwrap();
    assert!(editor_mut.undo());
    assert!(editor_mut.undo());
    assert!(!editor_mut.undo());
    assert_eq!(editor_mut.primitive().sketch, original);
}

#[test]
fn a_result_after_the_tab_closed_is_ignored() {
    let Fixture { mut app, path, p2 } = fixture("closed");
    dispatch(&mut app, &path, drag(p2, 0.5));
    let answer = solve_now(&app, &path);
    app.document_state.footprint_editors.remove(&path);

    dispatch(&mut app, &path, answer);

    assert!(!app.document_state.footprint_editors.contains_key(&path));
}

#[test]
fn an_undo_while_a_solve_runs_makes_its_result_stale() {
    let Fixture { mut app, path, p2 } = fixture("undo-stale");
    dispatch(&mut app, &path, drag(p2, 0.5));
    let answer = solve_now(&app, &path);
    assert!(
        app.document_state
            .footprint_editors
            .get_mut(&path)
            .unwrap()
            .undo()
    );
    let sketch = editor(&app, &path).primitive().sketch.clone();

    dispatch(&mut app, &path, answer);

    assert_eq!(editor(&app, &path).primitive().sketch, sketch);
    assert!((solved_x(editor(&app, &path), p2) - 8.0).abs() < 1e-6);
}

#[test]
fn a_conflicting_drag_is_rejected_like_the_inline_path_rejects_it() {
    let _ = signex_app::diagnostics::init_logging();
    let Fixture { mut app, path, p2 } = fixture("conflict");
    let good = editor(&app, &path).primitive().sketch.clone();

    dispatch(&mut app, &path, drag(p2, 12.0));
    let answer = solve_now(&app, &path);
    dispatch(&mut app, &path, answer);

    let e = editor(&app, &path);
    assert_eq!(
        e.primitive().sketch,
        good,
        "the sketch is back at its last good state"
    );
    assert_eq!(e.history.len(), 0, "a rejected drag leaves no undo step");
    assert!(!e.dirty, "and does not dirty the document");
    assert!(!e.state.conflict.is_empty(), "the conflict is highlighted");
    assert!(
        e.state
            .solve_warnings
            .iter()
            .any(|w| w.contains("over-constrained")),
        "the Properties strip says why: {:?}",
        e.state.solve_warnings
    );
    assert!(
        signex_app::diagnostics::recent_entries()
            .iter()
            .any(|d| d.message.contains("Sketch edit rejected")),
        "and so does the Messages panel"
    );
    assert!(
        e.state.sketch_solve.is_solve_owed(),
        "the restored sketch is solved again"
    );

    let again = solve_now(&app, &path);
    dispatch(&mut app, &path, again);
    let e = editor(&app, &path);
    assert!(
        e.state.last_solve.is_some(),
        "the canvas shows the good state again"
    );
    assert!(
        e.state
            .solve_warnings
            .iter()
            .any(|w| w.contains("over-constrained")),
        "the re-solve keeps the rejection's warning"
    );
}
