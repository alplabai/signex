//! m188, constraints: adding one from the selection and the one constraint
//! menu (Convert to reference, Make driving, Select constrained entities,
//! Delete). Suppress stays greyed until a flag is stored (#2687).

use signex_app::library::editor::footprint::sketch_mode::SketchCanvasMsg;
use signex_app::library::editor::footprint::sketch_mode::constraint_menu::SketchReferenceMsg;
use signex_app::library::messages::{FootprintEditorMsg, SketchConstraintTag};
use signex_sketch::constraint::{Constraint, ConstraintKind, DimTarget};
use signex_sketch::id::{ConstraintId, SketchEntityId};

use super::support::{FpRig, SketchBuilder, sketch_rig};

struct Fixture {
    rig: FpRig,
    point: SketchEntityId,
    line: SketchEntityId,
    dimension: ConstraintId,
}

/// A free line, and a point `dimension` holds 3 mm from a fixed origin.
fn fixture() -> Fixture {
    let mut s = SketchBuilder::new();
    let (a, b) = (s.point(10.0, 10.0), s.point(13.0, 11.0));
    let line = s.line(a, b);
    let (origin, point) = (s.point(0.0, 0.0), s.point(3.0, 0.0));
    s.data.constraints.push(Constraint {
        id: ConstraintId::new(),
        kind: ConstraintKind::Fixed { point: origin },
    });
    let dimension = ConstraintId::new();
    s.data.constraints.push(Constraint {
        id: dimension,
        kind: ConstraintKind::DistancePtPt {
            p1: origin,
            p2: point,
            target: DimTarget::Literal(3.0),
        },
    });
    Fixture {
        rig: sketch_rig(s.data),
        point,
        line,
        dimension,
    }
}

fn select(rig: &mut FpRig, id: SketchEntityId) {
    rig.send(FootprintEditorMsg::SketchSelect {
        id: Some(id),
        shift: false,
    });
}

fn canvas(rig: &mut FpRig, msg: SketchCanvasMsg) {
    rig.send(FootprintEditorMsg::SketchCanvas(msg));
}

fn horizontals(rig: &FpRig) -> usize {
    rig.sketch()
        .constraints
        .iter()
        .filter(|c| matches!(c.kind, ConstraintKind::Horizontal { .. }))
        .count()
}

#[test]
fn m188_horizontal_on_a_selected_line_adds_the_constraint_and_undo_removes_it() {
    let mut f = fixture();
    select(&mut f.rig, f.line);

    f.rig
        .send(FootprintEditorMsg::SketchAddConstraintForSelection(
            SketchConstraintTag::Horizontal,
        ));
    assert_eq!(horizontals(&f.rig), 1);
    assert!(f.rig.dirty());

    f.rig.undo();
    assert_eq!(horizontals(&f.rig), 0);
    assert!(!f.rig.dirty());
}

/// That a refused edit leaves no undo step is the audit's "a bad value is
/// refused"; the row does not spell it out, so it is an inference.
#[test]
fn m188_a_constraint_the_selection_cannot_take_is_refused_and_records_nothing() {
    let mut f = fixture();
    select(&mut f.rig, f.point);
    let (before, steps) = (f.rig.sketch().clone(), f.rig.steps());

    f.rig
        .send(FootprintEditorMsg::SketchAddConstraintForSelection(
            SketchConstraintTag::Horizontal,
        ));

    assert_eq!(f.rig.sketch(), &before);
    assert!(
        !f.rig.editor().state.solve_warnings.is_empty(),
        "the refusal is said"
    );
    assert!(!f.rig.dirty());
    assert_eq!(f.rig.steps(), steps, "a refused constraint is no undo step");
}

#[test]
fn m188_the_menu_opens_on_a_constraint_and_closes_without_an_edit() {
    let mut f = fixture();

    canvas(
        &mut f.rig,
        SketchCanvasMsg::ConstraintMenuOpened {
            constraint: f.dimension,
            x: 40.0,
            y: 40.0,
        },
    );
    assert!(f.rig.editor().state.sketch_overlay.menu.is_some());

    canvas(&mut f.rig, SketchCanvasMsg::MenuClosed);
    assert!(f.rig.editor().state.sketch_overlay.menu.is_none());
    assert!(!f.rig.dirty());
}

#[test]
fn m188_convert_to_reference_and_make_driving_are_undo_steps() {
    let mut f = fixture();
    let steps = f.rig.steps();

    f.rig.send(FootprintEditorMsg::SketchReference(
        SketchReferenceMsg::ReferenceConverted(f.dimension),
    ));
    assert!(f.rig.sketch().is_driven(f.dimension));
    assert_eq!(f.rig.steps(), steps + 1);

    f.rig.send(FootprintEditorMsg::SketchReference(
        SketchReferenceMsg::DrivingRestored(f.dimension),
    ));
    assert!(!f.rig.sketch().is_driven(f.dimension));
    assert_eq!(f.rig.steps(), steps + 2);

    f.rig.undo();
    assert!(f.rig.sketch().is_driven(f.dimension));
}

#[test]
fn m188_a_reference_dimension_refuses_a_value_edit() {
    let mut f = fixture();
    f.rig.send(FootprintEditorMsg::SketchReference(
        SketchReferenceMsg::ReferenceConverted(f.dimension),
    ));

    canvas(
        &mut f.rig,
        SketchCanvasMsg::ValueEditOpened(
            signex_app::library::editor::footprint::ValueTarget::Constraint(f.dimension),
        ),
    );

    assert!(f.rig.editor().state.sketch_overlay.value_edit.is_none());
}

#[test]
fn m188_select_constrained_entities_selects_what_the_constraint_names() {
    let mut f = fixture();

    canvas(
        &mut f.rig,
        SketchCanvasMsg::ConstrainedEntitiesSelected(f.dimension),
    );

    let state = &f.rig.editor().state;
    let selected = [state.selected_sketch, state.selected_sketch_secondary];
    assert!(selected.contains(&Some(f.point)), "{selected:?}");
    assert!(!f.rig.dirty());
}

#[test]
fn m188_delete_removes_the_constraint_and_undo_brings_it_back() {
    let mut f = fixture();
    let has = |rig: &FpRig, id: ConstraintId| rig.sketch().constraints.iter().any(|c| c.id == id);

    canvas(&mut f.rig, SketchCanvasMsg::ConstraintDeleted(f.dimension));
    assert!(!has(&f.rig, f.dimension));
    assert!(f.rig.dirty());

    f.rig.undo();
    assert!(has(&f.rig, f.dimension));
    assert!(!f.rig.dirty());
}
