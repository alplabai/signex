//! The shape tools commit the same way from a click, a double-click and
//! Enter (#2116).

use signex_types::schematic::{SchDrawing, SchematicSheet};

use super::super::super::*;
use crate::canvas::CanvasEvent;

fn app_with_tool(tool: Tool) -> Signex {
    let _ = crate::diagnostics::init_logging();
    let path = std::path::PathBuf::from("shape-tool.snxsch");
    let (mut app, _task) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.ui_state.snap_enabled = true;
    app.ui_state.grid_size_mm = 1.27;
    app.document_state.engines.insert(
        path.clone(),
        signex_engine::Engine::new(SchematicSheet::default()).expect("engine"),
    );
    app.document_state.tabs.push(TabInfo {
        title: "shape-tool".to_string(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path);
    app.interaction_state.current_tool = tool;
    app
}

fn click(app: &mut Signex, x: f64, y: f64) {
    let _ = app.update(Message::CanvasEvent(CanvasEvent::Clicked {
        world_x: x,
        world_y: y,
    }));
}

fn double_click(app: &mut Signex, x: f64, y: f64) {
    let _ = app.update(Message::CanvasEvent(CanvasEvent::DoubleClicked {
        world_x: x,
        world_y: y,
        screen_x: 0.0,
        screen_y: 0.0,
    }));
}

fn enter(app: &mut Signex) {
    let _ = app.update(Message::Selection(
        selection_request::SelectionRequest::LassoCommit,
    ));
}

fn drawings(app: &Signex) -> Vec<SchDrawing> {
    app.document_state
        .active_engine()
        .expect("engine")
        .document()
        .drawings
        .clone()
}

fn anchors_agree(app: &Signex) -> bool {
    let canvas = app.interaction_state.active_canvas();
    app.interaction_state.shape_anchor == canvas.shape_anchor.map(|(point, _)| point)
}

#[test]
fn a_line_click_seeds_then_commits_and_chains() {
    let mut app = app_with_tool(Tool::Line);
    click(&mut app, 0.0, 0.0);
    assert!(drawings(&app).is_empty());
    assert!(app.interaction_state.shape_anchor.is_some());
    click(&mut app, 12.7, 0.0);
    assert_eq!(drawings(&app).len(), 1);
    assert!(app.interaction_state.shape_anchor.is_some(), "re-armed");
    assert!(anchors_agree(&app));
}

#[test]
fn a_line_double_click_without_an_anchor_does_nothing() {
    let mut app = app_with_tool(Tool::Line);
    double_click(&mut app, 12.7, 0.0);
    assert!(drawings(&app).is_empty());
    assert!(app.interaction_state.shape_anchor.is_none());
}

#[test]
fn a_line_double_click_commits_like_the_second_click() {
    let mut app = app_with_tool(Tool::Line);
    click(&mut app, 0.0, 0.0);
    double_click(&mut app, 12.7, 0.0);
    assert_eq!(drawings(&app).len(), 1);
    assert!(app.interaction_state.shape_anchor.is_some(), "re-armed");
    assert!(anchors_agree(&app));
}

#[test]
fn a_rectangle_commits_from_a_click_and_from_a_double_click() {
    for second_is_double in [false, true] {
        let mut app = app_with_tool(Tool::Rectangle);
        click(&mut app, 0.0, 0.0);
        if second_is_double {
            double_click(&mut app, 12.7, 12.7);
        } else {
            click(&mut app, 12.7, 12.7);
        }
        assert!(matches!(drawings(&app)[..], [SchDrawing::Rect { .. }]));
        assert!(app.interaction_state.shape_anchor.is_none());
        assert!(app.interaction_state.active_canvas().shape_anchor.is_none());
    }
}

#[test]
fn a_circle_needs_a_radius_and_clears_the_anchor_either_way() {
    for second_is_double in [false, true] {
        let mut app = app_with_tool(Tool::Circle);
        click(&mut app, 0.0, 0.0);
        if second_is_double {
            double_click(&mut app, 0.0, 0.0);
        } else {
            click(&mut app, 0.0, 0.0);
        }
        assert!(drawings(&app).is_empty(), "no radius, no circle");
        assert!(app.interaction_state.shape_anchor.is_none());
        assert!(app.interaction_state.active_canvas().shape_anchor.is_none());

        click(&mut app, 0.0, 0.0);
        click(&mut app, 12.7, 0.0);
        assert!(matches!(drawings(&app)[..], [SchDrawing::Circle { .. }]));
    }
}

#[test]
fn an_arc_commits_on_the_third_point_and_empties_both_buffers() {
    for third_is_double in [false, true] {
        let mut app = app_with_tool(Tool::Arc);
        click(&mut app, 0.0, 0.0);
        click(&mut app, 6.35, 6.35);
        assert_eq!(app.interaction_state.active_canvas().arc_points.len(), 2);
        if third_is_double {
            double_click(&mut app, 12.7, 0.0);
        } else {
            click(&mut app, 12.7, 0.0);
        }
        assert!(matches!(drawings(&app)[..], [SchDrawing::Arc { .. }]));
        assert!(app.interaction_state.arc_points.is_empty());
        assert!(app.interaction_state.active_canvas().arc_points.is_empty());
    }
}

#[test]
fn a_polyline_double_click_closes_it_and_a_repeat_vertex_is_dropped() {
    let mut app = app_with_tool(Tool::Polyline);
    click(&mut app, 0.0, 0.0);
    double_click(&mut app, 12.7, 0.0);
    match &drawings(&app)[..] {
        [SchDrawing::Polyline { points, .. }] => assert_eq!(points.len(), 2),
        other => panic!("expected one polyline, got {} drawings", other.len()),
    }
    assert!(app.interaction_state.polyline_points.is_empty());

    let mut app = app_with_tool(Tool::Polyline);
    click(&mut app, 0.0, 0.0);
    double_click(&mut app, 0.0, 0.0);
    assert!(drawings(&app).is_empty(), "one vertex is not a polyline");
    assert_eq!(app.interaction_state.polyline_points.len(), 1);
}

/// Enter used to commit with the raw pre-placement width, 0 when the panel
/// was never opened; it now takes the project's default shape stroke, as a
/// double-click does.
#[test]
fn enter_commits_a_polyline_with_the_default_stroke() {
    let mut app = app_with_tool(Tool::Polyline);
    click(&mut app, 0.0, 0.0);
    click(&mut app, 12.7, 0.0);
    enter(&mut app);
    match &drawings(&app)[..] {
        [SchDrawing::Polyline { width, points, .. }] => {
            assert_eq!(points.len(), 2);
            let default_mm = app
                .document_state
                .active_schematic_defaults()
                .shape_stroke_mm(0.0);
            assert!((*width - default_mm).abs() < 1e-12, "width {width}");
        }
        other => panic!("expected one polyline, got {} drawings", other.len()),
    }
    assert!(app.interaction_state.polyline_points.is_empty());
    assert!(
        app.interaction_state
            .active_canvas()
            .polyline_points
            .is_empty()
    );
}
