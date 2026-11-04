//! #1685 — the footprint sketch's redundancy pass runs off the UI thread,
//! started from the editor-event dispatch. The runtime's `Task` is replaced
//! by running the pass inline and feeding its answer back through
//! `Signex::update`, so the dispatch wiring is what is under test.

#![expect(
    clippy::let_underscore_must_use,
    reason = "integration tests discard the `Task` update returns; this is test scaffolding, and `[lints]` in Cargo.toml is package-scoped so each integration-test crate root needs its own"
)]

use signex_app::app::{FootprintDocument, Message, Signex, TabInfo, TabKind};
use signex_app::library::editor::footprint::sketch_redundancy_task::{
    run_redundancy, snapshot_redundancy_request,
};
use signex_app::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
use signex_library::{Footprint, FootprintFile};
use signex_sketch::SketchData;
use signex_sketch::constraint::{Constraint, ConstraintKind};
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

/// An app whose open footprint states `Horizontal` on one line twice; the
/// second statement is the redundant one, whose id is returned.
fn app_with_a_redundant_constraint() -> (Signex, PathBuf, ConstraintId) {
    let path = PathBuf::from("redundant.snxfpt");
    let plane = PlaneId::new();
    let (a, b, line) = (
        SketchEntityId::new(),
        SketchEntityId::new(),
        SketchEntityId::new(),
    );
    let horizontal = || Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Horizontal { line },
    };
    let (first, second) = (horizontal(), horizontal());
    let redundant = second.id;
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
    let state = FootprintDocument::new(path.clone(), FootprintFile::from_footprint(fp));
    let (mut app, _task) = Signex::new();
    app.document_state
        .footprint_editors
        .insert(path.clone(), state);
    app.document_state.tabs.push(TabInfo {
        title: "redundant".into(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::FootprintEditor(path.clone()),
    });
    app.document_state.active_tab = 0;
    (app, path, redundant)
}

/// An edit that is not a drag tick, so `update` solves it inline.
fn inline_edit() -> FootprintEditorMsg {
    FootprintEditorMsg::SketchPlacePoint {
        x_mm: 9.0,
        y_mm: 9.0,
    }
}

fn answer_now(app: &Signex, path: &Path) -> FootprintEditorMsg {
    run_redundancy(snapshot_redundancy_request(editor(app, path)).expect("a solved sketch"))
}

#[test]
fn an_edit_starts_the_pass_and_its_answer_marks_the_redundant_constraint() {
    let (mut app, path, redundant) = app_with_a_redundant_constraint();

    dispatch(&mut app, &path, inline_edit());

    let e = editor(&app, &path);
    assert!(
        e.state.sketch_solve.is_redundancy_running(),
        "the editor event started the pass"
    );
    assert!(
        e.state
            .last_solve
            .as_ref()
            .unwrap()
            .redundant_constraints
            .is_empty()
    );

    let answer = answer_now(&app, &path);
    dispatch(&mut app, &path, answer);

    let e = editor(&app, &path);
    assert_eq!(
        e.state.last_solve.as_ref().unwrap().redundant_constraints,
        vec![redundant]
    );
    assert!(!e.state.sketch_solve.is_redundancy_running());
}

#[test]
fn a_stale_answer_starts_the_pass_for_the_newer_sketch() {
    let (mut app, path, redundant) = app_with_a_redundant_constraint();
    dispatch(&mut app, &path, inline_edit());
    let stale = answer_now(&app, &path);

    dispatch(&mut app, &path, inline_edit());
    dispatch(&mut app, &path, stale);

    let e = editor(&app, &path);
    assert!(
        e.state
            .last_solve
            .as_ref()
            .unwrap()
            .redundant_constraints
            .is_empty(),
        "the stale answer is not applied"
    );
    assert!(
        e.state.sketch_solve.is_redundancy_running(),
        "landing the stale answer started the pass the newer solve owes"
    );

    let fresh = answer_now(&app, &path);
    dispatch(&mut app, &path, fresh);
    assert_eq!(
        editor(&app, &path)
            .state
            .last_solve
            .as_ref()
            .unwrap()
            .redundant_constraints,
        vec![redundant]
    );
}
