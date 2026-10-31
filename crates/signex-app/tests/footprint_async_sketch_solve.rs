// Standalone binary: reads the process-wide, 200-entry diagnostics ring and installs the logger, which would turn every other test's logging into ring traffic.
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

use signex_app::app::{FootprintDocument, Message, Signex, TabInfo, TabKind};
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

fn editor<'a>(app: &'a Signex, path: &Path) -> &'a FootprintDocument {
    app.document_state.footprint_editors.get(path).unwrap()
}

/// The answer to the sketch as it stands now, as the message a `Task`
/// would deliver.
fn solve_now(app: &Signex, path: &Path) -> FootprintEditorMsg {
    run_solve(snapshot_request(editor(app, path)).expect("the editor has a sketch"))
}

/// Where the last applied solve put `id`.
fn solved_x(e: &FootprintDocument, id: SketchEntityId) -> f64 {
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
    /// Keeps the on-disk file the editor saves to alive.
    _dir: tempfile::TempDir,
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
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join(format!("{stem}.snxfpt"));
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
    let mut state = FootprintDocument::new(path.clone(), FootprintFile::from_footprint(fp));
    let (st, primitive) = state.parts_mut();
    apply_sketch_edit_with_warnings(st, primitive, SketchEdit::ForceRebuild);
    assert!(
        state.state.last_solve.is_some(),
        "the starting sketch solves"
    );
    state.set_dirty(false);
    write_to_disk(&mut state);

    let (mut app, _task) = Signex::new();
    app.document_state
        .footprint_editors
        .insert(path.clone(), state);
    app.document_state.tabs.push(TabInfo {
        title: stem.into(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::FootprintEditor(path.clone()),
    });
    app.document_state.active_tab = 0;
    Fixture {
        _dir: dir,
        app,
        path,
        p2,
    }
}

/// An inline solve, as the fixture's setup made: it advances the editor's
/// generation, so a rebuilt editor can be brought level with the old one.
fn solve_inline(app: &mut Signex, path: &Path) {
    let editor = app.document_state.footprint_editors.get_mut(path).unwrap();
    let (state, primitive) = editor.parts_mut();
    apply_sketch_edit_with_warnings(state, primitive, SketchEdit::ForceRebuild);
}

/// Put the editor's file on disk and make it the saved baseline.
fn write_to_disk(state: &mut FootprintDocument) {
    std::fs::write(&state.path, state.file.to_toml_string().expect("serialise")).expect("write");
    state.baseline = state.file.clone();
}

fn drag(id: SketchEntityId, dx: f64) -> FootprintEditorMsg {
    FootprintEditorMsg::SketchMovePoint { id, dx, dy: 0.0 }
}

#[test]
fn a_drag_tick_moves_the_geometry_at_once_and_owes_a_solve() {
    let Fixture {
        mut app,
        path,
        p2,
        _dir,
    } = fixture("tick-owes");
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
    let Fixture {
        mut app,
        path,
        p2,
        _dir,
    } = fixture("stale");
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
    let Fixture {
        mut app,
        path,
        p2,
        _dir,
    } = fixture("newest");
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
    let Fixture {
        mut app,
        path,
        p2,
        _dir,
    } = fixture("undo-equiv");
    let original = editor(&app, &path).primitive().sketch.clone();
    assert_eq!(editor(&app, &path).history.len(), 0);
    assert!(!editor(&app, &path).is_dirty());

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
    assert!(e.is_dirty());

    // The reference: the inline path — snapshot, edit and solve at once,
    // record the step, set the dirty flag — once per tick.
    let Fixture {
        app: mut ref_app,
        path: ref_path,
        p2: ref_p2,
        _dir: _ref_dir,
    } = fixture("undo-equiv-ref");
    let reference = ref_app
        .document_state
        .footprint_editors
        .get_mut(&ref_path)
        .unwrap();
    for _ in 0..2 {
        reference.push_history();
        let (st, primitive) = reference.parts_mut();
        apply_sketch_edit_with_warnings(
            st,
            primitive,
            SketchEdit::MovePoint {
                id: ref_p2,
                dx: 0.5,
                dy: 0.0,
            },
        );
        reference.set_dirty(true);
    }
    let moved = |sk: &Option<SketchData>, id| point(sk.as_ref().unwrap(), id);
    assert_eq!(e.history.len(), reference.history.len());
    assert_eq!(e.is_dirty(), reference.is_dirty());
    for (got, want) in e.history.iter().zip(&reference.history) {
        assert_eq!(
            moved(&got.file.footprints[0].sketch, p2),
            moved(&want.file.footprints[0].sketch, ref_p2),
            "each undo step restores the same sketch the inline path would"
        );
    }
    assert_eq!(e.redo.len(), reference.redo.len());
    assert_eq!(
        moved(&e.primitive().sketch, p2),
        moved(&reference.primitive().sketch, ref_p2),
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
fn a_result_for_a_closed_editor_never_lands_on_its_reopened_successor() {
    let Fixture {
        mut app,
        path,
        p2,
        _dir,
    } = fixture("closed");
    dispatch(&mut app, &path, drag(p2, 0.5));
    let answer = solve_now(&app, &path);
    // Close the tab, then reopen the same file: a new editor on the same
    // path, which has made exactly as many edits as the answer's sketch
    // had, so only the editor's identity tells the two apart.
    let closed = app.document_state.footprint_editors.remove(&path).unwrap();
    let reopened = FootprintDocument::new(path.clone(), closed.baseline.clone());
    app.document_state
        .footprint_editors
        .insert(path.clone(), reopened);
    solve_inline(&mut app, &path);
    dispatch(&mut app, &path, drag(p2, 0.5));
    app.document_state
        .footprint_editors
        .get_mut(&path)
        .unwrap()
        .state
        .last_solve = None;

    dispatch(&mut app, &path, answer);

    assert!(
        editor(&app, &path).state.last_solve.is_none(),
        "an answer for the closed editor was applied to its successor"
    );
    assert!(editor(&app, &path).state.sketch_solve.is_solve_owed());
}

#[test]
fn an_undo_while_a_solve_runs_makes_its_result_stale() {
    let Fixture {
        mut app,
        path,
        p2,
        _dir,
    } = fixture("undo-stale");
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
    let Fixture {
        mut app,
        path,
        p2,
        _dir,
    } = fixture("conflict");
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
    assert!(!e.is_dirty(), "and does not dirty the document");
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

// ---- hazards: anything that reads or replaces the sketch while a drag solve is owed ----

/// A save writes on a background task (#859): run it, and the messages it
/// answers with, to completion the way the iced runtime would.
fn save(app: &mut Signex, path: &Path) {
    let task = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.to_path_buf(),
        msg: PrimitiveEdit::Save,
    }));
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut pending = vec![task];
    while let Some(task) = pending.pop() {
        let Some(mut stream) = iced_runtime::task::into_stream(task) else {
            continue;
        };
        let mut messages = Vec::new();
        runtime.block_on(async {
            while let Some(action) = futures::StreamExt::next(&mut stream).await {
                if let iced_runtime::Action::Output(next) = action {
                    messages.push(next);
                }
            }
        });
        pending.extend(messages.into_iter().map(|m| app.update(m)));
    }
}

fn saved_p2(path: &Path, p2: SketchEntityId) -> (f64, f64) {
    let file =
        FootprintFile::from_toml_str(&std::fs::read_to_string(path).expect("read")).expect("parse");
    point(file.footprints[0].sketch.as_ref().unwrap(), p2)
}

#[test]
fn saving_while_a_rejected_drag_is_pending_writes_the_good_sketch() {
    let _ = signex_app::diagnostics::init_logging();
    let Fixture {
        mut app,
        path,
        p2,
        _dir,
    } = fixture("save-rejected");
    // The footprint is clean; the drag over-constrains it.
    dispatch(&mut app, &path, drag(p2, 12.0));
    let stale = solve_now(&app, &path);
    // Make the save write something, whatever it settles to.
    app.document_state
        .footprint_editors
        .get_mut(&path)
        .unwrap()
        .file
        .footprints[0]
        .name
        .push_str("-edited");

    save(&mut app, &path);

    assert_eq!(
        saved_p2(&path, p2),
        (8.0, 0.0),
        "the save wrote the unsolved, over-constraining sketch"
    );
    dispatch(&mut app, &path, stale);
    let e = editor(&app, &path);
    assert_eq!(
        point(e.primitive().sketch.as_ref().unwrap(), p2),
        (8.0, 0.0)
    );
    assert!(!e.is_dirty(), "the saved document is clean and stays so");
    assert!(!app.document_state.is_document_dirty(&path));
}

#[test]
fn saving_while_a_drag_solve_is_pending_writes_the_solved_geometry() {
    let (mut app, path, centre, dir) = chamfered_pad_editor("save-pending");
    dispatch(
        &mut app,
        &path,
        FootprintEditorMsg::SketchMovePoint {
            id: centre,
            dx: 5.0,
            dy: 3.0,
        },
    );
    let stale = solve_now(&app, &path);

    save(&mut app, &path);

    let file = FootprintFile::from_toml_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let live = editor(&app, &path);
    let anchor = |sketch: &SketchData, e: &FootprintDocument| {
        sidecar_point(sketch, &e.state.pads[0], "chamfer_ne_anchor1")
    };
    let saved_anchor = anchor(file.footprints[0].sketch.as_ref().unwrap(), live);
    assert!(
        (saved_anchor.0 - 5.75).abs() < 1e-9 && (saved_anchor.1 - 2.5).abs() < 1e-9,
        "the saved chamfer anchor must ride the drag to (5.75, 2.5), got {saved_anchor:?}"
    );
    assert!(!live.is_dirty());
    // The answer for the pre-save sketch must not touch the saved document.
    dispatch(&mut app, &path, stale);
    assert!(
        !editor(&app, &path).is_dirty(),
        "a stale answer must not dirty it"
    );
    drop(dir);
}

#[test]
fn an_edit_between_a_rejected_drag_and_its_result_keeps_its_own_undo_step() {
    let Fixture {
        mut app,
        path,
        p2,
        _dir,
    } = fixture("interleaved");
    dispatch(&mut app, &path, drag(p2, 12.0));
    let stale = solve_now(&app, &path);

    dispatch(
        &mut app,
        &path,
        FootprintEditorMsg::SetName("renamed".into()),
    );
    dispatch(&mut app, &path, stale);

    let e = editor(&app, &path);
    assert_eq!(e.primitive().name, "renamed");
    assert!(
        e.is_dirty(),
        "the rename is unsaved, so the document is dirty"
    );
    assert_eq!(e.history.len(), 1, "exactly the rename's step is left");
    assert_eq!(
        e.history[0].file.footprints[0].name, "interleaved",
        "and it is the step that undoes the rename"
    );
    assert_eq!(
        point(e.primitive().sketch.as_ref().unwrap(), p2),
        (8.0, 0.0)
    );
}

#[test]
fn a_rejected_drag_gives_back_the_redo_stack_and_the_evicted_history() {
    let Fixture {
        mut app,
        path,
        p2,
        _dir,
    } = fixture("redo-evict");
    {
        let e = app.document_state.footprint_editors.get_mut(&path).unwrap();
        for i in 0..FootprintDocument::HISTORY_DEPTH {
            e.file.footprints[0].name = format!("n{i}");
            e.push_history();
        }
        // A full history, and one undone step waiting to be redone.
        assert_eq!(e.history.len(), FootprintDocument::HISTORY_DEPTH);
        let mut undone = e.history[0].clone();
        undone.file.footprints[0].name = "undone".into();
        e.redo.push(undone);
        e.set_dirty(false);
    }
    let (first, redo_name) = {
        let e = editor(&app, &path);
        (
            e.history[0].file.footprints[0].name.clone(),
            e.redo[0].file.footprints[0].name.clone(),
        )
    };

    dispatch(&mut app, &path, drag(p2, 12.0));
    assert!(editor(&app, &path).redo.is_empty(), "the tick cleared redo");
    let answer = solve_now(&app, &path);
    dispatch(&mut app, &path, answer);

    let e = editor(&app, &path);
    assert_eq!(e.history.len(), FootprintDocument::HISTORY_DEPTH);
    assert_eq!(
        e.history[0].file.footprints[0].name, first,
        "the evicted step is back"
    );
    assert_eq!(e.redo.len(), 1, "the redo step is back");
    assert_eq!(e.redo[0].file.footprints[0].name, redo_name);
}

#[test]
fn a_rejected_drag_leaves_the_document_clean_for_every_reader() {
    let Fixture {
        mut app,
        path,
        p2,
        _dir,
    } = fixture("dirty-mirror");
    dispatch(&mut app, &path, drag(p2, 12.0));

    let answer = solve_now(&app, &path);
    dispatch(&mut app, &path, answer);

    assert!(!editor(&app, &path).is_dirty());
    assert!(!app.document_state.is_document_dirty(&path));
    assert!(!app.document_state.has_dirty_documents());
}

#[test]
fn a_stale_answer_cannot_land_after_switching_away_and_back() {
    let Fixture {
        mut app,
        path,
        p2,
        _dir,
    } = fixture("switch");
    app.document_state
        .footprint_editors
        .get_mut(&path)
        .unwrap()
        .file
        .footprints
        .push(Footprint::empty("other"));
    dispatch(&mut app, &path, drag(p2, 0.5));
    let stale = solve_now(&app, &path);

    dispatch(&mut app, &path, FootprintEditorMsg::SelectActiveIdx(1));
    dispatch(&mut app, &path, FootprintEditorMsg::SelectActiveIdx(0));
    // The rebuilt state, brought level with the old one and given one tick:
    // as many edits as the stale answer's sketch had.
    solve_inline(&mut app, &path);
    dispatch(&mut app, &path, drag(p2, 2.0));
    app.document_state
        .footprint_editors
        .get_mut(&path)
        .unwrap()
        .state
        .last_solve = None;

    dispatch(&mut app, &path, stale);

    let e = editor(&app, &path);
    assert!(
        e.state.last_solve.is_none(),
        "an answer for the state before the switch was applied to the one after it"
    );
    assert!(e.state.sketch_solve.is_solve_owed());
    assert_eq!(
        point(e.primitive().sketch.as_ref().unwrap(), p2),
        (10.5, 0.0)
    );
}

#[test]
fn switching_footprint_finishes_the_owed_solve_first() {
    let Fixture {
        mut app,
        path,
        p2,
        _dir,
    } = fixture("switch-owed");
    app.document_state
        .footprint_editors
        .get_mut(&path)
        .unwrap()
        .file
        .footprints
        .push(Footprint::empty("other"));
    // A rejected drag: if the switch dropped the owed solve, the
    // over-constraining sketch would stay in the file.
    dispatch(&mut app, &path, drag(p2, 12.0));

    dispatch(&mut app, &path, FootprintEditorMsg::SelectActiveIdx(1));

    let e = editor(&app, &path);
    assert_eq!(
        point(e.file.footprints[0].sketch.as_ref().unwrap(), p2),
        (8.0, 0.0),
        "the rejection was applied before the editor moved on"
    );
}

/// A Chamfered 2x1 pad minted into a footprint file on disk.
fn chamfered_pad_editor(stem: &str) -> (Signex, PathBuf, SketchEntityId, tempfile::TempDir) {
    use signex_app::library::editor::footprint::pad_to_sketch::mirror_add_pad_to_sketch;
    use signex_app::library::editor::footprint::state::EditorPad;
    use signex_library::PadShape;
    use signex_library::primitive::footprint::ChamferedCorners;

    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join(format!("{stem}.snxfpt"));
    let mut pad = EditorPad::new_default("1".into(), (0.0, 0.0));
    pad.size_mm = (2.0, 1.0);
    pad.shape = PadShape::Chamfered {
        chamfer_ratio: 0.25,
        corners: ChamferedCorners {
            top_right: true,
            ..Default::default()
        },
    };
    let mut fp = Footprint::empty(stem);
    mirror_add_pad_to_sketch(&mut pad, &mut fp);
    let centre = pad.sketch_entity_id.expect("the pad minted a centre point");
    let mut state = FootprintDocument::new(path.clone(), FootprintFile::from_footprint(fp));
    state.state.pads = vec![pad];
    state.state.selected_pad = Some(0);
    write_to_disk(&mut state);
    install(state, stem, path, dir, centre)
}

fn install(
    state: FootprintDocument,
    stem: &str,
    path: PathBuf,
    dir: tempfile::TempDir,
    id: SketchEntityId,
) -> (Signex, PathBuf, SketchEntityId, tempfile::TempDir) {
    let (mut app, _task) = Signex::new();
    app.document_state
        .footprint_editors
        .insert(path.clone(), state);
    app.document_state.tabs.push(TabInfo {
        title: stem.into(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::FootprintEditor(path.clone()),
    });
    app.document_state.active_tab = 0;
    (app, path, id, dir)
}

fn sidecar_point(
    sketch: &SketchData,
    pad: &signex_app::library::editor::footprint::state::EditorPad,
    key: &str,
) -> (f64, f64) {
    let raw = pad.shape_params.get(key).expect("sidecar binding");
    let id = SketchEntityId(uuid::Uuid::parse_str(raw).expect("uuid slug"));
    point(sketch, id)
}

trait CloneForCompare {
    fn clone_for_compare(
        &self,
    ) -> (
        Option<SketchData>,
        Vec<signex_app::library::editor::footprint::state::EditorPad>,
    );
}

impl CloneForCompare for FootprintDocument {
    fn clone_for_compare(
        &self,
    ) -> (
        Option<SketchData>,
        Vec<signex_app::library::editor::footprint::state::EditorPad>,
    ) {
        (self.primitive().sketch.clone(), self.state.pads.clone())
    }
}

/// Deliver every owed solve, the way the runtime would.
fn settle_solves(app: &mut Signex, path: &Path) {
    for _ in 0..4 {
        let e = editor(app, path);
        if !e.state.sketch_solve.is_solve_owed() {
            return;
        }
        let msg = solve_now(app, path);
        dispatch(app, path, msg);
    }
}

/// The reference values below were captured by running the same three
/// ticks through the inline path on trunk before this change: an inward
/// pull of a constrained Rect pad's corner is undone (the pad frame owns
/// the outline), and an outward pull resizes the pad. Dragging async must
/// end every tick in the same place.
#[test]
fn a_corner_drag_on_a_constrained_rect_pad_ends_where_the_inline_path_did() {
    use signex_app::library::editor::footprint::pad_to_sketch::mirror_add_pad_to_sketch;
    use signex_app::library::editor::footprint::state::EditorPad;

    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("corner.snxfpt");
    let mut pad = EditorPad::new_default("1".into(), (0.0, 0.0));
    pad.size_mm = (2.0, 1.0);
    let mut fp = Footprint::empty("corner");
    mirror_add_pad_to_sketch(&mut pad, &mut fp);
    let ne = pad.corner_entity_ids.expect("a pad mints bbox corners")[0];
    let mut state = FootprintDocument::new(path.clone(), FootprintFile::from_footprint(fp));
    state.state.pads = vec![pad];
    let (mut app, path, _, _dir) = install(state, "corner", path, dir, ne);

    type Pt = (f64, f64);
    let ticks: [(Pt, Pt, Pt, Pt); 3] = [
        // (drag), (NE corner), (pad size), (pad centre)
        ((-0.25, 0.125), (1.0, -0.5), (2.0, 1.0), (0.0, 0.0)),
        ((-0.25, 0.125), (1.0, -0.5), (2.0, 1.0), (0.0, 0.0)),
        ((0.0, -0.5), (1.0, -1.0), (2.0, 1.5), (0.0, -0.25)),
    ];
    for ((dx, dy), want_ne, want_size, want_centre) in ticks {
        dispatch(
            &mut app,
            &path,
            FootprintEditorMsg::SketchMovePoint { id: ne, dx, dy },
        );
        settle_solves(&mut app, &path);

        let e = editor(&app, &path);
        let close =
            |a: (f64, f64), b: (f64, f64)| (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9;
        let pad = &e.state.pads[0];
        let baked = &e.primitive().pads[0];
        let got_ne = point(e.primitive().sketch.as_ref().unwrap(), ne);
        assert!(
            close(got_ne, want_ne),
            "tick ({dx}, {dy}): NE corner {got_ne:?}"
        );
        assert!(
            close(pad.size_mm, want_size),
            "tick ({dx}, {dy}): size {:?}",
            pad.size_mm
        );
        assert!(
            close(pad.position_mm, want_centre),
            "tick ({dx}, {dy}): centre {:?}",
            pad.position_mm
        );
        assert!(
            close((baked.size[0], baked.size[1]), want_size)
                && close((baked.position[0], baked.position[1]), want_centre),
            "tick ({dx}, {dy}): the baked pad is {:?} at {:?}",
            baked.size,
            baked.position
        );
    }
}

// ---- undo/redo into an unsolved drag state, and a drag longer than the history ----

fn centre_tick(centre: SketchEntityId) -> FootprintEditorMsg {
    FootprintEditorMsg::SketchMovePoint {
        id: centre,
        dx: 1.0,
        dy: 0.0,
    }
}

/// The saved file's pad-centre point and chamfer anchor.
fn saved_centre_and_anchor(
    path: &Path,
    live: &FootprintDocument,
    centre: SketchEntityId,
) -> ((f64, f64), (f64, f64)) {
    let file = FootprintFile::from_toml_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let sketch = file.footprints[0].sketch.as_ref().unwrap();
    (
        point(sketch, centre),
        sidecar_point(sketch, &live.state.pads[0], "chamfer_ne_anchor1"),
    )
}

/// Three centre ticks of +1 mm with a solve delivered after the first and
/// the third: the second tick's undo step is an unsolved state.
fn three_ticks_with_two_solves(app: &mut Signex, path: &Path, centre: SketchEntityId) {
    dispatch(app, path, centre_tick(centre));
    let first = solve_now(app, path);
    dispatch(app, path, first);
    dispatch(app, path, centre_tick(centre));
    dispatch(app, path, centre_tick(centre));
    let last = solve_now(app, path);
    dispatch(app, path, last);
}

#[test]
fn undo_after_a_drag_batch_restores_the_last_solved_state_and_saves_it() {
    let (mut app, path, centre, _dir) = chamfered_pad_editor("undo-save");
    three_ticks_with_two_solves(&mut app, &path, centre);
    // Tick 1 was solved alone; ticks 2 and 3 were one batch: two steps.
    assert_eq!(editor(&app, &path).history.len(), 2);

    assert!(
        app.document_state
            .footprint_editors
            .get_mut(&path)
            .unwrap()
            .undo()
    );
    save(&mut app, &path);

    let (saved_centre, saved_anchor) = saved_centre_and_anchor(&path, editor(&app, &path), centre);
    assert_eq!(saved_centre, (1.0, 0.0), "back to the state after tick 1");
    assert!(
        (saved_anchor.0 - 1.75).abs() < 1e-9 && (saved_anchor.1 + 0.5).abs() < 1e-9,
        "the chamfer anchor is the solved one for that centre, got {saved_anchor:?}"
    );
}

#[test]
fn redo_after_a_drag_batch_lands_on_the_solved_end_state() {
    let (mut app, path, centre, _dir) = chamfered_pad_editor("redo-save");
    three_ticks_with_two_solves(&mut app, &path, centre);
    {
        let e = app.document_state.footprint_editors.get_mut(&path).unwrap();
        assert!(e.undo());
        assert!(e.undo());
        assert!(!e.undo(), "two steps: the single tick and the batch");
        assert!(e.redo());
    }
    save(&mut app, &path);

    let (saved_centre, saved_anchor) = saved_centre_and_anchor(&path, editor(&app, &path), centre);
    assert_eq!(saved_centre, (1.0, 0.0));
    assert!((saved_anchor.0 - 1.75).abs() < 1e-9, "{saved_anchor:?}");
}

#[test]
fn a_long_drag_is_one_undo_step() {
    let Fixture {
        mut app,
        path,
        p2,
        _dir,
    } = fixture("long-collapse");
    for i in 0..3 {
        let e = app.document_state.footprint_editors.get_mut(&path).unwrap();
        e.file.footprints[0].name = format!("pre{i}");
        e.push_history();
    }
    for _ in 0..30 {
        dispatch(&mut app, &path, drag(p2, 0.01));
    }
    let answer = solve_now(&app, &path);
    dispatch(&mut app, &path, answer);

    let e = editor(&app, &path);
    assert_eq!(
        e.history.len(),
        4,
        "three earlier steps and one for the drag"
    );
    assert!(e.is_dirty());
    let drag_step = &e.history[3];
    assert_eq!(
        point(drag_step.file.footprints[0].sketch.as_ref().unwrap(), p2),
        (8.0, 0.0),
        "the drag's one step restores the pre-drag sketch"
    );
}

#[test]
fn a_drag_through_a_conflict_and_back_undoes_to_the_pre_drag_state() {
    let Fixture {
        mut app,
        path,
        p2,
        _dir,
    } = fixture("conflict-and-back");
    // +12 mm puts P3 out of reach (unsolved, over-constrained); -11.5 mm
    // brings it back. Only the final answer lands.
    dispatch(&mut app, &path, drag(p2, 12.0));
    dispatch(&mut app, &path, drag(p2, -11.5));
    let answer = solve_now(&app, &path);
    dispatch(&mut app, &path, answer);
    assert_eq!(
        editor(&app, &path).history.len(),
        1,
        "one step for the drag"
    );

    {
        let e = app.document_state.footprint_editors.get_mut(&path).unwrap();
        assert!(e.undo());
    }
    save(&mut app, &path);

    assert_eq!(
        saved_p2(&path, p2),
        (8.0, 0.0),
        "the saved sketch is the pre-drag one"
    );
    assert_eq!(
        point(editor(&app, &path).primitive().sketch.as_ref().unwrap(), p2),
        (8.0, 0.0)
    );

    // And forward again: redo lands on the solved end state, not on a
    // half-dragged one.
    {
        let e = app.document_state.footprint_editors.get_mut(&path).unwrap();
        assert!(e.redo());
    }
    save(&mut app, &path);
    assert_eq!(saved_p2(&path, p2), (8.5, 0.0));
}

#[test]
fn undo_after_a_collapsed_drag_restores_the_pre_drag_sketch_and_pads() {
    let (mut app, path, centre, _dir) = chamfered_pad_editor("collapse-undo");
    let before = editor(&app, &path).clone_for_compare();
    for _ in 0..5 {
        dispatch(&mut app, &path, centre_tick(centre));
    }
    let answer = solve_now(&app, &path);
    dispatch(&mut app, &path, answer);
    assert_eq!(editor(&app, &path).history.len(), 1);

    assert!(
        app.document_state
            .footprint_editors
            .get_mut(&path)
            .unwrap()
            .undo()
    );

    let e = editor(&app, &path);
    assert_eq!(e.primitive().sketch, before.0, "the pre-drag sketch");
    assert_eq!(e.state.pads, before.1, "the pre-drag pads");
}

#[test]
fn a_rejected_drag_longer_than_the_history_restores_the_pre_drag_history() {
    let Fixture {
        mut app,
        path,
        p2,
        _dir,
    } = fixture("long-drag");
    const PRE: usize = 5;
    {
        let e = app.document_state.footprint_editors.get_mut(&path).unwrap();
        for i in 0..PRE {
            e.file.footprints[0].name = format!("pre{i}");
            e.push_history();
        }
        let mut undone = e.history[0].clone();
        undone.file.footprints[0].name = "undone".into();
        e.redo.push(undone);
        e.set_dirty(false);
    }
    let before: Vec<String> = editor(&app, &path)
        .history
        .iter()
        .map(|s| s.file.footprints[0].name.clone())
        .collect();

    // 120 ticks of 0.1 mm walk p2 from 8 to 20 mm: out of P3's reach.
    for _ in 0..120 {
        dispatch(&mut app, &path, drag(p2, 0.1));
    }
    assert_eq!(
        editor(&app, &path).history.len(),
        FootprintDocument::HISTORY_DEPTH,
        "the ticks filled the history"
    );
    let answer = solve_now(&app, &path);
    dispatch(&mut app, &path, answer);

    let e = editor(&app, &path);
    let after: Vec<String> = e
        .history
        .iter()
        .map(|s| s.file.footprints[0].name.clone())
        .collect();
    assert_eq!(after, before, "the history is the one from before the drag");
    assert_eq!(
        point(e.primitive().sketch.as_ref().unwrap(), p2),
        (8.0, 0.0)
    );
    assert!(
        e.history
            .iter()
            .all(|s| point(s.file.footprints[0].sketch.as_ref().unwrap(), p2) == (8.0, 0.0)),
        "no undo step holds the rejected geometry"
    );
    assert_eq!(e.redo.len(), 1, "the redo step is back");
    assert!(!e.is_dirty());
}

#[test]
fn a_rejection_settled_by_a_footprint_switch_leaves_the_document_clean() {
    let Fixture {
        mut app,
        path,
        p2,
        _dir,
    } = fixture("switch-mirror");
    app.document_state
        .footprint_editors
        .get_mut(&path)
        .unwrap()
        .file
        .footprints
        .push(Footprint::empty("other"));
    dispatch(&mut app, &path, drag(p2, 12.0));

    dispatch(&mut app, &path, FootprintEditorMsg::SelectActiveIdx(1));

    assert!(!editor(&app, &path).is_dirty());
    assert!(!app.document_state.is_document_dirty(&path));
    assert!(!app.document_state.has_dirty_documents());
}

// ---- ticks that change nothing, inside a drag batch ----

/// Add a Line between the fixture's P1 and P3, as the drag-a-line handler
/// needs one, and return its id.
fn add_line(app: &mut Signex, path: &Path) -> SketchEntityId {
    let e = app.document_state.footprint_editors.get_mut(path).unwrap();
    let sketch = e.primitive_mut().sketch.as_mut().unwrap();
    let (p1, p3) = (sketch.entities[0].id, sketch.entities[2].id);
    let plane = sketch.planes[0].id;
    let id = SketchEntityId::new();
    sketch.entities.push(Entity::new(
        id,
        plane,
        EntityKind::Line { start: p1, end: p3 },
    ));
    e.set_dirty(false);
    id
}

fn no_motion(line: SketchEntityId) -> FootprintEditorMsg {
    FootprintEditorMsg::SketchMoveLine {
        id: line,
        dx: 0.0,
        dy: 0.0,
    }
}

/// A drag through a conflict and back, with `idle_ticks` zero-motion line
/// ticks placed after `after` real ones: +12 mm, -11.5 mm.
fn drag_with_idle_ticks(stem: &str, idle_between: usize, idle_last: usize) {
    let Fixture {
        mut app,
        path,
        p2,
        _dir,
    } = fixture(stem);
    let line = add_line(&mut app, &path);

    dispatch(&mut app, &path, drag(p2, 12.0));
    for _ in 0..idle_between {
        dispatch(&mut app, &path, no_motion(line));
    }
    dispatch(&mut app, &path, drag(p2, -11.5));
    for _ in 0..idle_last {
        dispatch(&mut app, &path, no_motion(line));
    }
    let answer = solve_now(&app, &path);
    dispatch(&mut app, &path, answer);

    assert_eq!(
        editor(&app, &path).history.len(),
        1,
        "the drag is one undo step however many idle ticks it had"
    );
    assert!(
        app.document_state
            .footprint_editors
            .get_mut(&path)
            .unwrap()
            .undo()
    );
    save(&mut app, &path);
    assert_eq!(
        saved_p2(&path, p2),
        (8.0, 0.0),
        "the saved sketch is the pre-drag solved one"
    );
    assert_eq!(
        point(editor(&app, &path).primitive().sketch.as_ref().unwrap(), p2),
        (8.0, 0.0)
    );
}

#[test]
fn a_zero_motion_tick_last_in_a_drag_still_collapses_to_one_step() {
    drag_with_idle_ticks("idle-last", 0, 1);
}

#[test]
fn zero_motion_ticks_mid_drag_still_collapse_to_one_step() {
    drag_with_idle_ticks("idle-mid", 2, 0);
}

#[test]
fn a_drag_of_only_zero_motion_ticks_records_nothing() {
    let Fixture {
        mut app,
        path,
        _dir,
        ..
    } = fixture("idle-only");
    let line = add_line(&mut app, &path);
    app.document_state.mark_document_clean(&path);

    for _ in 0..3 {
        dispatch(&mut app, &path, no_motion(line));
    }

    let e = editor(&app, &path);
    assert_eq!(
        e.history.len(),
        0,
        "no undo step for a drag that moved nothing"
    );
    assert!(!e.is_dirty(), "and the document stays clean");
    assert!(
        !e.state.sketch_solve.is_solve_owed(),
        "and nothing is solved"
    );
}

#[test]
fn a_drag_longer_than_the_history_collapses_without_a_warning() {
    let _ = signex_app::diagnostics::init_logging();
    let Fixture {
        mut app,
        path,
        p2,
        _dir,
    } = fixture("overflow-drag");
    const PRE: usize = 5;
    for i in 0..PRE {
        let e = app.document_state.footprint_editors.get_mut(&path).unwrap();
        e.file.footprints[0].name = format!("pre{i}");
        e.push_history();
    }
    let before: Vec<String> = editor(&app, &path)
        .history
        .iter()
        .map(|s| s.file.footprints[0].name.clone())
        .collect();

    // 115 ticks: enough to push every earlier step, and the drag's own
    // first step, off the bottom of the 100-step history.
    for _ in 0..115 {
        dispatch(&mut app, &path, drag(p2, 0.01));
    }
    let answer = solve_now(&app, &path);
    dispatch(&mut app, &path, answer);

    let e = editor(&app, &path);
    let names: Vec<String> = e
        .history
        .iter()
        .map(|s| s.file.footprints[0].name.clone())
        .collect();
    assert_eq!(
        names.len(),
        PRE + 1,
        "the earlier steps and one for the drag"
    );
    assert_eq!(names[..PRE], before[..], "the earlier steps are back");
    assert_eq!(
        point(
            e.history[PRE].file.footprints[0].sketch.as_ref().unwrap(),
            p2
        ),
        (8.0, 0.0),
        "the drag's step restores the pre-drag sketch"
    );
    assert!(
        !signex_app::diagnostics::recent_entries()
            .iter()
            .any(|d| d.message.contains("undo history changed while its solve")),
        "an overflowing drag is not an anomaly"
    );
}

// ---- a mode switch is an inline solve: it settles the owed drag solve first ----

fn set_mode_normal() -> FootprintEditorMsg {
    FootprintEditorMsg::SetMode(signex_app::library::editor::footprint::state::EditorMode::Normal)
}

#[test]
fn a_mode_switch_during_an_accepted_drag_keeps_no_unsolved_step() {
    let (mut app, path, centre, _dir) = chamfered_pad_editor("mode-accepted");
    dispatch(&mut app, &path, centre_tick(centre));
    dispatch(&mut app, &path, centre_tick(centre));
    // The drag's solve has not landed when the mode key is pressed.
    dispatch(&mut app, &path, set_mode_normal());

    let e = editor(&app, &path);
    assert_eq!(e.history.len(), 1, "the drag is one undo step");
    assert_eq!(
        point(
            e.history[0].file.footprints[0].sketch.as_ref().unwrap(),
            centre
        ),
        (0.0, 0.0),
        "and that step is the solved pre-drag state, not a half-dragged one"
    );
    let live = sidecar_point(
        e.primitive().sketch.as_ref().unwrap(),
        &e.state.pads[0],
        "chamfer_ne_anchor1",
    );
    assert!(
        (live.0 - 2.75).abs() < 1e-9,
        "the live pads follow the dragged sketch, got {live:?}"
    );

    assert!(
        app.document_state
            .footprint_editors
            .get_mut(&path)
            .unwrap()
            .undo()
    );
    save(&mut app, &path);

    let (saved_centre, saved_anchor) = saved_centre_and_anchor(&path, editor(&app, &path), centre);
    assert_eq!(saved_centre, (0.0, 0.0));
    assert!(
        (saved_anchor.0 - 0.75).abs() < 1e-9 && (saved_anchor.1 + 0.5).abs() < 1e-9,
        "saved pads match the saved sketch, got {saved_anchor:?}"
    );
}

#[test]
fn a_mode_switch_during_a_conflicting_drag_rolls_the_drag_back() {
    let _ = signex_app::diagnostics::init_logging();
    let Fixture {
        mut app,
        path,
        p2,
        _dir,
    } = fixture("mode-conflict");
    {
        // One undone edit leaves a redo step for the drag to park and give back.
        let e = app.document_state.footprint_editors.get_mut(&path).unwrap();
        e.file.footprints[0].name.push_str("-edited");
        e.push_history();
        assert!(e.undo());
        assert_eq!(e.redo.len(), 1);
        e.set_dirty(false);
    }
    let good = editor(&app, &path).primitive().sketch.clone();
    let history_before = editor(&app, &path).history.len();

    dispatch(&mut app, &path, drag(p2, 12.0));
    dispatch(&mut app, &path, set_mode_normal());

    let e = editor(&app, &path);
    assert_eq!(
        e.primitive().sketch,
        good,
        "the sketch is back at its last good state"
    );
    assert_eq!(
        e.history.len(),
        history_before,
        "the drag left no undo step"
    );
    assert_eq!(e.redo.len(), 1, "the redo stack was given back");

    save(&mut app, &path);
    assert_eq!(
        saved_p2(&path, p2),
        (8.0, 0.0),
        "Save did not persist the conflict"
    );
}

#[test]
fn an_answer_addressed_to_a_rekeyed_editors_old_path_still_frees_its_slot() {
    let Fixture {
        mut app,
        path,
        p2,
        _dir,
    } = fixture("rekeyed");
    dispatch(&mut app, &path, drag(p2, 0.5));
    let answer = solve_now(&app, &path);
    assert!(editor(&app, &path).state.sketch_solve.is_in_flight());

    // Save As / a project-tree rename re-key the live editor; the running
    // task still carries the old path.
    let new_path = path.with_file_name("rekeyed-renamed.snxfpt");
    let mut moved = app.document_state.footprint_editors.remove(&path).unwrap();
    moved.path = new_path.clone();
    app.document_state
        .footprint_editors
        .insert(new_path.clone(), moved);

    dispatch(&mut app, &path, answer);

    let e = editor(&app, &new_path);
    assert!(
        !e.state.sketch_solve.is_in_flight(),
        "the answer found its editor by instance and freed the slot"
    );
    assert!(
        (solved_x(e, p2) - 8.5).abs() < 1e-6,
        "and it was applied to the re-keyed editor"
    );

    dispatch(&mut app, &new_path, drag(p2, 0.5));
    assert!(
        editor(&app, &new_path).state.sketch_solve.is_in_flight(),
        "the next drag starts a new solve"
    );
    let next = solve_now(&app, &new_path);
    dispatch(&mut app, &new_path, next);
    assert!((solved_x(editor(&app, &new_path), p2) - 9.0).abs() < 1e-6);
}

#[test]
fn cursor_tracking_during_a_drag_neither_solves_nor_settles() {
    let Fixture {
        mut app,
        path,
        p2,
        _dir,
    } = fixture("cursor-exempt");
    dispatch(&mut app, &path, drag(p2, 0.5));
    let solved_before = editor(&app, &path).state.last_solve.clone();

    dispatch(
        &mut app,
        &path,
        FootprintEditorMsg::CursorAt {
            x_mm: 1.0,
            y_mm: 2.0,
            px_per_mm: 30.0,
        },
    );

    let e = editor(&app, &path);
    assert!(
        e.state.sketch_solve.is_solve_owed(),
        "the solve is still owed"
    );
    assert_eq!(
        e.state.last_solve.as_ref().map(|s| s.jacobian.len()),
        solved_before.as_ref().map(|s| s.jacobian.len()),
        "no inline solve ran"
    );
    assert_eq!(
        point(e.primitive().sketch.as_ref().unwrap(), p2),
        (8.5, 0.0),
        "the drag is still unsolved geometry, not settled away"
    );
}

#[test]
fn an_exhausted_work_budget_in_the_async_solve_is_reported_and_keeps_the_edit() {
    let _ = signex_app::diagnostics::init_logging();
    let Fixture {
        mut app,
        path,
        p2,
        _dir,
    } = fixture("budget");
    // The solver clone the task runs on carries the same work budget.
    app.document_state
        .footprint_editors
        .get_mut(&path)
        .unwrap()
        .state
        .sketch_solver
        .work_budget = 1;

    dispatch(&mut app, &path, drag(p2, 0.5));
    let answer = solve_now(&app, &path);
    dispatch(&mut app, &path, answer);

    let e = editor(&app, &path);
    assert!(!e.state.sketch_solve.is_in_flight(), "the slot is free");
    assert!(
        !e.state.solve_warnings.is_empty(),
        "the failure reaches the Properties strip"
    );
    assert!(e.state.last_solve.is_none(), "no stale solve is shown");
    assert_eq!(
        point(e.primitive().sketch.as_ref().unwrap(), p2),
        (8.5, 0.0),
        "a budget failure is not a conflict: the edit is kept, as inline"
    );
}
