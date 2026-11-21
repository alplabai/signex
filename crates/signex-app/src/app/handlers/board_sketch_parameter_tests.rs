//! Caner, 2026-11-21: Make parameter on a literal angle in a board sketch
//! keeps the number the user reads, and the geometry, through
//! `Signex::update`. A footprint sketch's parameter is as it was.

use signex_sketch::SketchData;
use signex_sketch::constraint::{Constraint, ConstraintKind, DimTarget};
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::{ConstraintId, SketchEntityId};
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};

use super::tests::{
    board_sketch_data, footprint_sketch_data, footprint_sketching, near, send, sketch_edit,
    sketching_off_grid,
};
use crate::app::{EditMsg, FootprintDocument, Message, Signex};
use crate::library::editor::footprint::parameters_table::SketchParametersMsg;
use crate::library::editor::footprint::sketch_dispatch::apply_sketch_edit_with_warnings;
use crate::library::editor::footprint::sketch_mode::value_edit::{
    constraint_buffer, dimension_label, resolved_params,
};
use crate::library::editor::footprint::sketch_mode::{SketchCanvasMsg, SketchEdit};
use crate::library::editor::footprint::snap::point_pos;
use crate::library::messages::FootprintEditorMsg;

/// A fixed base line from (10, 10) to (20, 10) and a 10 mm arm from (10, 10)
/// held by a literal angle of `model_deg` in the model's axes.
struct AngleSketch {
    data: SketchData,
    arm_end: SketchEntityId,
    angle: ConstraintId,
}

fn angle_sketch(model_deg: f64) -> AngleSketch {
    let plane = PlaneId::new();
    let mut data = SketchData::default();
    data.planes.push(Plane {
        id: plane,
        kind: PlaneKind::BoardTop,
    });
    let rad = model_deg.to_radians();
    let mut add = |kind| {
        let id = SketchEntityId::new();
        data.entities.push(Entity::new(id, plane, kind));
        id
    };
    let p0 = add(EntityKind::Point { x: 10.0, y: 10.0 });
    let p1 = add(EntityKind::Point { x: 20.0, y: 10.0 });
    let arm_end = add(EntityKind::Point {
        x: 10.0 + 10.0 * rad.cos(),
        y: 10.0 + 10.0 * rad.sin(),
    });
    let l1 = add(EntityKind::Line { start: p0, end: p1 });
    let l2 = add(EntityKind::Line {
        start: p0,
        end: arm_end,
    });
    let angle = ConstraintId::new();
    let kinds = [
        (ConstraintId::new(), ConstraintKind::Fixed { point: p0 }),
        (ConstraintId::new(), ConstraintKind::Fixed { point: p1 }),
        (
            ConstraintId::new(),
            ConstraintKind::DistancePtPt {
                p1: p0,
                p2: arm_end,
                target: DimTarget::Literal(10.0),
            },
        ),
        (
            angle,
            ConstraintKind::Angle {
                l1,
                l2,
                target: DimTarget::Literal(rad),
            },
        ),
    ];
    data.constraints
        .extend(kinds.map(|(id, kind)| Constraint { id, kind }));
    AngleSketch {
        data,
        arm_end,
        angle,
    }
}

fn install(editor: &mut FootprintDocument, data: SketchData) {
    editor.primitive_mut().sketch = Some(data);
    editor.with_parts(|state, primitive| {
        apply_sketch_edit_with_warnings(state, primitive, SketchEdit::ForceRebuild);
    });
    editor.mark_saved();
}

fn on_board(model_deg: f64) -> (Signex, tempfile::TempDir, std::path::PathBuf, AngleSketch) {
    let (mut app, tmp) = sketching_off_grid();
    let path = app.active_tab_path().expect("the board tab");
    let sketch = angle_sketch(model_deg);
    let session = app.interaction_state.board_sketch.as_mut().expect("on");
    install(&mut session.editor, sketch.data.clone());
    (app, tmp, path, sketch)
}

fn make_parameter(app: &mut Signex, path: &std::path::Path, id: ConstraintId) {
    sketch_edit(
        app,
        path,
        FootprintEditorMsg::SketchCanvas(SketchCanvasMsg::ParameterRequested(id)),
    );
}

fn angle_target(sketch: &SketchData, id: ConstraintId) -> DimTarget {
    let constraint = sketch.constraints.iter().find(|c| c.id == id);
    match &constraint.expect("the angle").kind {
        ConstraintKind::Angle { target, .. } => target.clone(),
        other => panic!("not an angle: {other:?}"),
    }
}

fn solved_board_point(app: &Signex, id: SketchEntityId) -> (f64, f64) {
    let session = app.interaction_state.board_sketch.as_ref().expect("on");
    let sketch = session.working_sketch();
    point_pos(id, Some(&sketch), &session.editor.state).expect("the point")
}

/// Stored -30 degrees reads 30 on a board: the parameter is 30 and the
/// dimension reads its negative.
#[test]
fn make_parameter_keeps_a_board_angle_its_geometry_and_one_undo_step() {
    let (mut app, _tmp, path, sketch) = on_board(-30.0);
    let before = board_sketch_data(&app);
    let end = solved_board_point(&app, sketch.arm_end);
    make_parameter(&mut app, &path, sketch.angle);

    let made = board_sketch_data(&app);
    assert_eq!(made.parameters.get_raw("angle_1"), Some("30deg"));
    assert_eq!(
        angle_target(&made, sketch.angle),
        DimTarget::Expr("-angle_1".into())
    );
    let after = solved_board_point(&app, sketch.arm_end);
    assert!(near(after.0, end.0) && near(after.1, end.1), "{after:?}");

    {
        let session = app.interaction_state.board_sketch.as_ref().expect("on");
        let state = &session.editor.state;
        let kind = &made
            .constraints
            .iter()
            .find(|c| c.id == sketch.angle)
            .expect("the angle")
            .kind;
        let shown = state.shown_constraint(kind);
        assert_eq!(constraint_buffer(&shown).as_deref(), Some("angle_1"));
        let params = resolved_params(&made, state);
        let (name, value) =
            dimension_label(&shown, &made, &params, &state.canvas_text).expect("a label");
        assert_eq!(name.as_deref(), Some("angle_1"));
        assert!(value.starts_with("30"), "reads 30: {value}");
    }

    send(&mut app, Message::Edit(EditMsg::Undo));
    let undone = board_sketch_data(&app);
    assert_eq!(
        angle_target(&undone, sketch.angle),
        angle_target(&before, sketch.angle),
        "one undo restores the literal"
    );
    assert_eq!(undone.parameters.get_raw("angle_1"), None);
}

/// A larger parameter turns the arm further counter-clockwise on screen,
/// which is up: less y in the model.
#[test]
fn changing_the_parameter_turns_the_line_counter_clockwise_on_screen() {
    let (mut app, _tmp, path, sketch) = on_board(-30.0);
    make_parameter(&mut app, &path, sketch.angle);
    sketch_edit(
        &mut app,
        &path,
        FootprintEditorMsg::SketchParameters(SketchParametersMsg::ExpressionEdited {
            name: "angle_1".into(),
            expr: "60deg".into(),
        }),
    );
    let (x, y) = solved_board_point(&app, sketch.arm_end);
    let rad = 60.0_f64.to_radians();
    let close = |a: f64, b: f64| (a - b).abs() < 1e-3;
    assert!(close(x, 10.0 + 10.0 * rad.cos()), "{x}");
    assert!(close(y, 10.0 - 10.0 * rad.sin()), "{y}");
}

/// Entering the same name on a negated dimension keeps it negated.
#[test]
fn a_name_entered_on_a_mirrored_angle_stays_mirrored() {
    let (mut app, _tmp, path, sketch) = on_board(-30.0);
    make_parameter(&mut app, &path, sketch.angle);
    let made = board_sketch_data(&app);
    let kind = &made
        .constraints
        .iter()
        .find(|c| c.id == sketch.angle)
        .expect("the angle")
        .kind;
    let session = app.interaction_state.board_sketch.as_ref().expect("on");
    let stored = session
        .editor
        .state
        .stored_target(kind, DimTarget::Expr("angle_1".into()));
    assert_eq!(stored, DimTarget::Expr("-angle_1".into()));
}

#[test]
fn make_parameter_on_a_footprint_angle_is_unchanged() {
    let (mut app, path) = footprint_sketching();
    let sketch = angle_sketch(-30.0);
    let editor = app.document_state.footprint_editors.get_mut(&path);
    install(editor.expect("the editor"), sketch.data.clone());
    make_parameter(&mut app, &path, sketch.angle);
    let made = footprint_sketch_data(&app, &path);
    assert_eq!(made.parameters.get_raw("angle_1"), Some("-30deg"));
    assert_eq!(
        angle_target(&made, sketch.angle),
        DimTarget::Expr("angle_1".into())
    );
}
