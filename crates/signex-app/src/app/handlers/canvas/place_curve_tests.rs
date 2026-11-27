//! The Ellipse, Elliptical Arc and Round Rectangle tools on a sheet (#1267):
//! armed from their command, placed by their clicks as one undo step, with
//! a preview and a hint while clicks are missing.

use signex_types::schematic::{CurveShape, CurveTool, Point, SchDrawing};

use super::placement_tests::{app_with_tool, click, doc, grid};
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::{Signex, Tool};
use crate::canvas::AnnotationDraftKind;
use crate::keymap::AppCommandId;

fn run(app: &mut Signex, id: &str) -> CommandOutcome {
    let id = AppCommandId::new(id).expect("a catalog id");
    app.dispatch_command_reporting(&id, CommandArgs::none(), None)
        .0
}

fn clicks(app: &mut Signex, points: &[Point]) {
    for p in points {
        click(app, (p.x, p.y));
    }
}

fn only_curve(app: &Signex) -> CurveShape {
    match doc(app).drawings.as_slice() {
        [SchDrawing::Curve(c)] => c.shape,
        other => panic!("one curve drawing, got {other:?}"),
    }
}

#[test]
fn each_curve_command_arms_its_own_tool() {
    for (id, tool) in [
        ("draw_ellipse", Tool::Ellipse),
        ("draw_elliptical_arc", Tool::EllipticalArc),
        ("draw_round_rectangle", Tool::RoundRectangle),
    ] {
        let mut app = app_with_tool(Tool::Select);
        run(&mut app, id);
        assert_eq!(app.interaction_state.current_tool, tool, "{id}");
        assert!(doc(&app).drawings.is_empty(), "{id} placed on arming");
    }
}

#[test]
fn the_ellipse_tool_places_on_its_second_click_as_one_undo_step() {
    let mut app = app_with_tool(Tool::Select);
    run(&mut app, "draw_ellipse");
    let (center, corner) = (grid(8.0, 8.0), grid(12.0, 10.0));
    clicks(&mut app, &[center]);
    assert!(doc(&app).drawings.is_empty());
    let canvas = app.interaction_state.active_canvas();
    let draft = canvas.annotation_draft.clone();
    assert_eq!(
        draft.map(|d| d.kind),
        Some(AnnotationDraftKind::Curve(CurveTool::Ellipse)),
        "the first click starts a preview"
    );

    clicks(&mut app, &[corner]);
    assert_eq!(
        only_curve(&app),
        CurveShape::Ellipse {
            center,
            radius_x: (corner.x - center.x).abs(),
            radius_y: (corner.y - center.y).abs(),
        }
    );
    assert!(app.interaction_state.annotation_tool.points.is_empty());

    run(&mut app, "undo");
    assert!(doc(&app).drawings.is_empty(), "one undo takes it back");
}

#[test]
fn the_elliptical_arc_tool_takes_five_clicks() {
    let mut app = app_with_tool(Tool::Select);
    run(&mut app, "draw_elliptical_arc");
    let center = grid(8.0, 8.0);
    let points = [
        center,
        grid(12.0, 0.0),
        grid(0.0, 10.0),
        grid(12.0, 8.0),
        grid(8.0, 10.0),
    ];
    clicks(&mut app, &points[..4]);
    assert!(doc(&app).drawings.is_empty(), "four clicks place nothing");
    clicks(&mut app, &points[4..]);
    let CurveShape::EllipticalArc {
        start_deg, end_deg, ..
    } = only_curve(&app)
    else {
        panic!("an elliptical arc");
    };
    assert!(start_deg.abs() < 1e-9, "{start_deg}");
    assert!((end_deg - 90.0).abs() < 1e-9, "{end_deg}");
}

#[test]
fn the_round_rectangle_tool_takes_two_corners_and_the_default_radius() {
    let mut app = app_with_tool(Tool::Select);
    run(&mut app, "draw_round_rectangle");
    let (a, b) = (grid(20.0, 4.0), grid(4.0, 12.0));
    clicks(&mut app, &[a, b]);
    assert_eq!(only_curve(&app), CurveShape::round_rect(a, b));
}

#[test]
fn two_clicks_on_one_point_place_no_curve() {
    for id in ["draw_ellipse", "draw_round_rectangle"] {
        let mut app = app_with_tool(Tool::Select);
        run(&mut app, id);
        let at = grid(4.0, 4.0);
        clicks(&mut app, &[at, at]);
        assert!(doc(&app).drawings.is_empty(), "{id}");
        assert!(app.interaction_state.annotation_tool.points.is_empty());
    }
}
