//! Footprint sketch updates — values on the drawing, the constraint and
//! pattern menus, and instance suppression (mockup row 187, mockup row 188, mockup row 191), on both
//! hosts (a board sketch runs the same editor).
//!
//! The overlay messages (open, type, close) are UI state and record no
//! undo step. An edit that changes the sketch (a value written, a
//! constraint or pattern deleted, an instance suppressed) is one undo step
//! through `record_edit`, solved and baked like every sketch edit; one the
//! solver rejects as over-constrained is reverted, its conflict named, and
//! leaves no step and no dirty flag behind.

use signex_library::primitive::footprint::Footprint;
use signex_sketch::SketchError;
use signex_sketch::array::ArrayId;
use signex_sketch::constraint::{ConstraintKind, DimTarget};
use signex_sketch::id::{ConstraintId, SketchEntityId};

use crate::app::FootprintDocument;
use crate::library::editor::footprint::sketch_conflict::mutate_solve_or_revert;
use crate::library::editor::footprint::sketch_dispatch::{apply_sketch_edit, report_solve_failure};
use crate::library::editor::footprint::sketch_mode::{SketchCanvasMsg, SketchEdit, value_edit};
use crate::library::editor::footprint::state::FootprintEditorState;
use crate::library::editor::footprint::state::sketch_overlay::{
    CanvasMenu, MenuTarget, PatternField, ValueEdit, ValueTarget,
};
use crate::library::messages::FootprintEditorMsg;

pub(in crate::library::editor::footprint::updates) fn apply(
    editor: &mut FootprintDocument,
    msg: SketchCanvasMsg,
) {
    match msg {
        SketchCanvasMsg::ValueEditOpened(target) => open_value_edit(editor, target),
        SketchCanvasMsg::ValueTyped(ch) => edit_buffer(editor, |b| value_edit::typed(b, ch)),
        SketchCanvasMsg::ValueErased => edit_buffer(editor, |b| {
            let mut next = b.to_string();
            next.pop().map(|_| next)
        }),
        SketchCanvasMsg::ValueCommitted => commit_value(editor),
        SketchCanvasMsg::ValueEditCancelled => editor.state.sketch_overlay.value_edit = None,
        SketchCanvasMsg::ConstraintMenuOpened { constraint, x, y } => {
            open_menu(editor, MenuTarget::Constraint(constraint), x, y)
        }
        SketchCanvasMsg::PatternMenuOpened { array, x, y } => {
            editor.state.sketch_overlay.selected_pattern = Some(array);
            open_menu(editor, MenuTarget::Pattern(array), x, y)
        }
        SketchCanvasMsg::MenuClosed => editor.state.sketch_overlay.menu = None,
        SketchCanvasMsg::ConstrainedEntitiesSelected(id) => {
            let ids = constrained_entities(editor, id);
            select_entities(editor, ids)
        }
        SketchCanvasMsg::ConflictsShown(_) => {
            let ids = conflict_entities(editor);
            select_entities(editor, ids)
        }
        SketchCanvasMsg::ConstraintDeleted(id) => delete_constraint(editor, id),
        SketchCanvasMsg::ParameterRequested(id) => request_parameter(editor, id),
        SketchCanvasMsg::PatternSelected(array) => select_pattern(editor, array),
        SketchCanvasMsg::PatternDeleted(array) => delete_pattern(editor, array),
        SketchCanvasMsg::InstanceSuppressionToggled { array, i, j } => {
            toggle_instance(editor, array, i, j)
        }
    }
    editor.canvas_cache.clear();
}

/// Run `change` on the sketch as one undo step. Returns whether the
/// sketch changed (a rejected or empty change records nothing).
pub(super) fn record_sketch_change(
    editor: &mut FootprintDocument,
    change: impl FnOnce(&mut FootprintEditorState, &mut Footprint) -> Result<(), SketchError>,
) -> bool {
    editor.record_edit(None, |editor| {
        let before = editor.primitive().sketch.clone();
        let result = editor.with_parts(change);
        if let Err(error) = &result {
            report_solve_failure(&mut editor.state, error);
        }
        let changed = editor.primitive().sketch != before;
        if changed {
            editor.mark_dirty();
        }
        changed
    })
}

/// The sketch, if the footprint has one.
fn sketch(editor: &FootprintDocument) -> Option<&signex_sketch::SketchData> {
    editor.primitive().sketch.as_ref()
}

fn open_value_edit(editor: &mut FootprintDocument, target: ValueTarget) {
    let shown = sketch(editor).and_then(|s| match target {
        ValueTarget::Constraint(id) => {
            let constraint = s.constraints.iter().find(|c| c.id == id)?;
            value_edit::constraint_buffer(&editor.state.shown_constraint(&constraint.kind))
        }
        ValueTarget::Pattern(..) => value_edit::initial_buffer(target, s),
    });
    let Some(buffer) = shown else {
        return;
    };
    // A reference (driven) dimension measures; its value is not edited.
    if let ValueTarget::Constraint(id) = target
        && sketch(editor).is_some_and(|s| s.is_driven(id))
    {
        return;
    }
    let overlay = &mut editor.state.sketch_overlay;
    overlay.menu = None;
    if let ValueTarget::Pattern(array, _) = target {
        overlay.selected_pattern = Some(array);
    }
    overlay.value_edit = Some(ValueEdit {
        target,
        buffer,
        invalid: false,
    });
}

/// Replace the open edit's text with `next(text)`, when it gives one.
fn edit_buffer(editor: &mut FootprintDocument, next: impl FnOnce(&str) -> Option<String>) {
    if let Some(edit) = editor.state.sketch_overlay.value_edit.as_mut()
        && let Some(buffer) = next(&edit.buffer)
    {
        edit.buffer = buffer;
        edit.invalid = false;
    }
}

/// What a committed edit writes.
enum ValueWrite {
    /// What was entered is what the dimension already shows: no edit.
    Unchanged,
    Constraint(ConstraintId, DimTarget),
    Pattern(ArrayId, PatternField, String),
}

/// Whether the edited value still exists (an undo may have removed it).
fn target_exists(editor: &FootprintDocument, target: ValueTarget) -> bool {
    sketch(editor).is_some_and(|s| match target {
        ValueTarget::Constraint(id) => s.constraints.iter().any(|c| c.id == id),
        ValueTarget::Pattern(array, _) => s.arrays.iter().any(|a| a.id == array),
    })
}

/// The write the edit's text stands for; `None` when the text does not
/// read as a value for its target.
fn prepared_write(editor: &FootprintDocument, edit: &ValueEdit) -> Option<ValueWrite> {
    let sketch = sketch(editor)?;
    match edit.target {
        ValueTarget::Constraint(id) => {
            let constraint = sketch.constraints.iter().find(|c| c.id == id)?;
            let params = value_edit::resolved_params(sketch, &editor.state);
            let typed =
                value_edit::parse_constraint_buffer(&edit.buffer, &constraint.kind, &params)?;
            if value_edit::repeats_shown_board_angle(
                &editor.state,
                &constraint.kind,
                &edit.buffer,
                &typed,
                &params,
            ) {
                return Some(ValueWrite::Unchanged);
            }
            let target = editor.state.stored_target(&constraint.kind, typed);
            Some(ValueWrite::Constraint(id, target))
        }
        ValueTarget::Pattern(array, field) => {
            let typed = edit.buffer.trim();
            value_edit::pattern_value_ok(sketch, field, typed)
                .then(|| ValueWrite::Pattern(array, field, typed.to_string()))
        }
    }
}

fn write_value(fp: &mut Footprint, write: &ValueWrite) {
    let Some(sketch) = fp.sketch.as_mut() else {
        return;
    };
    match write {
        ValueWrite::Unchanged => {}
        ValueWrite::Constraint(id, new_target) => {
            let Some(constraint) = sketch.constraints.iter_mut().find(|c| c.id == *id) else {
                return;
            };
            if let ConstraintKind::DistancePtPt { target, .. }
            | ConstraintKind::DistancePtLine { target, .. }
            | ConstraintKind::DistancePtCircle { target, .. }
            | ConstraintKind::Angle { target, .. } = &mut constraint.kind
            {
                *target = new_target.clone();
            }
        }
        ValueWrite::Pattern(array, field, expr) => {
            if let Some(a) = sketch.arrays.iter_mut().find(|a| a.id == *array) {
                field.set_expr(&mut a.kind, expr);
            }
        }
    }
}

/// Enter: write the value as one undo step and solve. Unreadable text
/// keeps the edit open, marked; a gone target just closes it.
fn commit_value(editor: &mut FootprintDocument) {
    let Some(edit) = editor.state.sketch_overlay.value_edit.clone() else {
        return;
    };
    if !target_exists(editor, edit.target) {
        editor.state.sketch_overlay.value_edit = None;
        return;
    }
    let Some(write) = prepared_write(editor, &edit) else {
        if let Some(open) = editor.state.sketch_overlay.value_edit.as_mut() {
            open.invalid = true;
        }
        return;
    };
    editor.state.sketch_overlay.value_edit = None;
    if matches!(write, ValueWrite::Unchanged) {
        return;
    }
    record_sketch_change(editor, |state, fp| {
        mutate_solve_or_revert(state, fp, |fp| write_value(fp, &write))
    });
}

fn open_menu(editor: &mut FootprintDocument, target: MenuTarget, x: f32, y: f32) {
    let overlay = &mut editor.state.sketch_overlay;
    overlay.value_edit = None;
    overlay.menu = Some(CanvasMenu { target, x, y });
}

/// The entities constraint `id` names.
fn constrained_entities(editor: &FootprintDocument, id: ConstraintId) -> Vec<SketchEntityId> {
    sketch(editor)
        .and_then(|s| s.constraints.iter().find(|c| c.id == id))
        .map(|c| c.kind.entity_refs())
        .unwrap_or_default()
}

/// Every entity of the conflict: those the conflicting constraints name,
/// and those a rejected edit's constraint named before it was reverted.
fn conflict_entities(editor: &FootprintDocument) -> Vec<SketchEntityId> {
    let Some(sketch) = sketch(editor) else {
        return Vec::new();
    };
    let conflicts = value_edit::conflicting_constraints(&editor.state);
    let mut ids: Vec<SketchEntityId> = Vec::new();
    let named = sketch
        .constraints
        .iter()
        .filter(|c| conflicts.contains(&c.id))
        .flat_map(|c| c.kind.entity_refs());
    let rejected = editor.state.conflict.entities(sketch);
    for id in named.chain(rejected) {
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    ids
}

fn select_entities(editor: &mut FootprintDocument, ids: Vec<SketchEntityId>) {
    editor.state.sketch_overlay.menu = None;
    super::ui::apply(editor, FootprintEditorMsg::SketchSelectMany(ids));
}

fn delete_constraint(editor: &mut FootprintDocument, id: ConstraintId) {
    let overlay = &mut editor.state.sketch_overlay;
    overlay.menu = None;
    if overlay.is_editing(ValueTarget::Constraint(id)) {
        overlay.value_edit = None;
    }
    if !target_exists(editor, ValueTarget::Constraint(id)) {
        return;
    }
    record_sketch_change(editor, |state, fp| {
        apply_sketch_edit(state, fp, SketchEdit::DeleteConstraint(id))
    });
}

/// Make parameter: handed to the sketch parameters table, which names the
/// parameter and binds the dimension to it.
fn request_parameter(editor: &mut FootprintDocument, id: ConstraintId) {
    editor.state.sketch_overlay.menu = None;
    crate::library::editor::footprint::updates::apply_footprint_primitive_edit(
        editor,
        crate::library::editor::footprint::sketch_mode::constraint_menu::parameter_request(id),
    );
}

fn select_pattern(editor: &mut FootprintDocument, array: Option<ArrayId>) {
    let overlay = &mut editor.state.sketch_overlay;
    overlay.selected_pattern = array;
    let editing_other_pattern = overlay
        .value_edit
        .as_ref()
        .is_some_and(|e| matches!(e.target, ValueTarget::Pattern(a, _) if Some(a) != array));
    if editing_other_pattern {
        overlay.value_edit = None;
    }
}

fn delete_pattern(editor: &mut FootprintDocument, array: ArrayId) {
    let overlay = &mut editor.state.sketch_overlay;
    overlay.menu = None;
    if overlay.selected_pattern == Some(array) {
        overlay.selected_pattern = None;
        overlay.value_edit = None;
    }
    if !target_exists(editor, ValueTarget::Pattern(array, PatternField::Count)) {
        return;
    }
    // The source entity stays, as the Properties pattern form's Delete
    // leaves it.
    record_sketch_change(editor, |state, fp| {
        mutate_solve_or_revert(state, fp, |fp| {
            if let Some(sketch) = fp.sketch.as_mut() {
                sketch.arrays.retain(|a| a.id != array);
            }
        })
    });
}

/// A click on an instance slot of the selected pattern. A linear pattern
/// has no suppression list, so its slots never toggle.
fn toggle_instance(editor: &mut FootprintDocument, array: ArrayId, i: u32, j: u32) {
    record_sketch_change(editor, |state, fp| {
        mutate_solve_or_revert(state, fp, |fp| {
            if let Some(a) = fp
                .sketch
                .as_mut()
                .and_then(|s| s.arrays.iter_mut().find(|a| a.id == array))
            {
                a.kind.toggle_instance_suppressed(i, j);
            }
        })
    });
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use signex_library::FootprintFile;
    use signex_sketch::SketchData;
    use signex_sketch::array::{Array, ArrayKind, NumberingScheme};
    use signex_sketch::constraint::Constraint;
    use signex_sketch::entity::{Entity, EntityKind};
    use signex_sketch::plane::{Plane, PlaneId, PlaneKind};

    use super::*;
    use crate::library::editor::footprint::updates::apply_footprint_primitive_edit;

    struct Fixture {
        editor: FootprintDocument,
        /// The 5 mm distance between the two points.
        first: ConstraintId,
        /// A second 5 mm distance between the same points: redundant, so a
        /// different value for either over-constrains the sketch.
        second: ConstraintId,
        grid: ArrayId,
        linear: ArrayId,
    }

    fn distance(id: ConstraintId, p1: SketchEntityId, p2: SketchEntityId) -> Constraint {
        Constraint {
            id,
            kind: ConstraintKind::DistancePtPt {
                p1,
                p2,
                target: DimTarget::Literal(5.0),
            },
        }
    }

    /// P1 fixed at the origin, P2 5 mm away held by two equal distances,
    /// a 3 x 2 grid and a 4-long linear pattern of P2; solved, clean.
    fn fixture() -> Fixture {
        let plane = PlaneId::new();
        let (p1, p2) = (SketchEntityId::new(), SketchEntityId::new());
        let (first, second) = (ConstraintId::new(), ConstraintId::new());
        let (grid, linear) = (ArrayId::new(), ArrayId::new());
        let mut fp = Footprint::empty("values");
        fp.sketch = Some(SketchData {
            planes: vec![Plane {
                id: plane,
                kind: PlaneKind::BoardTop,
            }],
            entities: vec![
                Entity::new(p1, plane, EntityKind::Point { x: 0.0, y: 0.0 }),
                Entity::new(p2, plane, EntityKind::Point { x: 5.0, y: 0.0 }),
            ],
            constraints: vec![
                Constraint {
                    id: ConstraintId::new(),
                    kind: ConstraintKind::Fixed { point: p1 },
                },
                distance(first, p1, p2),
                distance(second, p1, p2),
            ],
            arrays: vec![
                Array {
                    id: grid,
                    kind: ArrayKind::Grid {
                        source: p2,
                        nx_expr: "3".into(),
                        ny_expr: "2".into(),
                        dx_expr: "1mm".into(),
                        dy_expr: "1mm".into(),
                        depopulation: None,
                    },
                    numbering: NumberingScheme::default(),
                },
                Array {
                    id: linear,
                    kind: ArrayKind::Linear {
                        source: p2,
                        count_expr: "4".into(),
                        dx_expr: "1mm".into(),
                        dy_expr: "0mm".into(),
                    },
                    numbering: NumberingScheme::default(),
                },
            ],
            ..SketchData::default()
        });
        let mut editor = FootprintDocument::new(
            PathBuf::from("values.snxfpt"),
            FootprintFile::from_footprint(fp),
        );
        editor.with_parts(|state, primitive| {
            apply_sketch_edit(state, primitive, SketchEdit::ForceRebuild)
                .expect("the starting sketch is satisfiable");
        });
        editor.set_dirty(false);
        Fixture {
            editor,
            first,
            second,
            grid,
            linear,
        }
    }

    fn send(editor: &mut FootprintDocument, msg: SketchCanvasMsg) {
        apply_footprint_primitive_edit(editor, FootprintEditorMsg::SketchCanvas(msg));
    }

    fn type_text(editor: &mut FootprintDocument, text: &str) {
        while editor
            .state
            .sketch_overlay
            .value_edit
            .as_ref()
            .is_some_and(|e| !e.buffer.is_empty())
        {
            send(editor, SketchCanvasMsg::ValueErased);
        }
        for ch in text.chars() {
            send(editor, SketchCanvasMsg::ValueTyped(ch));
        }
    }

    fn target_of(editor: &FootprintDocument, id: ConstraintId) -> DimTarget {
        let sketch = editor.primitive().sketch.as_ref().expect("sketch");
        let c = sketch
            .constraints
            .iter()
            .find(|c| c.id == id)
            .expect("constraint");
        c.kind.dim_target().expect("a dimension").clone()
    }

    fn array_kind(editor: &FootprintDocument, id: ArrayId) -> ArrayKind {
        let sketch = editor.primitive().sketch.as_ref().expect("sketch");
        sketch
            .arrays
            .iter()
            .find(|a| a.id == id)
            .expect("array")
            .kind
            .clone()
    }

    #[test]
    fn a_value_edit_starts_from_the_value_and_enter_writes_it_as_one_undo_step() {
        let Fixture {
            mut editor,
            first,
            second,
            ..
        } = fixture();
        // Both distances move together, or the sketch over-constrains.
        let undo_before = editor.history.undo_len();

        send(
            &mut editor,
            SketchCanvasMsg::ValueEditOpened(ValueTarget::Constraint(second)),
        );
        let open = editor
            .state
            .sketch_overlay
            .value_edit
            .clone()
            .expect("edit open");
        assert_eq!(open.buffer, "5");
        send(&mut editor, SketchCanvasMsg::ConstraintDeleted(first));
        assert_eq!(
            editor.history.undo_len(),
            undo_before + 1,
            "the delete is one step"
        );
        type_text(&mut editor, "7");
        send(&mut editor, SketchCanvasMsg::ValueCommitted);

        assert_eq!(target_of(&editor, second), DimTarget::Literal(7.0));
        assert!(
            editor.state.sketch_overlay.value_edit.is_none(),
            "Enter closes the edit"
        );
        assert_eq!(
            editor.history.undo_len(),
            undo_before + 2,
            "the value is one more step"
        );
        assert!(editor.is_dirty());
    }

    #[test]
    fn esc_drops_the_edit_and_changes_nothing() {
        let Fixture {
            mut editor, second, ..
        } = fixture();
        let undo_before = editor.history.undo_len();

        send(
            &mut editor,
            SketchCanvasMsg::ValueEditOpened(ValueTarget::Constraint(second)),
        );
        type_text(&mut editor, "9");
        send(&mut editor, SketchCanvasMsg::ValueEditCancelled);

        assert!(editor.state.sketch_overlay.value_edit.is_none());
        assert_eq!(target_of(&editor, second), DimTarget::Literal(5.0));
        assert_eq!(editor.history.undo_len(), undo_before);
        assert!(!editor.is_dirty());
    }

    #[test]
    fn an_over_constraining_value_is_reverted_and_its_conflict_named() {
        let Fixture {
            mut editor, second, ..
        } = fixture();
        let undo_before = editor.history.undo_len();

        send(
            &mut editor,
            SketchCanvasMsg::ValueEditOpened(ValueTarget::Constraint(second)),
        );
        type_text(&mut editor, "9");
        send(&mut editor, SketchCanvasMsg::ValueCommitted);

        assert_eq!(
            target_of(&editor, second),
            DimTarget::Literal(5.0),
            "reverted"
        );
        assert!(!editor.state.conflict.is_empty(), "the conflict is named");
        assert_eq!(
            editor.history.undo_len(),
            undo_before,
            "a rejected edit records no step"
        );
        assert!(!editor.is_dirty());
    }

    #[test]
    fn unreadable_text_keeps_the_edit_open_and_marked() {
        let Fixture {
            mut editor, second, ..
        } = fixture();
        send(
            &mut editor,
            SketchCanvasMsg::ValueEditOpened(ValueTarget::Constraint(second)),
        );
        type_text(&mut editor, "no_such_parameter");
        send(&mut editor, SketchCanvasMsg::ValueCommitted);

        let open = editor
            .state
            .sketch_overlay
            .value_edit
            .clone()
            .expect("still open");
        assert!(open.invalid);
        assert_eq!(target_of(&editor, second), DimTarget::Literal(5.0));
        send(&mut editor, SketchCanvasMsg::ValueTyped('x'));
        assert!(
            !editor
                .state
                .sketch_overlay
                .value_edit
                .as_ref()
                .expect("open")
                .invalid,
            "typing clears the mark"
        );
    }

    #[test]
    fn remove_deletes_one_constraint_in_one_undo_step() {
        let Fixture {
            mut editor, first, ..
        } = fixture();
        let count =
            |e: &FootprintDocument| e.primitive().sketch.as_ref().expect("s").constraints.len();
        let (before, undo_before) = (count(&editor), editor.history.undo_len());

        send(&mut editor, SketchCanvasMsg::ConstraintDeleted(first));

        assert_eq!(count(&editor), before - 1);
        assert_eq!(editor.history.undo_len(), undo_before + 1);
        assert!(editor.undo(), "the step undoes");
        assert_eq!(count(&editor), before);
    }

    #[test]
    fn a_grid_instance_click_suppresses_it_and_a_second_click_brings_it_back() {
        let Fixture {
            mut editor, grid, ..
        } = fixture();
        let undo_before = editor.history.undo_len();
        let toggle = SketchCanvasMsg::InstanceSuppressionToggled {
            array: grid,
            i: 1,
            j: 0,
        };

        send(&mut editor, toggle.clone());
        assert!(array_kind(&editor, grid).is_instance_suppressed(1, 0));
        assert_eq!(editor.history.undo_len(), undo_before + 1);

        send(&mut editor, toggle);
        assert!(!array_kind(&editor, grid).is_instance_suppressed(1, 0));
        assert!(
            matches!(
                array_kind(&editor, grid),
                ArrayKind::Grid {
                    depopulation: None,
                    ..
                }
            ),
            "an emptied list with a blank mask leaves no depopulation behind"
        );
        assert_eq!(editor.history.undo_len(), undo_before + 2);
    }

    #[test]
    fn a_linear_pattern_instance_does_not_toggle() {
        let Fixture {
            mut editor, linear, ..
        } = fixture();
        let before = array_kind(&editor, linear);
        let undo_before = editor.history.undo_len();

        send(
            &mut editor,
            SketchCanvasMsg::InstanceSuppressionToggled {
                array: linear,
                i: 1,
                j: 0,
            },
        );

        assert_eq!(array_kind(&editor, linear), before);
        assert_eq!(editor.history.undo_len(), undo_before);
    }

    #[test]
    fn a_pattern_count_edits_on_the_drawing() {
        let Fixture {
            mut editor, linear, ..
        } = fixture();
        send(
            &mut editor,
            SketchCanvasMsg::ValueEditOpened(ValueTarget::Pattern(linear, PatternField::Count)),
        );
        assert_eq!(editor.state.sketch_overlay.selected_pattern, Some(linear));
        type_text(&mut editor, "6");
        send(&mut editor, SketchCanvasMsg::ValueCommitted);

        assert!(matches!(
            array_kind(&editor, linear),
            ArrayKind::Linear { count_expr, .. } if count_expr == "6"
        ));
    }

    #[test]
    fn the_menu_and_the_edit_are_ui_state_and_record_nothing() {
        let Fixture {
            mut editor, first, ..
        } = fixture();
        let undo_before = editor.history.undo_len();
        send(
            &mut editor,
            SketchCanvasMsg::ConstraintMenuOpened {
                constraint: first,
                x: 10.0,
                y: 20.0,
            },
        );
        assert!(editor.state.sketch_overlay.menu.is_some());
        send(&mut editor, SketchCanvasMsg::MenuClosed);
        assert!(editor.state.sketch_overlay.menu.is_none());
        assert_eq!(editor.history.undo_len(), undo_before);
        assert!(!editor.is_dirty());
    }
}
