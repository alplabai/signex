//! What is not an edit leaves undo, Redo and the save point alone: selection
//! (#2774), leaving sketch mode untouched (#2779), and a refused bake or
//! constraint (#2782).

use std::path::PathBuf;

use signex_library::FootprintFile;
use signex_library::primitive::footprint::Footprint;
use signex_sketch::attr::CourtyardAttr;
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;
use signex_sketch::parameter::resolve;
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
use signex_sketch::{SketchData, Solver};

use super::apply_footprint_primitive_edit as send;
use crate::app::FootprintDocument;
use crate::library::editor::footprint::state::EditorMode;
use crate::library::messages::{FootprintEditorMsg as M, SketchConstraintTag};

fn editor_on(footprint: Footprint) -> FootprintDocument {
    FootprintDocument::new(
        PathBuf::from("no-edit.snxfpt"),
        FootprintFile::from_footprint(footprint),
    )
}

/// One redo step, so a test can see whether a message cleared it.
fn with_a_redo_step(editor: &mut FootprintDocument) {
    let spare = editor.begin_edit();
    editor.history.redo_steps_mut().push(spare);
}

/// A sketch of courtyard lines through `corners`, left open, with its ends.
fn open_sketch(corners: &[(f64, f64)]) -> (SketchData, Vec<SketchEntityId>) {
    let plane = PlaneId::new();
    let ids: Vec<SketchEntityId> = corners.iter().map(|_| SketchEntityId::new()).collect();
    let mut entities: Vec<Entity> = ids
        .iter()
        .zip(corners)
        .map(|(&id, &(x, y))| Entity::new(id, plane, EntityKind::Point { x, y }))
        .collect();
    for pair in ids.windows(2) {
        let kind = EntityKind::Line {
            start: pair[0],
            end: pair[1],
        };
        let mut line = Entity::new(SketchEntityId::new(), plane, kind);
        line.courtyard = Some(CourtyardAttr {});
        entities.push(line);
    }
    let sketch = SketchData {
        planes: vec![Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        }],
        entities,
        ..SketchData::default()
    };
    (sketch, ids)
}

#[test]
fn selection_leaves_undo_redo_and_the_save_point_alone() {
    let mut editor = editor_on(Footprint::empty("select"));
    send(
        &mut editor,
        M::AddPad {
            x_mm: 0.0,
            y_mm: 0.0,
        },
    );
    editor.mark_saved();
    with_a_redo_step(&mut editor);
    let steps = editor.history.undo_len();

    for msg in [
        M::SelectPads(vec![0]),
        M::SketchSelectMany(Vec::new()),
        M::SelectAllOnLayer,
        M::LassoArm,
        M::LassoAddVertex {
            x_mm: -1.0,
            y_mm: -1.0,
        },
        M::LassoCancel,
        M::LassoArm,
        M::LassoCommit,
        M::TouchingLineArm,
        M::TouchingLineFirst {
            x_mm: -1.0,
            y_mm: 0.0,
        },
        M::TouchingLineCancel,
        M::TouchingLineArm,
        M::TouchingLineFirst {
            x_mm: -1.0,
            y_mm: 0.0,
        },
        M::TouchingLineCommit {
            x_mm: 1.0,
            y_mm: 0.0,
        },
        M::SelectOverlapped,
        M::SelectNextOverlapped,
        M::SelectOffGridPads,
    ] {
        let name = format!("{msg:?}");
        send(&mut editor, msg);
        assert_eq!(editor.history.undo_len(), steps, "{name}");
        assert_eq!(editor.history.redo_len(), 1, "{name}");
        assert!(!editor.is_dirty(), "{name}");
    }
}

#[test]
fn leaving_sketch_mode_without_an_edit_stays_clean() {
    for leave in [M::SketchDiscardRequested, M::SketchBakeRequested] {
        let mut editor = editor_on(Footprint::empty("leave"));
        send(&mut editor, M::SetMode(EditorMode::Sketch));
        editor.mark_saved();

        send(&mut editor, leave);

        assert_eq!(editor.state.mode, EditorMode::Normal);
        assert!(!editor.is_dirty());
    }
}

/// The restored footprint is not what the last save wrote.
#[test]
fn a_discard_after_a_save_inside_sketch_mode_is_unsaved() {
    let mut editor = editor_on(Footprint::empty("discard"));
    send(&mut editor, M::SetMode(EditorMode::Sketch));
    send(
        &mut editor,
        M::SketchPlacePoint {
            x_mm: 1.0,
            y_mm: 1.0,
        },
    );
    editor.mark_saved();

    send(&mut editor, M::SketchDiscardRequested);
    send(&mut editor, M::SketchDiscardConfirmed);

    assert_eq!(editor.state.mode, EditorMode::Normal);
    assert!(editor.is_dirty());
}

#[test]
fn an_open_profile_stops_the_bake_and_shows_its_ends() {
    let corners = [(0.0, 0.0), (5.0, 0.0), (5.0, 5.0), (0.0, 0.2)];
    let (sketch, points) = open_sketch(&corners);
    let params = resolve(&sketch.parameters).expect("no parameters");
    let solve = Solver::default().solve(&sketch, &params).expect("solves");
    let mut footprint = Footprint::empty("open");
    footprint.sketch = Some(sketch);
    let mut editor = editor_on(footprint);
    send(&mut editor, M::SetMode(EditorMode::Sketch));
    editor.state.last_solve = Some(solve);
    editor.mark_saved();
    let steps = editor.history.undo_len();

    send(&mut editor, M::SketchBakeRequested);

    assert_eq!(editor.state.mode, EditorMode::Sketch);
    assert!(
        editor.state.sketch_exit.preview,
        "the preview names the gap"
    );
    let shown = [
        editor.state.selected_sketch,
        editor.state.selected_sketch_secondary,
    ];
    assert!(shown.contains(&Some(points[0])), "{shown:?}");
    assert!(shown.contains(&Some(points[3])), "{shown:?}");
    assert_eq!(editor.history.undo_len(), steps);
    assert!(!editor.is_dirty());
}

#[test]
fn a_refused_constraint_records_nothing_and_keeps_redo() {
    let (sketch, points) = open_sketch(&[(0.0, 0.0), (3.0, 1.0)]);
    let mut footprint = Footprint::empty("refused");
    footprint.sketch = Some(sketch);
    let mut editor = editor_on(footprint);
    send(&mut editor, M::SetMode(EditorMode::Sketch));
    editor.mark_saved();
    editor.state.selected_sketch = Some(points[0]);
    with_a_redo_step(&mut editor);
    let steps = editor.history.undo_len();

    send(
        &mut editor,
        M::SketchAddConstraintForSelection(SketchConstraintTag::Horizontal),
    );

    assert!(
        !editor.state.solve_warnings.is_empty(),
        "the refusal is said"
    );
    assert_eq!(editor.history.undo_len(), steps);
    assert_eq!(editor.history.redo_len(), 1);
    assert!(!editor.is_dirty());
}
