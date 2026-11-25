//! #2514 — while a route is drawn or a copper placement tool is armed, the
//! board keeps that net bright and dims the others; the scene is drawn
//! again when the tool starts and ends, Esc included.

use super::tests::{app_with_board, click, escape, grid_only, hover, run};
use crate::app::Signex;

/// The nets in focus the board scene is drawn with.
fn drawn_nets(app: &Signex) -> Vec<u32> {
    app.interaction_state
        .pcb_canvas
        .active_renderer_snapshot()
        .and_then(|snapshot| snapshot.paint.net_focus())
        .map(|focus| focus.nets.clone())
        .unwrap_or_default()
}

#[test]
fn a_track_run_keeps_its_net_bright_until_esc_ends_it() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _path) = app_with_board(dir.path());
    grid_only(&mut app);
    run(&mut app, "place_board_track");
    assert!(drawn_nets(&app).is_empty(), "armed, no run yet");

    click(&mut app, 2.1, 0.2);
    assert_eq!(drawn_nets(&app), [3], "the run starts on the net-3 track");
    hover(&mut app, 5.2, 4.1);
    assert_eq!(drawn_nets(&app), [3], "the pointer moving keeps it");

    escape(&mut app);
    assert!(app.interaction_state.pcb_tool.is_some(), "the tool stays");
    assert!(drawn_nets(&app).is_empty(), "Esc ends the run");
    escape(&mut app);
    assert!(app.interaction_state.pcb_tool.is_none());
    assert!(drawn_nets(&app).is_empty());
}

#[test]
fn an_interactive_route_keeps_its_net_bright_until_esc_ends_it() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _path) = app_with_board(dir.path());
    grid_only(&mut app);
    run(&mut app, "place_board_route");
    assert!(drawn_nets(&app).is_empty(), "armed, no route yet");

    click(&mut app, 2.0, 0.0);
    assert_eq!(drawn_nets(&app), [3]);

    escape(&mut app);
    escape(&mut app);
    assert!(app.interaction_state.pcb_tool.is_none());
    assert!(drawn_nets(&app).is_empty());
}

#[test]
fn a_via_placed_on_a_set_net_keeps_that_net_bright_while_armed() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _path) = app_with_board(dir.path());
    app.interaction_state.pcb_tool_defaults.via_net = Some(3);
    run(&mut app, "place_board_via");
    assert_eq!(drawn_nets(&app), [3], "arming the tool dims the others");

    escape(&mut app);
    assert!(app.interaction_state.pcb_tool.is_none(), "Esc ends it");
    assert!(drawn_nets(&app).is_empty(), "and the board comes back");
}

#[test]
fn a_placement_on_no_net_dims_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _path) = app_with_board(dir.path());
    for net in [None, Some(0)] {
        app.interaction_state.pcb_tool_defaults.via_net = net;
        run(&mut app, "place_board_via");
        assert!(drawn_nets(&app).is_empty(), "{net:?}");
        escape(&mut app);
    }
}
