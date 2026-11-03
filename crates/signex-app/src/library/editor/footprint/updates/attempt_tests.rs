//! A rejected sketch edit leaves no undo step, no redo clear and no dirty
//! flag; a kept edit leaves all three.

use std::path::PathBuf;

use signex_library::primitive::footprint::Footprint;
use signex_library::{FootprintFile, LoadRepair};
use signex_sketch::SketchData;
use signex_sketch::constraint::{Constraint, ConstraintKind, DimTarget};
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::{ConstraintId, SketchEntityId};
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};

use super::apply_footprint_primitive_edit;
use crate::app::FootprintDocument;
use crate::library::messages::{FootprintEditorMsg, SketchConstraintTag};

/// P1 fixed at the origin, P2 free, one satisfiable 5 mm distance; solved,
/// with P1 and P2 selected and `dimension_input` holding `mm`.
fn editor_ready_to_constrain(mm: &str) -> FootprintDocument {
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
    let mut editor = FootprintDocument::new(
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
    editor.set_dirty(false);
    editor
}

fn constrain_distance(editor: &mut FootprintDocument) {
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
    for _ in 0..FootprintDocument::HISTORY_DEPTH {
        editor.push_history();
    }
    editor.history[0].selected_pad = Some(7);
    let spare = editor.begin_edit();
    editor.redo.push(spare);
    editor.set_dirty(false);

    constrain_distance(&mut editor);

    assert!(!editor.state.conflict.is_empty(), "the edit was rejected");
    assert_eq!(editor.history.len(), FootprintDocument::HISTORY_DEPTH);
    assert_eq!(
        editor.history[0].selected_pad,
        Some(7),
        "the oldest undo step was not evicted"
    );
    assert_eq!(editor.redo.len(), 1, "the redo stack was not cleared");
    assert!(
        !editor.is_dirty(),
        "a rejected edit does not dirty the document"
    );
}

#[test]
fn opening_a_footprint_the_loader_repaired_marks_it_dirty_and_clears_the_record() {
    let mut fp = Footprint::empty("repaired");
    fp.load_repairs = vec![LoadRepair::ArcEndMoved(SketchEntityId::new())];

    let editor = FootprintDocument::new(
        PathBuf::from("repaired.snxfpt"),
        FootprintFile::from_footprint(fp),
    );

    assert!(editor.is_dirty(), "a save must persist the repair");
    assert!(
        editor.file.footprints[0].load_repairs.is_empty(),
        "reported once; history snapshots never carry the record"
    );
}

#[test]
fn opening_an_untouched_footprint_stays_clean() {
    let fp = Footprint::empty("clean");

    let editor = FootprintDocument::new(
        PathBuf::from("clean.snxfpt"),
        FootprintFile::from_footprint(fp),
    );

    assert!(!editor.is_dirty());
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
    assert!(editor.is_dirty());
}

#[test]
fn opening_a_footprint_with_only_dangling_removals_marks_it_dirty() {
    let mut fp = Footprint::empty("healed");
    fp.load_repairs = vec![LoadRepair::DanglingRemoved(3)];

    let editor = FootprintDocument::new(
        PathBuf::from("healed.snxfpt"),
        FootprintFile::from_footprint(fp),
    );

    assert!(editor.is_dirty(), "a save must persist the heal");
    assert!(editor.file.footprints[0].load_repairs.is_empty());
}

/// A rejected edit gives `dirty` back as it was, but rows a failed cascade
/// still owes keep the editor unsaved.
#[test]
fn a_rejected_edit_does_not_clear_the_mark_of_owed_rows() {
    let mut editor = editor_ready_to_constrain("9");
    editor.set_dirty(false);
    editor.cascade_owed = vec![uuid::Uuid::now_v7()];

    constrain_distance(&mut editor);

    assert!(!editor.state.conflict.is_empty(), "the edit was rejected");
    assert!(editor.is_dirty(), "the owed rows are still unsaved");
}

/// #1854: undo back to the opened state reads clean, redo reads dirty.
#[test]
fn undoing_a_kept_edit_back_to_the_opened_state_reads_clean() {
    let mut editor = editor_ready_to_constrain("5");
    constrain_distance(&mut editor);
    assert!(editor.is_dirty());

    assert!(editor.undo());
    assert!(!editor.is_dirty(), "the opened state is the file");
    assert!(editor.redo());
    assert!(editor.is_dirty(), "redo moved away from the file");
}

/// #1854: a rejected edit disturbs the save point through its `mark_dirty`
/// and must give it back.
#[test]
fn a_rejected_edit_keeps_the_save_point() {
    let mut editor = editor_ready_to_constrain("9");
    editor.mark_saved();
    constrain_distance(&mut editor);

    assert!(!editor.state.conflict.is_empty(), "the edit was rejected");
    assert!(!editor.is_dirty());
    assert!(
        editor.save_point.is_at(editor.history.len()),
        "the document is still at the saved state"
    );
}

/// #1854: a history that evicted the saved step cannot reproduce the file.
#[test]
fn undoing_past_an_evicted_save_point_stays_dirty() {
    let mut editor = FootprintDocument::new(
        PathBuf::from("evict.snxfpt"),
        FootprintFile::from_footprint(Footprint::empty("evict")),
    );
    for _ in 0..=FootprintDocument::HISTORY_DEPTH {
        editor.record_edit(None, |e| {
            e.mark_dirty();
            true
        });
    }
    while editor.undo() {}
    assert!(editor.is_dirty(), "the opened state fell off the history");
}

#[test]
fn a_repaired_load_is_not_clean_after_undo() {
    let mut fp = Footprint::empty("repaired");
    fp.load_repairs = vec![LoadRepair::ArcEndMoved(SketchEntityId::new())];
    let mut editor = FootprintDocument::new(
        PathBuf::from("repaired.snxfpt"),
        FootprintFile::from_footprint(fp),
    );
    editor.record_edit(None, |e| {
        e.mark_dirty();
        true
    });
    assert!(editor.undo());
    assert!(editor.is_dirty(), "the repair is not on disk");
}

/// A save in the middle of a coalesced run: the run keeps editing the
/// saved state, so redoing back to it must not read clean.
#[test]
fn a_coalesced_edit_into_the_saved_step_forgets_the_save_point() {
    let mut editor = FootprintDocument::new(
        PathBuf::from("coalesce.snxfpt"),
        FootprintFile::from_footprint(Footprint::empty("coalesce")),
    );
    let key = Some(("name", 0));
    editor.record_edit(key, |e| {
        e.mark_dirty();
        true
    });
    editor.mark_saved();
    editor.record_edit(key, |e| {
        e.mark_dirty();
        true
    });
    assert!(editor.undo());
    assert!(editor.redo());
    assert!(editor.is_dirty(), "the redone state is not what was saved");
}
