//! m191, Mirror and patterns: the Modify tools, values on the drawing,
//! instance suppression (grid and polar only, decision of 2026-11-20) and
//! Delete pattern.

use signex_app::library::editor::footprint::sketch_mode::SketchCanvasMsg;
use signex_app::library::editor::footprint::{PatternField, SketchTool, ValueTarget};
use signex_app::library::messages::FootprintEditorMsg;
use signex_sketch::array::{Array, ArrayId, ArrayKind, NumberingScheme};
use signex_sketch::constraint::ConstraintKind;
use signex_sketch::entity::EntityKind;
use signex_sketch::id::SketchEntityId;

use super::support::{FpRig, SketchBuilder, near, sketch_rig};

fn canvas(rig: &mut FpRig, msg: SketchCanvasMsg) {
    rig.send(FootprintEditorMsg::SketchCanvas(msg));
}

fn with_array(kind: impl FnOnce(SketchEntityId) -> ArrayKind) -> (FpRig, ArrayId) {
    let mut s = SketchBuilder::new();
    let source = s.point(5.0, 0.0);
    let id = ArrayId::new();
    s.data.arrays.push(Array {
        id,
        kind: kind(source),
        numbering: NumberingScheme::default(),
    });
    (sketch_rig(s.data), id)
}

fn grid(source: SketchEntityId) -> ArrayKind {
    ArrayKind::Grid {
        source,
        nx_expr: "3".into(),
        ny_expr: "2".into(),
        dx_expr: "1mm".into(),
        dy_expr: "1mm".into(),
        depopulation: None,
    }
}

fn linear(source: SketchEntityId) -> ArrayKind {
    ArrayKind::Linear {
        source,
        count_expr: "4".into(),
        dx_expr: "1mm".into(),
        dy_expr: "0mm".into(),
    }
}

fn array_kind(rig: &FpRig, id: ArrayId) -> &ArrayKind {
    &rig.sketch()
        .arrays
        .iter()
        .find(|a| a.id == id)
        .expect("the pattern")
        .kind
}

fn count_of(rig: &FpRig, id: ArrayId) -> String {
    match array_kind(rig, id) {
        ArrayKind::Linear { count_expr, .. } => count_expr.clone(),
        other => panic!("a linear pattern: {other:?}"),
    }
}

fn type_count(rig: &mut FpRig, id: ArrayId, text: &str) {
    canvas(
        rig,
        SketchCanvasMsg::ValueEditOpened(ValueTarget::Pattern(id, PatternField::Count)),
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

#[test]
fn m191_linear_pattern_adds_one_pattern_and_undo_removes_it() {
    let mut s = SketchBuilder::new();
    let source = s.point(1.0, 1.0);
    let mut rig = sketch_rig(s.data);
    rig.arm(SketchTool::LinearPattern);

    rig.tool_click(1.0, 1.0, Some(source));

    assert_eq!(rig.sketch().arrays.len(), 1);
    assert!(matches!(
        &rig.sketch().arrays[0].kind,
        ArrayKind::Linear { source: from, .. } if *from == source
    ));
    assert!(rig.dirty());
    rig.undo();
    assert!(rig.sketch().arrays.is_empty());
    assert!(!rig.dirty());
}

#[test]
fn m191_a_linear_pitch_that_is_not_a_length_adds_no_pattern() {
    let mut s = SketchBuilder::new();
    let source = s.point(1.0, 1.0);
    let mut rig = sketch_rig(s.data);
    rig.arm(SketchTool::LinearPattern);
    rig.send(FootprintEditorMsg::SketchDimensionInput("-2".into()));

    rig.tool_click(1.0, 1.0, Some(source));

    assert!(rig.sketch().arrays.is_empty());
}

#[test]
fn m191_rectangular_pattern_adds_a_grid() {
    let mut s = SketchBuilder::new();
    let source = s.point(1.0, 1.0);
    let mut rig = sketch_rig(s.data);
    rig.arm(SketchTool::RectPattern);

    rig.tool_click(1.0, 1.0, Some(source));

    assert_eq!(rig.sketch().arrays.len(), 1);
    assert!(matches!(
        rig.sketch().arrays[0].kind,
        ArrayKind::Grid { .. }
    ));
}

#[test]
fn m191_mirror_about_a_centerline_keeps_the_copy_linked() {
    let mut s = SketchBuilder::new();
    let (c0, c1) = (s.point(0.0, -5.0), s.point(0.0, 5.0));
    let centerline = s.line(c0, c1);
    s.entity_mut(centerline).centerline = true;
    let source = s.point(3.0, 1.0);
    let mut rig = sketch_rig(s.data);
    rig.arm(SketchTool::Mirror);

    rig.tool_click(0.05, 2.0, None);
    rig.tool_click(3.0, 1.0, Some(source));

    let image = rig
        .sketch()
        .constraints
        .iter()
        .find_map(|c| match c.kind {
            ConstraintKind::SymmetricAboutLine { p1, p2, line }
                if p1 == source && line == centerline =>
            {
                Some(p2)
            }
            _ => None,
        })
        .expect("the copy is kept symmetric");
    let at = rig
        .sketch()
        .entities
        .iter()
        .find_map(|e| match e.kind {
            EntityKind::Point { x, y } if e.id == image => Some((x, y)),
            _ => None,
        })
        .expect("the mirrored point");
    assert!(near(at.0, -3.0) && near(at.1, 1.0), "{at:?}");
}

#[test]
fn m191_mirror_with_no_line_adds_nothing() {
    let mut s = SketchBuilder::new();
    let source = s.point(3.0, 1.0);
    let mut rig = sketch_rig(s.data);
    let entities = rig.sketch().entities.len();
    rig.arm(SketchTool::Mirror);

    rig.tool_click(3.0, 1.0, Some(source));

    assert_eq!(rig.sketch().entities.len(), entities);
    assert!(rig.sketch().constraints.is_empty());
}

#[test]
fn m191_a_count_typed_on_the_drawing_is_written_and_undo_restores_it() {
    let (mut rig, id) = with_array(linear);

    type_count(&mut rig, id, "6");
    canvas(&mut rig, SketchCanvasMsg::ValueCommitted);

    assert_eq!(count_of(&rig, id), "6");
    assert!(rig.dirty());
    rig.undo();
    assert_eq!(count_of(&rig, id), "4");
    assert!(!rig.dirty());
}

#[test]
fn m191_a_count_that_is_not_a_count_is_refused_and_stays_open() {
    let (mut rig, id) = with_array(linear);

    type_count(&mut rig, id, "2mm");
    canvas(&mut rig, SketchCanvasMsg::ValueCommitted);

    assert_eq!(count_of(&rig, id), "4");
    let edit = rig.editor().state.sketch_overlay.value_edit.as_ref();
    assert!(edit.is_some_and(|e| e.invalid), "{edit:?}");
    assert!(!rig.dirty());
}

#[test]
fn m191_a_grid_instance_click_suppresses_it_and_a_second_brings_it_back() {
    let (mut rig, id) = with_array(grid);
    let toggle = SketchCanvasMsg::InstanceSuppressionToggled {
        array: id,
        i: 1,
        j: 0,
    };

    canvas(&mut rig, toggle.clone());
    assert!(array_kind(&rig, id).is_instance_suppressed(1, 0));
    assert!(rig.dirty());

    canvas(&mut rig, toggle);
    assert!(!array_kind(&rig, id).is_instance_suppressed(1, 0));
}

#[test]
fn m191_undo_brings_a_suppressed_instance_back() {
    let (mut rig, id) = with_array(grid);
    canvas(
        &mut rig,
        SketchCanvasMsg::InstanceSuppressionToggled {
            array: id,
            i: 1,
            j: 0,
        },
    );

    rig.undo();

    assert!(!array_kind(&rig, id).is_instance_suppressed(1, 0));
    assert!(!rig.dirty());
}

#[test]
fn m191_a_linear_instance_does_not_toggle() {
    let (mut rig, id) = with_array(linear);
    let steps = rig.steps();

    canvas(
        &mut rig,
        SketchCanvasMsg::InstanceSuppressionToggled {
            array: id,
            i: 1,
            j: 0,
        },
    );

    assert!(!array_kind(&rig, id).is_instance_suppressed(1, 0));
    assert_eq!(rig.steps(), steps);
    assert!(!rig.dirty());
}

#[test]
fn m191_delete_pattern_keeps_the_source_and_undo_restores_the_pattern() {
    let (mut rig, id) = with_array(linear);
    let entities = rig.sketch().entities.len();

    canvas(&mut rig, SketchCanvasMsg::PatternDeleted(id));
    assert!(rig.sketch().arrays.is_empty());
    assert_eq!(rig.sketch().entities.len(), entities, "the source stays");

    rig.undo();
    assert_eq!(rig.sketch().arrays.len(), 1);
    assert!(!rig.dirty());
}
