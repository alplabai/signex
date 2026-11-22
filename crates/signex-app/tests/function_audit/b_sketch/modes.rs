//! m187, footprint sketch mode: the switch, a value edited on the drawing,
//! the Linetype segment, and the way out (Bake preview, Discard, Bake).

use signex_app::library::editor::footprint::sketch_mode::{SketchCanvasMsg, SketchLinetype};
use signex_app::library::editor::footprint::{EditorMode, SketchTool, ValueTarget};
use signex_app::library::messages::FootprintEditorMsg;
use signex_library::Footprint;
use signex_sketch::constraint::{Constraint, ConstraintKind, DimTarget};
use signex_sketch::entity::EntityKind;
use signex_sketch::id::ConstraintId;

use super::support::{FpRig, SketchBuilder, open_footprint, sketch_rig};

/// `a` fixed at the origin and `b` a literal 2 mm from it, in Sketch mode.
fn dimension_rig() -> (FpRig, ConstraintId) {
    let mut s = SketchBuilder::new();
    let (a, b) = (s.point(0.0, 0.0), s.point(2.0, 0.0));
    s.data.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Fixed { point: a },
    });
    let dimension = ConstraintId::new();
    s.data.constraints.push(Constraint {
        id: dimension,
        kind: ConstraintKind::DistancePtPt {
            p1: a,
            p2: b,
            target: DimTarget::Literal(2.0),
        },
    });
    (sketch_rig(s.data), dimension)
}

fn canvas(rig: &mut FpRig, msg: SketchCanvasMsg) {
    rig.send(FootprintEditorMsg::SketchCanvas(msg));
}

fn target(rig: &FpRig, id: ConstraintId) -> DimTarget {
    rig.sketch()
        .constraints
        .iter()
        .find(|c| c.id == id)
        .and_then(|c| c.kind.dim_target().cloned())
        .expect("the dimension")
}

/// Open the value of `id` and replace its text with `text`.
fn type_value(rig: &mut FpRig, id: ConstraintId, text: &str) {
    canvas(
        rig,
        SketchCanvasMsg::ValueEditOpened(ValueTarget::Constraint(id)),
    );
    while rig
        .editor()
        .state
        .sketch_overlay
        .value_edit
        .as_ref()
        .is_some_and(|edit| !edit.buffer.is_empty())
    {
        canvas(rig, SketchCanvasMsg::ValueErased);
    }
    for ch in text.chars() {
        canvas(rig, SketchCanvasMsg::ValueTyped(ch));
    }
}

fn edited(rig: &mut FpRig, id: ConstraintId) {
    type_value(rig, id, "4");
    canvas(rig, SketchCanvasMsg::ValueCommitted);
    assert_eq!(target(rig, id), DimTarget::Literal(4.0));
}

#[test]
fn m187_the_switch_enters_sketch_mode_and_leaves_it_without_an_edit() {
    let mut rig = open_footprint(Footprint::empty("switch"));

    rig.send(FootprintEditorMsg::SetMode(EditorMode::Sketch));
    assert_eq!(rig.editor().state.mode, EditorMode::Sketch);
    rig.send(FootprintEditorMsg::SetMode(EditorMode::Normal));

    assert_eq!(rig.editor().state.mode, EditorMode::Normal);
    assert_eq!(rig.steps(), 0);
}

#[test]
fn m187_enter_on_a_value_writes_it_as_one_undo_step() {
    let (mut rig, id) = dimension_rig();
    let steps = rig.steps();

    edited(&mut rig, id);

    assert!(rig.editor().state.sketch_overlay.value_edit.is_none());
    assert_eq!(rig.steps(), steps + 1);
    assert!(rig.dirty());
    rig.undo();
    assert_eq!(target(&rig, id), DimTarget::Literal(2.0));
    assert!(!rig.dirty());
}

#[test]
fn m187_esc_on_a_value_keeps_the_dimension() {
    let (mut rig, id) = dimension_rig();

    type_value(&mut rig, id, "4");
    canvas(&mut rig, SketchCanvasMsg::ValueEditCancelled);

    assert_eq!(target(&rig, id), DimTarget::Literal(2.0));
    assert!(rig.editor().state.sketch_overlay.value_edit.is_none());
    assert!(!rig.dirty());
}

#[test]
fn m187_a_value_that_does_not_read_stays_open_and_marked() {
    let (mut rig, id) = dimension_rig();
    let steps = rig.steps();

    type_value(&mut rig, id, "nosuchname");
    canvas(&mut rig, SketchCanvasMsg::ValueCommitted);

    let edit = rig.editor().state.sketch_overlay.value_edit.as_ref();
    assert!(edit.is_some_and(|e| e.invalid), "{edit:?}");
    assert_eq!(target(&rig, id), DimTarget::Literal(2.0));
    assert_eq!(rig.steps(), steps);
    assert!(!rig.dirty());
}

#[test]
fn m187_the_linetype_segment_sets_what_the_next_line_is() {
    let mut rig = sketch_rig(SketchBuilder::new().data);

    rig.send(FootprintEditorMsg::SketchLinetypePicked(
        SketchLinetype::Construction,
    ));
    rig.arm(SketchTool::Line);
    rig.tool_click(1.0, 1.0, None);
    rig.tool_click(4.0, 1.0, None);
    rig.send(FootprintEditorMsg::ToolEscape);

    let line = rig
        .sketch()
        .entities
        .iter()
        .find(|e| matches!(e.kind, EntityKind::Line { .. }))
        .expect("a line was drawn");
    assert!(line.construction);
    assert!(!line.centerline);
}

#[test]
fn m187_bake_preview_opens_and_back_to_sketch_closes_without_an_edit() {
    let (mut rig, _) = dimension_rig();

    rig.send(FootprintEditorMsg::SketchBakePreviewToggled);
    assert!(rig.editor().state.sketch_exit.preview);
    rig.send(FootprintEditorMsg::SketchBakePreviewToggled);

    assert!(!rig.editor().state.sketch_exit.preview);
    assert_eq!(rig.editor().state.mode, EditorMode::Sketch);
    assert!(!rig.dirty());
}

#[test]
fn m187_bake_keeps_the_edit_and_goes_back_to_the_pads_view() {
    let (mut rig, id) = dimension_rig();
    edited(&mut rig, id);

    rig.send(FootprintEditorMsg::SketchBakeRequested);

    assert_eq!(rig.editor().state.mode, EditorMode::Normal);
    assert_eq!(target(&rig, id), DimTarget::Literal(4.0));
    assert!(rig.dirty());
}

#[test]
fn m187_discard_with_no_edit_leaves_at_once() {
    let (mut rig, _) = dimension_rig();

    rig.send(FootprintEditorMsg::SketchDiscardRequested);

    assert_eq!(rig.editor().state.mode, EditorMode::Normal);
    assert!(!rig.dirty());
}

#[test]
fn m187_discard_with_an_edit_asks_and_keep_editing_keeps_it() {
    let (mut rig, id) = dimension_rig();
    edited(&mut rig, id);

    rig.send(FootprintEditorMsg::SketchDiscardRequested);
    assert!(rig.editor().state.sketch_exit.confirming_discard);
    assert_eq!(rig.editor().state.mode, EditorMode::Sketch);

    rig.send(FootprintEditorMsg::SketchDiscardCancelled);
    assert!(!rig.editor().state.sketch_exit.confirming_discard);
    assert_eq!(target(&rig, id), DimTarget::Literal(4.0));
}

#[test]
fn m187_a_confirmed_discard_drops_the_edit_and_undo_brings_it_back() {
    let (mut rig, id) = dimension_rig();
    edited(&mut rig, id);

    rig.send(FootprintEditorMsg::SketchDiscardRequested);
    rig.send(FootprintEditorMsg::SketchDiscardConfirmed);
    assert_eq!(rig.editor().state.mode, EditorMode::Normal);
    assert_eq!(target(&rig, id), DimTarget::Literal(2.0));

    rig.undo();
    assert_eq!(target(&rig, id), DimTarget::Literal(4.0));
}
