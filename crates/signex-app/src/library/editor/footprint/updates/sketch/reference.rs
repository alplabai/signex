//! Reference dimensions and pattern mirrors (#187, #188): Convert to
//! reference and Make driving from the constraint menu or a Properties
//! conflict row, and Delete mirror from a mirror glyph's menu.
//!
//! Each is one undo step through `canvas_values::record_sketch_change`,
//! solved and baked like every sketch edit. Make driving that
//! over-constrains the sketch is reverted with its conflict named and
//! records nothing, as a value written on the drawing is.

use signex_sketch::array::ArrayId;
use signex_sketch::id::ConstraintId;

use super::canvas_values::record_sketch_change;
use crate::app::FootprintDocument;
use crate::library::editor::footprint::sketch_conflict::mutate_solve_or_revert;
use crate::library::editor::footprint::sketch_mode::constraint_menu::SketchReferenceMsg;
use crate::library::editor::footprint::state::sketch_overlay::ValueTarget;

pub(in crate::library::editor::footprint::updates) fn apply(
    editor: &mut FootprintDocument,
    msg: SketchReferenceMsg,
) {
    editor.state.sketch_overlay.menu = None;
    match msg {
        SketchReferenceMsg::ReferenceConverted(id) => set_driven(editor, id, true),
        SketchReferenceMsg::DrivingRestored(id) => set_driven(editor, id, false),
        SketchReferenceMsg::MirrorDeleted(id) => delete_mirror(editor, id),
    }
    editor.canvas_cache.clear();
}

/// Whether making dimension `id` driven (or driving) would change it.
fn would_change(editor: &FootprintDocument, id: ConstraintId, driven: bool) -> bool {
    let Some(sketch) = editor.primitive().sketch.as_ref() else {
        return false;
    };
    let is_dimension = sketch
        .constraints
        .iter()
        .any(|c| c.id == id && c.kind.dim_target().is_some());
    is_dimension && sketch.is_driven(id) != driven
}

/// Convert to reference (`driven`) or Make driving, as one undo step.
fn set_driven(editor: &mut FootprintDocument, id: ConstraintId, driven: bool) {
    if !would_change(editor, id, driven) {
        return;
    }
    let overlay = &mut editor.state.sketch_overlay;
    if overlay.is_editing(ValueTarget::Constraint(id)) {
        overlay.value_edit = None;
    }
    record_sketch_change(editor, |state, fp| {
        mutate_solve_or_revert(state, fp, |fp| {
            if let Some(sketch) = fp.sketch.as_mut() {
                sketch.set_driven(id, driven);
            }
        })
    });
}

/// Delete mirror: the mirrored pattern object goes; the pattern it mirrors
/// and the mirror line stay.
fn delete_mirror(editor: &mut FootprintDocument, id: ArrayId) {
    let overlay = &mut editor.state.sketch_overlay;
    if overlay.selected_pattern == Some(id) {
        overlay.selected_pattern = None;
    }
    let exists = editor
        .primitive()
        .sketch
        .as_ref()
        .is_some_and(|s| s.pattern_mirrors.iter().any(|m| m.id == id));
    if !exists {
        return;
    }
    record_sketch_change(editor, |state, fp| {
        mutate_solve_or_revert(state, fp, |fp| {
            if let Some(sketch) = fp.sketch.as_mut() {
                sketch.remove_pattern_mirror(id);
            }
        })
    });
}

impl crate::app::Signex {
    /// Properties' conflict row Convert to reference: make that dimension
    /// a reference in the editor in view, as the canvas constraint menu
    /// does (one undo step, then a solve).
    pub(crate) fn fp_editor_convert_conflicting_constraint(
        &mut self,
        constraint: ConstraintId,
    ) -> iced::Task<crate::app::Message> {
        use crate::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
        let Some(path) = self.active_footprint_editor().map(|e| e.path.clone()) else {
            return iced::Task::none();
        };
        self.update(crate::app::Message::Library(
            LibraryMessage::PrimitiveEditorEvent {
                path,
                msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchReference(
                    SketchReferenceMsg::ReferenceConverted(constraint),
                )),
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use signex_library::FootprintFile;
    use signex_library::primitive::footprint::Footprint;
    use signex_sketch::SketchData;
    use signex_sketch::constraint::{Constraint, ConstraintKind, DimTarget};
    use signex_sketch::entity::{Entity, EntityKind};
    use signex_sketch::id::SketchEntityId;
    use signex_sketch::plane::{Plane, PlaneId, PlaneKind};

    use super::*;
    use crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit;
    use crate::library::editor::footprint::sketch_mode::SketchEdit;
    use crate::library::editor::footprint::updates::apply_footprint_primitive_edit;
    use crate::library::messages::FootprintEditorMsg;

    /// P1 fixed, P2 5 mm from it by `first`; `second` a horizontal.
    fn fixture() -> (FootprintDocument, ConstraintId, ConstraintId) {
        let plane = PlaneId::new();
        let (p1, p2) = (SketchEntityId::new(), SketchEntityId::new());
        let (first, second) = (ConstraintId::new(), ConstraintId::new());
        let line = SketchEntityId::new();
        let mut fp = Footprint::empty("reference");
        fp.sketch = Some(SketchData {
            planes: vec![Plane {
                id: plane,
                kind: PlaneKind::BoardTop,
            }],
            entities: vec![
                Entity::new(p1, plane, EntityKind::Point { x: 0.0, y: 0.0 }),
                Entity::new(p2, plane, EntityKind::Point { x: 5.0, y: 0.0 }),
                Entity::new(line, plane, EntityKind::Line { start: p1, end: p2 }),
            ],
            constraints: vec![
                Constraint {
                    id: ConstraintId::new(),
                    kind: ConstraintKind::Fixed { point: p1 },
                },
                Constraint {
                    id: first,
                    kind: ConstraintKind::DistancePtPt {
                        p1,
                        p2,
                        target: DimTarget::Literal(5.0),
                    },
                },
                Constraint {
                    id: second,
                    kind: ConstraintKind::Horizontal { line },
                },
            ],
            ..SketchData::default()
        });
        let mut editor = FootprintDocument::new(
            PathBuf::from("reference.snxfpt"),
            FootprintFile::from_footprint(fp),
        );
        editor.with_parts(|state, primitive| {
            apply_sketch_edit(state, primitive, SketchEdit::ForceRebuild).expect("solvable");
        });
        editor.set_dirty(false);
        (editor, first, second)
    }

    fn send(editor: &mut FootprintDocument, msg: SketchReferenceMsg) {
        apply_footprint_primitive_edit(editor, FootprintEditorMsg::SketchReference(msg));
    }

    fn driven(editor: &FootprintDocument, id: ConstraintId) -> bool {
        editor
            .primitive()
            .sketch
            .as_ref()
            .is_some_and(|s| s.is_driven(id))
    }

    #[test]
    fn convert_and_make_driving_are_one_undo_step_each() {
        let (mut editor, first, _) = fixture();
        let undo_before = editor.history.undo_len();

        send(&mut editor, SketchReferenceMsg::ReferenceConverted(first));
        assert!(driven(&editor, first));
        assert_eq!(editor.history.undo_len(), undo_before + 1);
        assert!(editor.is_dirty());

        send(&mut editor, SketchReferenceMsg::DrivingRestored(first));
        assert!(!driven(&editor, first));
        assert_eq!(editor.history.undo_len(), undo_before + 2);

        assert!(editor.undo(), "the step undoes");
        assert!(driven(&editor, first), "undo brings the reference back");
    }

    #[test]
    fn a_relation_or_a_repeat_records_nothing() {
        let (mut editor, first, second) = fixture();
        let undo_before = editor.history.undo_len();
        send(&mut editor, SketchReferenceMsg::ReferenceConverted(second));
        send(&mut editor, SketchReferenceMsg::DrivingRestored(first));
        assert!(!driven(&editor, second));
        assert_eq!(editor.history.undo_len(), undo_before);
        assert!(!editor.is_dirty());
    }

    #[test]
    fn a_driven_value_does_not_open_for_editing() {
        let (mut editor, first, _) = fixture();
        send(&mut editor, SketchReferenceMsg::ReferenceConverted(first));
        apply_footprint_primitive_edit(
            &mut editor,
            FootprintEditorMsg::SketchCanvas(
                crate::library::editor::footprint::sketch_mode::SketchCanvasMsg::ValueEditOpened(
                    ValueTarget::Constraint(first),
                ),
            ),
        );
        assert!(editor.state.sketch_overlay.value_edit.is_none());
    }
}
