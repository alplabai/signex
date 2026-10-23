//! A rejected sketch edit leaves no undo step, no redo clear and no dirty
//! flag; a kept edit leaves all three.

use std::path::PathBuf;

use signex_library::FootprintFile;
use signex_library::primitive::footprint::Footprint;
use signex_sketch::SketchData;
use signex_sketch::constraint::{Constraint, ConstraintKind, DimTarget};
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::{ConstraintId, SketchEntityId};
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};

use super::apply_footprint_primitive_edit;
use crate::app::FootprintEditorState as Editor;
use crate::library::messages::{FootprintEditorMsg, SketchConstraintTag};

/// P1 fixed at the origin, P2 free, one satisfiable 5 mm distance; solved,
/// with P1 and P2 selected and `dimension_input` holding `mm`.
fn editor_ready_to_constrain(mm: &str) -> Editor {
    let plane = PlaneId::new();
    let (p1, p2) = (SketchEntityId::new(), SketchEntityId::new());
    let mut fp = Footprint::empty("attempt");
    fp.sketch = Some(SketchData {
        planes: vec![Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        }],
        entities: vec![
            Entity::new(p1, plane, EntityKind::Point { x: 0.0, y: 0.0 }),
            Entity::new(p2, plane, EntityKind::Point { x: 1.0, y: 0.0 }),
        ],
        constraints: vec![
            Constraint {
                id: ConstraintId::new(),
                kind: ConstraintKind::Fixed { point: p1 },
            },
            Constraint {
                id: ConstraintId::new(),
                kind: ConstraintKind::DistancePtPt {
                    p1,
                    p2,
                    target: DimTarget::Literal(5.0),
                },
            },
        ],
        ..SketchData::default()
    });
    let mut editor = Editor::new(
        PathBuf::from("attempt.snxfpt"),
        FootprintFile::from_footprint(fp),
    );
    editor.with_parts(|state, primitive| {
        crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit(
            state,
            primitive,
            crate::library::editor::footprint::sketch_mode::SketchEdit::ForceRebuild,
        )
        .expect("the starting sketch is satisfiable");
    });
    editor.state.selected_sketch = Some(p1);
    editor.state.selected_sketch_secondary = Some(p2);
    editor.state.dimension_input = mm.to_string();
    editor.dirty = false;
    editor
}

fn constrain_distance(editor: &mut Editor) {
    apply_footprint_primitive_edit(
        editor,
        FootprintEditorMsg::SketchAddConstraintForSelection(SketchConstraintTag::DistancePtPt),
    );
}

#[test]
fn a_rejected_edit_leaves_undo_redo_and_dirty_untouched() {
    let mut editor = editor_ready_to_constrain("9");
    // A full undo stack whose oldest step is recognisable, and a redo step:
    // recording the rejected edit would evict the one and clear the other.
    for _ in 0..Editor::HISTORY_DEPTH {
        editor.push_history();
    }
    editor.history[0].selected_pad = Some(7);
    let spare = editor.begin_edit();
    editor.redo.push(spare);
    editor.dirty = false;

    constrain_distance(&mut editor);

    assert!(!editor.state.conflict.is_empty(), "the edit was rejected");
    assert_eq!(editor.history.len(), Editor::HISTORY_DEPTH);
    assert_eq!(
        editor.history[0].selected_pad,
        Some(7),
        "the oldest undo step was not evicted"
    );
    assert_eq!(editor.redo.len(), 1, "the redo stack was not cleared");
    assert!(!editor.dirty, "a rejected edit does not dirty the document");
}

#[test]
fn opening_a_footprint_the_loader_repaired_marks_it_dirty_and_clears_the_record() {
    let mut fp = Footprint::empty("repaired");
    fp.load_repairs = vec![SketchEntityId::new()];

    let editor = Editor::new(
        PathBuf::from("repaired.snxfpt"),
        FootprintFile::from_footprint(fp),
    );

    assert!(editor.dirty, "a save must persist the repair");
    assert!(
        editor.file.footprints[0].load_repairs.is_empty(),
        "reported once; history snapshots never carry the record"
    );
}

#[test]
fn opening_an_untouched_footprint_stays_clean() {
    let fp = Footprint::empty("clean");

    let editor = Editor::new(
        PathBuf::from("clean.snxfpt"),
        FootprintFile::from_footprint(fp),
    );

    assert!(!editor.dirty);
}

#[test]
fn a_kept_edit_records_history_clears_redo_and_marks_dirty() {
    // 5 mm again: consistent with the existing 5 mm distance.
    let mut editor = editor_ready_to_constrain("5");
    let spare = editor.begin_edit();
    editor.redo.push(spare);

    constrain_distance(&mut editor);

    assert!(editor.state.conflict.is_empty(), "the edit solved");
    assert_eq!(editor.history.len(), 1);
    assert!(editor.redo.is_empty());
    assert!(editor.dirty);
}
