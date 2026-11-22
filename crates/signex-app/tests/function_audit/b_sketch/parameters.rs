//! m189, the sketch parameters table: open and close, Add, typing that
//! re-solves, an error that keeps the last solve, Rename, Delete with its
//! prompt, and Make parameter.

use signex_app::library::editor::footprint::parameters_table::SketchParametersMsg;
use signex_app::library::messages::FootprintEditorMsg;
use signex_sketch::constraint::{Constraint, ConstraintKind, DimTarget};
use signex_sketch::id::{ConstraintId, SketchEntityId};
use signex_sketch::parameter_refs::{ParameterNameError, RowValue, resolve_rows};
use signex_sketch::solver::state::point_xy;

use super::support::{FpRig, SketchBuilder, near, sketch_rig};

struct Fixture {
    rig: FpRig,
    b: SketchEntityId,
    driven: ConstraintId,
    literal: ConstraintId,
}

/// `a` fixed at the origin, `b` at the distance `w` (2 mm) from it, `c` a
/// literal 1 mm from it. The table is open.
fn fixture() -> Fixture {
    let mut s = SketchBuilder::new();
    let (a, b, c) = (s.point(0.0, 0.0), s.point(1.0, 0.0), s.point(0.0, 1.5));
    s.data.parameters.insert("w", "2mm");
    s.data.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Fixed { point: a },
    });
    let driven = ConstraintId::new();
    s.data.constraints.push(Constraint {
        id: driven,
        kind: ConstraintKind::DistancePtPt {
            p1: a,
            p2: b,
            target: DimTarget::Expr("w".into()),
        },
    });
    let literal = ConstraintId::new();
    s.data.constraints.push(Constraint {
        id: literal,
        kind: ConstraintKind::DistancePtPt {
            p1: a,
            p2: c,
            target: DimTarget::Literal(1.0),
        },
    });
    let mut rig = sketch_rig(s.data);
    table(&mut rig, SketchParametersMsg::TableToggled);
    Fixture {
        rig,
        b,
        driven,
        literal,
    }
}

fn table(rig: &mut FpRig, msg: SketchParametersMsg) {
    rig.send(FootprintEditorMsg::SketchParameters(msg));
}

fn parameter(rig: &FpRig, name: &str) -> Option<String> {
    rig.sketch().parameters.get_raw(name).map(str::to_string)
}

fn target(rig: &FpRig, id: ConstraintId) -> DimTarget {
    rig.sketch()
        .constraints
        .iter()
        .find(|c| c.id == id)
        .and_then(|c| c.kind.dim_target().cloned())
        .expect("the dimension")
}

/// How far the last applied solve puts `b` from the origin.
fn distance(rig: &FpRig, b: SketchEntityId) -> f64 {
    let solve = rig.editor().state.last_solve.as_ref().expect("solved");
    let (x, y) = point_xy(b, &solve.result.state, &solve.result.index, rig.sketch())
        .expect("the point solved");
    x.hypot(y)
}

fn type_expr(rig: &mut FpRig, expr: &str) {
    table(
        rig,
        SketchParametersMsg::ExpressionEdited {
            name: "w".into(),
            expr: expr.into(),
        },
    );
}

#[test]
fn m189_the_table_opens_and_closes_without_an_edit() {
    let mut f = fixture();
    assert!(f.rig.editor().state.parameters_table.is_some());

    table(&mut f.rig, SketchParametersMsg::TableClosed);

    assert!(f.rig.editor().state.parameters_table.is_none());
    assert!(!f.rig.dirty());
}

#[test]
fn m189_add_parameter_adds_a_row_and_undo_removes_it() {
    let mut f = fixture();

    table(&mut f.rig, SketchParametersMsg::AddClicked);
    assert_eq!(parameter(&f.rig, "param_1").as_deref(), Some("1mm"));
    assert!(f.rig.dirty());

    f.rig.undo();
    assert_eq!(parameter(&f.rig, "param_1"), None);
    assert!(!f.rig.dirty());
}

#[test]
fn m189_typing_an_expression_re_solves_the_drawing() {
    let mut f = fixture();
    assert!(near(distance(&f.rig, f.b), 2.0), "solved on entry");

    type_expr(&mut f.rig, "3mm");

    assert!(near(distance(&f.rig, f.b), 3.0));
    assert!(f.rig.dirty());
}

#[test]
fn m189_undo_of_a_typed_expression_restores_the_value() {
    let mut f = fixture();
    type_expr(&mut f.rig, "3mm");

    f.rig.undo();

    assert_eq!(parameter(&f.rig, "w").as_deref(), Some("2mm"));
    assert!(!f.rig.dirty());
}

#[test]
fn m189_an_unknown_name_keeps_the_last_good_solve_and_marks_the_row() {
    let mut f = fixture();
    type_expr(&mut f.rig, "3mm");

    type_expr(&mut f.rig, "nosuchname + 1mm");

    assert!(near(distance(&f.rig, f.b), 3.0), "the drawing stays");
    let rows = resolve_rows(&f.rig.sketch().parameters);
    assert!(matches!(rows["w"], RowValue::Error(_)));
    let open = f.rig.editor().state.parameters_table.as_ref().unwrap();
    let last = open.last_good.get("w").expect("the last value is kept");
    assert!(near(last.value, 3.0));
}

#[test]
fn m189_rename_updates_every_use_and_undo_restores_it() {
    let mut f = fixture();
    table(
        &mut f.rig,
        SketchParametersMsg::RenameStarted { name: "w".into() },
    );
    table(
        &mut f.rig,
        SketchParametersMsg::RenameDraftChanged("width".into()),
    );

    table(&mut f.rig, SketchParametersMsg::RenameSubmitted);

    assert_eq!(parameter(&f.rig, "w"), None);
    assert_eq!(target(&f.rig, f.driven), DimTarget::Expr("width".into()));
    f.rig.undo();
    assert_eq!(target(&f.rig, f.driven), DimTarget::Expr("w".into()));
}

#[test]
fn m189_a_reserved_name_is_refused_and_changes_nothing() {
    let mut f = fixture();
    table(
        &mut f.rig,
        SketchParametersMsg::RenameStarted { name: "w".into() },
    );
    table(
        &mut f.rig,
        SketchParametersMsg::RenameDraftChanged("pi".into()),
    );
    let (before, steps) = (f.rig.sketch().clone(), f.rig.steps());

    table(&mut f.rig, SketchParametersMsg::RenameSubmitted);

    assert_eq!(f.rig.sketch(), &before);
    assert_eq!(f.rig.steps(), steps);
    let open = f.rig.editor().state.parameters_table.as_ref().unwrap();
    let rename = open.rename.as_ref().expect("still renaming");
    assert_eq!(
        rename.error,
        Some(ParameterNameError::Reserved("pi".into()))
    );
}

#[test]
fn m189_cancel_on_the_delete_prompt_keeps_a_used_parameter() {
    let mut f = fixture();

    table(
        &mut f.rig,
        SketchParametersMsg::DeleteRequested { name: "w".into() },
    );
    let open = f.rig.editor().state.parameters_table.as_ref().unwrap();
    assert_eq!(open.delete_prompt.as_deref(), Some("w"), "asks first");
    table(&mut f.rig, SketchParametersMsg::DeleteCancelled);

    assert!(parameter(&f.rig, "w").is_some());
    assert!(!f.rig.dirty());
}

#[test]
fn m189_replace_with_value_inlines_the_value_and_undo_restores_it() {
    let mut f = fixture();
    table(
        &mut f.rig,
        SketchParametersMsg::DeleteRequested { name: "w".into() },
    );

    table(&mut f.rig, SketchParametersMsg::DeleteConfirmed);

    assert_eq!(parameter(&f.rig, "w"), None);
    assert_ne!(target(&f.rig, f.driven), DimTarget::Expr("w".into()));
    assert!(near(distance(&f.rig, f.b), 2.0), "the drawing is unchanged");
    f.rig.undo();
    assert_eq!(parameter(&f.rig, "w").as_deref(), Some("2mm"));
    assert_eq!(target(&f.rig, f.driven), DimTarget::Expr("w".into()));
}

#[test]
fn m189_make_parameter_binds_a_dimension_and_undo_restores_the_literal() {
    let mut f = fixture();

    table(
        &mut f.rig,
        SketchParametersMsg::ParameterFromConstraintRequested {
            constraint: f.literal,
        },
    );

    assert!(
        matches!(target(&f.rig, f.literal), DimTarget::Expr(_)),
        "the dimension reads a parameter"
    );
    f.rig.undo();
    assert_eq!(target(&f.rig, f.literal), DimTarget::Literal(1.0));
}
