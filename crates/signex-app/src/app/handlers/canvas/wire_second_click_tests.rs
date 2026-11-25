//! The Wire tool ends a wire on its second click and stays armed for the
//! next one; it no longer continues the same wire (docs/DECISIONS.md,
//! 2026-11-24, Caner). The bus tool keeps going on from its second click.

use signex_types::schematic::Point;

use super::super::super::*;
use super::placement_tests::{app_with_tool, click, doc, grid};

fn at(p: Point) -> (f64, f64) {
    (p.x, p.y)
}

fn wire_ends(app: &Signex) -> Vec<(Point, Point)> {
    doc(app).wires.iter().map(|w| (w.start, w.end)).collect()
}

/// No wire in progress: the state, the canvas buffer and its rubber band.
fn assert_no_wire_in_progress(app: &Signex) {
    let state = &app.interaction_state;
    assert!(!state.wire_drawing, "no wire in progress");
    assert!(state.wire_points.is_empty(), "{:?}", state.wire_points);
    let canvas = state.active_canvas();
    assert!(!canvas.drawing_mode, "the canvas draws no rubber band");
    assert!(canvas.wire_preview.is_empty(), "{:?}", canvas.wire_preview);
    assert_eq!(state.current_tool, Tool::Wire, "the tool stays armed");
}

#[test]
fn two_clicks_place_exactly_one_wire_and_leave_no_wire_in_progress() {
    let mut app = app_with_tool(Tool::Wire);
    click(&mut app, at(grid(0.0, 0.0)));
    assert!(
        app.interaction_state.wire_drawing,
        "the first click starts it"
    );
    click(&mut app, at(grid(10.0, 0.0)));
    assert_eq!(wire_ends(&app), vec![(grid(0.0, 0.0), grid(10.0, 0.0))]);
    assert_no_wire_in_progress(&app);
}

#[test]
fn the_second_click_places_the_corner_of_the_wiring_mode() {
    let mut app = app_with_tool(Tool::Wire);
    click(&mut app, at(grid(0.0, 0.0)));
    click(&mut app, at(grid(10.0, 6.0)));
    // Horizontal first, the default: both legs of the L, then no wire.
    assert_eq!(
        wire_ends(&app),
        vec![
            (grid(0.0, 0.0), grid(10.0, 0.0)),
            (grid(10.0, 0.0), grid(10.0, 6.0)),
        ]
    );
    assert_no_wire_in_progress(&app);
}

#[test]
fn a_third_click_starts_a_new_wire_from_that_point() {
    let mut app = app_with_tool(Tool::Wire);
    click(&mut app, at(grid(0.0, 0.0)));
    click(&mut app, at(grid(10.0, 0.0)));

    click(&mut app, at(grid(20.0, 4.0)));
    assert_eq!(wire_ends(&app).len(), 1, "the third click places nothing");
    assert!(app.interaction_state.wire_drawing);
    assert_eq!(app.interaction_state.wire_points, vec![grid(20.0, 4.0)]);

    click(&mut app, at(grid(30.0, 4.0)));
    assert_eq!(
        wire_ends(&app),
        vec![
            (grid(0.0, 0.0), grid(10.0, 0.0)),
            (grid(20.0, 4.0), grid(30.0, 4.0)),
        ],
        "the new wire starts at the third click, not at the first wire's end"
    );
    assert_no_wire_in_progress(&app);
}

#[test]
fn one_undo_removes_only_the_last_wire() {
    let mut app = app_with_tool(Tool::Wire);
    click(&mut app, at(grid(0.0, 0.0)));
    click(&mut app, at(grid(10.0, 0.0)));
    let first = wire_ends(&app);
    // An L: two legs, one wire, one undo step.
    click(&mut app, at(grid(0.0, 10.0)));
    click(&mut app, at(grid(10.0, 16.0)));
    assert_eq!(wire_ends(&app).len(), 3);

    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(wire_ends(&app), first, "the first wire stays");
    assert_eq!(app.interaction_state.current_tool, Tool::Wire);
}

#[test]
fn the_bus_tool_still_goes_on_from_its_second_click() {
    let mut app = app_with_tool(Tool::Bus);
    click(&mut app, at(grid(0.0, 0.0)));
    click(&mut app, at(grid(10.0, 0.0)));
    assert!(app.interaction_state.wire_drawing, "the bus run is open");
    assert_eq!(app.interaction_state.wire_points, vec![grid(10.0, 0.0)]);
}
