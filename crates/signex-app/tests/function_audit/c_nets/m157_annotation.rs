//! Mockup row 157: the Bezier and dimension tools. Both are annotation: they
//! place one undo step each and never join a net. Their point, style and unit
//! controls take a type the crate does not export (see the row's report).

use signex_app::active_bar::ActiveBarAction;
use signex_app::app::Tool;
use signex_app::panels::{AnnotationField, AnnotationPanelMsg, PanelMsg};
use signex_types::schematic::{DimensionDirection, Point, SchDimension, SchDrawing, SelectedKind};
use uuid::Uuid;

use super::support::{Bench, bench, empty_sheet};

const CURVE: [(f64, f64); 4] = [(10.16, 10.16), (12.7, 20.32), (25.4, 20.32), (27.94, 10.16)];

#[test]
fn m157_four_clicks_place_one_curve_with_no_colour_of_its_own() {
    let mut b = bench(empty_sheet());
    b.arm(ActiveBarAction::DrawBezier);
    assert_eq!(b.app.interaction_state.current_tool, Tool::Bezier);
    for (i, (x, y)) in CURVE.iter().enumerate() {
        assert_eq!(b.sheet().drawings.len(), 0, "nothing before click {i}");
        b.click(*x, *y);
    }
    let [SchDrawing::Bezier(curve)] = b.sheet().drawings.as_slice() else {
        panic!("one curve, got {:?}", b.sheet().drawings);
    };
    assert_eq!(curve.start, Point::new(10.16, 10.16));
    assert_eq!(curve.c1, Point::new(12.7, 20.32));
    assert_eq!(curve.c2, Point::new(25.4, 20.32));
    assert_eq!(curve.end, Point::new(27.94, 10.16));
    assert!(curve.stroke_color.is_none(), "the sheet's drawing colour");
    assert!(b.dirty());
}

#[test]
fn m157_undo_takes_the_curve_away_in_one_step() {
    let mut b = bench(empty_sheet());
    b.arm(ActiveBarAction::DrawBezier);
    for (x, y) in CURVE {
        b.click(x, y);
    }
    b.undo();
    assert!(b.sheet().drawings.is_empty());
    assert!(!b.dirty());
    b.redo();
    assert_eq!(b.sheet().drawings.len(), 1);
}

#[test]
fn m157_a_curve_makes_no_connection() {
    let mut b = bench(empty_sheet());
    b.arm(ActiveBarAction::DrawBezier);
    for (x, y) in CURVE {
        b.click(x, y);
    }
    assert!(b.sheet().wires.is_empty());
    assert!(signex_net::build_netlist(b.sheet()).nets.is_empty());
}

fn place_dimension(b: &mut Bench, action: ActiveBarAction, clicks: [(f64, f64); 3]) {
    b.arm(action);
    for (x, y) in clicks {
        b.click(x, y);
    }
}

fn dimension(b: &Bench) -> &SchDimension {
    match b.sheet().drawings.as_slice() {
        [SchDrawing::Dimension(d)] => d,
        other => panic!("one dimension, got {other:?}"),
    }
}

#[test]
fn m157_a_linear_dimension_measures_along_the_drag_and_undo_removes_it() {
    let mut b = bench(empty_sheet());
    place_dimension(
        &mut b,
        ActiveBarAction::PlaceLinearDimension,
        [(10.0, 10.0), (22.7, 10.0), (16.0, 5.0)],
    );
    assert_eq!(dimension(&b).direction, DimensionDirection::Horizontal);
    assert!((dimension(&b).measured_mm() - 12.7).abs() < 1e-9);
    assert_eq!(dimension(&b).label(), "12.70 mm");
    assert!(b.dirty());
    b.undo();
    assert!(b.sheet().drawings.is_empty());
    assert!(!b.dirty());
}

#[test]
fn m157_an_aligned_dimension_measures_the_true_distance() {
    let mut b = bench(empty_sheet());
    place_dimension(
        &mut b,
        ActiveBarAction::PlaceAlignedDimension,
        [(0.0, 0.0), (3.0, 4.0), (5.0, 0.0)],
    );
    assert_eq!(dimension(&b).direction, DimensionDirection::Aligned);
    assert!((dimension(&b).measured_mm() - 5.0).abs() < 1e-9);
}

#[test]
fn m157_two_clicks_on_one_spot_measure_nothing_and_place_nothing() {
    let mut b = bench(empty_sheet());
    place_dimension(
        &mut b,
        ActiveBarAction::PlaceLinearDimension,
        [(10.0, 10.0), (10.0, 10.0), (16.0, 5.0)],
    );
    assert!(b.sheet().drawings.is_empty());
    assert!(!b.dirty());
}

fn selected_dimension() -> (Bench, Uuid) {
    let d = SchDimension::new(
        Uuid::now_v7(),
        Point::new(20.0, 20.0),
        Point::new(32.7, 20.0),
        5.0,
        DimensionDirection::Horizontal,
    );
    let id = d.uuid;
    let mut sheet = empty_sheet();
    sheet.drawings.push(SchDrawing::Dimension(d));
    let mut b = bench(sheet);
    b.select_everything();
    assert_eq!(
        b.app.document_state.panel_ctx.selected_kind,
        Some(SelectedKind::Drawing),
        "the dimension alone is selected"
    );
    (b, id)
}

#[test]
fn m157_override_text_replaces_the_shown_value_and_undo_restores_it() {
    let (mut b, _) = selected_dimension();
    let measured = dimension(&b).label();
    b.panel(PanelMsg::Annotation(AnnotationPanelMsg::FieldTyped(
        AnnotationField::OverrideText,
        "pitch".to_string(),
    )));
    assert_eq!(dimension(&b).override_text.as_deref(), Some("pitch"));
    assert_eq!(dimension(&b).label(), "pitch");
    assert!(b.dirty());
    b.undo();
    assert_eq!(dimension(&b).override_text, None);
    assert_eq!(dimension(&b).label(), measured);
    assert!(!b.dirty());
}

#[test]
fn m157_enter_in_a_box_drops_its_draft_and_keeps_the_value() {
    let (mut b, _) = selected_dimension();
    b.panel(PanelMsg::Annotation(AnnotationPanelMsg::FieldTyped(
        AnnotationField::OverrideText,
        "pitch".to_string(),
    )));
    assert!(b.app.document_state.panel_ctx.annotation.draft.is_some());
    b.panel(PanelMsg::Annotation(AnnotationPanelMsg::FieldSubmitted(
        AnnotationField::OverrideText,
    )));
    assert!(b.app.document_state.panel_ctx.annotation.draft.is_none());
    assert_eq!(dimension(&b).override_text.as_deref(), Some("pitch"));
}
