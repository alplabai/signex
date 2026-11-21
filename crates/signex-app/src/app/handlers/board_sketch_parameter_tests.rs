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
use crate::library::editor::footprint::ValueTarget;
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

/// Open the angle's value edit, replace its text with `text`, press Enter.
fn type_on(app: &mut Signex, path: &std::path::Path, id: ConstraintId, text: &str) {
    let mut canvas = |msg| sketch_edit(app, path, FootprintEditorMsg::SketchCanvas(msg));
    canvas(SketchCanvasMsg::ValueEditOpened(ValueTarget::Constraint(
        id,
    )));
    for _ in 0..32 {
        canvas(SketchCanvasMsg::ValueErased);
    }
    for ch in text.chars() {
        canvas(SketchCanvasMsg::ValueTyped(ch));
    }
    canvas(SketchCanvasMsg::ValueCommitted);
}

/// The angle's edit text and its value on the drawing.
fn reads(app: &Signex, id: ConstraintId) -> (String, String) {
    let session = app.interaction_state.board_sketch.as_ref().expect("on");
    let state = &session.editor.state;
    let data = session.working_sketch();
    let kind = &data
        .constraints
        .iter()
        .find(|c| c.id == id)
        .expect("the angle")
        .kind;
    let shown = state.shown_constraint(kind);
    let params = resolved_params(&data, state);
    let (_, value) = dimension_label(&shown, &data, &params, &state.canvas_text).expect("a label");
    (constraint_buffer(&shown).expect("a value"), value)
}

fn with_parameters(app: &mut Signex, sketch: &AngleSketch) {
    let mut data = sketch.data.clone();
    for (name, value) in [
        ("slope", "25deg"),
        ("angle_2", "20deg"),
        ("a", "10deg"),
        ("b", "15deg"),
    ] {
        data.parameters.insert(name, value);
    }
    let session = app.interaction_state.board_sketch.as_mut().expect("on");
    install(&mut session.editor, data);
}

/// `data` written into the board's `.snxpcb` text and read back.
fn through_the_file(app: &Signex, path: &std::path::Path, data: &SketchData) -> SketchData {
    let mut board = app.document_state.pcb_board(path).expect("board").clone();
    board.sketch = Some(signex_types::pcb_sketch::BoardSketch {
        sketch: data.clone(),
        ..Default::default()
    });
    let text = signex_types::format::SnxPcb::new(board)
        .write_string()
        .expect("written");
    let read = signex_types::format::SnxPcb::parse(&text).expect("read");
    read.board.sketch.expect("the sketch").sketch
}

/// The arm is at `degrees` counter-clockwise on screen, so up is less y.
fn assert_arm_at(app: &Signex, sketch: &AngleSketch, degrees: f64, what: &str) {
    let (x, y) = solved_board_point(app, sketch.arm_end);
    let rad = degrees.to_radians();
    let close = |a: f64, b: f64| (a - b).abs() < 1e-3;
    assert!(close(x, 10.0 + 10.0 * rad.cos()), "{what}: x {x}");
    assert!(close(y, 10.0 - 10.0 * rad.sin()), "{what}: y {y}");
}

/// Whatever is typed on a board angle means that value Y-up: it reads back,
/// turns the line that way on screen, survives the file and undoes in one
/// step. `-(-slope)` is `slope` in its plain form.
#[test]
fn a_typed_expression_keeps_the_sense_it_shows() {
    for (typed, reads_as, degrees) in [
        ("30", "30", 30.0_f64),
        ("30deg", "30deg", 30.0),
        ("-30deg", "-30deg", -30.0),
        ("slope", "slope", 25.0),
        ("-slope", "-slope", -25.0),
        ("-(-slope)", "slope", 25.0),
        ("-angle_2", "-angle_2", -20.0),
        ("(a+b)", "(a+b)", 25.0),
        ("-a+b", "-a+b", 5.0),
    ] {
        let (mut app, _tmp, path, sketch) = on_board(-10.0);
        with_parameters(&mut app, &sketch);
        let before = angle_target(&board_sketch_data(&app), sketch.angle);
        type_on(&mut app, &path, sketch.angle, typed);

        let (text, value) = reads(&app, sketch.angle);
        assert_eq!(text, reads_as, "{typed}");
        assert!(
            value.starts_with(&format!("{degrees:.0}")),
            "{typed}: {value}"
        );
        assert_arm_at(&app, &sketch, degrees, typed);

        let saved = board_sketch_data(&app);
        let loaded = through_the_file(&app, &path, &saved);
        assert_eq!(
            angle_target(&loaded, sketch.angle),
            angle_target(&saved, sketch.angle),
            "{typed}"
        );
        let session = app.interaction_state.board_sketch.as_mut().expect("on");
        install(&mut session.editor, loaded);
        assert_eq!(reads(&app, sketch.angle).0, reads_as, "{typed} reloaded");
        assert_arm_at(&app, &sketch, degrees, typed);
        let session = app.interaction_state.board_sketch.as_mut().expect("on");
        install(&mut session.editor, saved);

        type_on(&mut app, &path, sketch.angle, typed);
        assert_eq!(reads(&app, sketch.angle).0, reads_as, "{typed} retyped");

        send(&mut app, Message::Edit(EditMsg::Undo));
        let undone = board_sketch_data(&app);
        assert_eq!(angle_target(&undone, sketch.angle), before, "{typed}");
    }
}

/// Typing what a bound angle already shows is no edit: the one undo step
/// that follows is the Make parameter before it.
#[test]
fn the_same_number_on_a_bound_angle_changes_nothing() {
    let (mut app, _tmp, path, sketch) = on_board(-30.0);
    let literal = angle_target(&board_sketch_data(&app), sketch.angle);
    make_parameter(&mut app, &path, sketch.angle);
    let bound = board_sketch_data(&app);
    type_on(&mut app, &path, sketch.angle, "30");
    assert_eq!(board_sketch_data(&app), bound);
    send(&mut app, Message::Edit(EditMsg::Undo));
    assert_eq!(
        angle_target(&board_sketch_data(&app), sketch.angle),
        literal
    );
}

/// An older dimension stored as bare `slope = 30deg`, at model +30.
fn on_board_bare_slope() -> (Signex, tempfile::TempDir, std::path::PathBuf, AngleSketch) {
    let (mut app, tmp, path, sketch) = on_board(30.0);
    let mut data = sketch.data.clone();
    data.parameters.insert("slope", "30deg");
    for constraint in &mut data.constraints {
        if constraint.id == sketch.angle
            && let ConstraintKind::Angle { target, .. } = &mut constraint.kind
        {
            *target = DimTarget::Expr("slope".into());
        }
    }
    let session = app.interaction_state.board_sketch.as_mut().expect("on");
    install(&mut session.editor, data);
    (app, tmp, path, sketch)
}

/// An older bare `slope` opens as it is stored and reads its true on-screen
/// value, -30.
#[test]
fn an_old_bare_slope_opens_unchanged_and_reads_y_up() {
    let (app, _tmp, _path, sketch) = on_board_bare_slope();
    assert_eq!(
        angle_target(&board_sketch_data(&app), sketch.angle),
        DimTarget::Expr("slope".into())
    );
    assert_arm_at(&app, &sketch, -30.0, "opened");
    let (text, value) = reads(&app, sketch.angle);
    assert_eq!(text, "-slope");
    assert!(value.starts_with("-30"), "{value}");
}

/// What it shows, in any spelling, is no edit; a new number is read Y-up, so
/// -31 moves it one degree from the -30 it reads.
#[test]
fn a_plain_bound_angle_keeps_its_number_and_takes_a_new_one_y_up() {
    let (mut app, _tmp, path, sketch) = on_board_bare_slope();
    let bound = board_sketch_data(&app);
    for same in ["-30", "-30deg", "-slope"] {
        type_on(&mut app, &path, sketch.angle, same);
        assert_eq!(board_sketch_data(&app), bound, "{same}");
    }
    type_on(&mut app, &path, sketch.angle, "-31");
    assert_eq!(
        angle_target(&board_sketch_data(&app), sketch.angle),
        DimTarget::Literal(0.0 - (-31.0_f64).to_radians())
    );
    assert_arm_at(&app, &sketch, -31.0, "typed -31");
}

/// A stored 30.00004 displays as 30, but typing 30 cleans it.
#[test]
fn typing_the_rounded_number_cleans_a_stored_angle() {
    let (mut app, _tmp, path, sketch) = on_board(-30.00004);
    assert_eq!(reads(&app, sketch.angle).0, "30");
    type_on(&mut app, &path, sketch.angle, "30");
    assert_eq!(
        angle_target(&board_sketch_data(&app), sketch.angle),
        DimTarget::Literal(0.0 - 30.0_f64.to_radians())
    );
}

/// Make parameter keeps the number on what an expression shows.
#[test]
fn make_parameter_on_a_typed_expression_keeps_the_number() {
    for (typed, degrees) in [("30deg", 30.0_f64), ("a+b", 25.0)] {
        let (mut app, _tmp, path, sketch) = on_board(-10.0);
        with_parameters(&mut app, &sketch);
        type_on(&mut app, &path, sketch.angle, typed);
        make_parameter(&mut app, &path, sketch.angle);

        let made = board_sketch_data(&app);
        assert_eq!(made.parameters.get_raw("angle_1"), Some(typed));
        assert_eq!(
            angle_target(&made, sketch.angle),
            DimTarget::Expr("-angle_1".into())
        );
        let (text, value) = reads(&app, sketch.angle);
        assert_eq!(text, "angle_1");
        assert!(value.starts_with(&format!("{degrees:.0}")), "{value}");
        assert_arm_at(&app, &sketch, degrees, typed);
    }
}

/// Replace with value keeps the number shown and the geometry.
#[test]
fn inlining_a_parameter_keeps_the_number_shown() {
    let (mut app, _tmp, path, sketch) = on_board(-30.0);
    make_parameter(&mut app, &path, sketch.angle);
    let (_, shown) = reads(&app, sketch.angle);
    for msg in [
        SketchParametersMsg::DeleteRequested {
            name: "angle_1".into(),
        },
        SketchParametersMsg::DeleteConfirmed,
    ] {
        sketch_edit(&mut app, &path, FootprintEditorMsg::SketchParameters(msg));
    }
    assert_eq!(board_sketch_data(&app).parameters.get_raw("angle_1"), None);
    assert_eq!(reads(&app, sketch.angle).1, shown);
    assert_arm_at(&app, &sketch, 30.0, "inlined");
}

/// A footprint sketch stores every form exactly as typed.
#[test]
fn a_typed_expression_on_a_footprint_angle_is_stored_as_typed() {
    for typed in ["30deg", "-30deg", "slope", "-slope", "-(-slope)", "(a+b)", "-a+b"] {
        let (mut app, path) = footprint_sketching();
        let mut sketch = angle_sketch(-10.0);
        for (name, value) in [("slope", "25deg"), ("a", "10deg"), ("b", "15deg")] {
            sketch.data.parameters.insert(name, value);
        }
        let editor = app.document_state.footprint_editors.get_mut(&path);
        install(editor.expect("the editor"), sketch.data.clone());
        type_on(&mut app, &path, sketch.angle, typed);
        assert_eq!(
            angle_target(&footprint_sketch_data(&app, &path), sketch.angle),
            DimTarget::Expr(typed.into())
        );
    }
    let (mut app, path) = footprint_sketching();
    let sketch = angle_sketch(-10.0);
    let editor = app.document_state.footprint_editors.get_mut(&path);
    install(editor.expect("the editor"), sketch.data.clone());
    type_on(&mut app, &path, sketch.angle, "30");
    assert_eq!(
        angle_target(&footprint_sketch_data(&app, &path), sketch.angle),
        DimTarget::Literal(30.0_f64.to_radians())
    );
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
