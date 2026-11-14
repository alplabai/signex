//! #2373 — Fill and Solid Region through the app: solid copper on the
//! active copper layer, on the net under the first point, one undo step.

use signex_types::pcb::Point;

use super::tests::{
    app_with_board, board_of, click, escape, has_ghost, hover, live, right_click, run, undo,
};

#[test]
fn fill_is_the_rectangle_between_two_clicks_in_one_step() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let before = live(&app, &path);
    run(&mut app, "place_board_fill");
    click(&mut app, 1.1, 0.1);
    hover(&mut app, 4.0, 3.0);
    assert!(has_ghost(&app), "the rectangle follows the pointer");
    assert_eq!(live(&app, &path), before, "nothing until the second corner");
    click(&mut app, 4.0, 3.0);
    let zones = &board_of(&app, &path).zones;
    assert_eq!(zones.len(), 1);
    assert_eq!(
        zones[0].outline,
        [
            Point::new(1.0, 0.0),
            Point::new(4.0, 0.0),
            Point::new(4.0, 3.0),
            Point::new(1.0, 3.0)
        ]
    );
    assert_eq!((zones[0].layer.as_str(), zones[0].net), ("TopCopper", 3));
    let session = app.interaction_state.pcb_tool.as_ref().expect("still armed");
    assert!(session.points.is_empty(), "ready for the next fill");
    undo(&mut app);
    assert_eq!(live(&app, &path), before);
}

#[test]
fn a_solid_region_closes_on_its_first_corner_or_a_right_click() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let before = live(&app, &path);
    run(&mut app, "place_board_region");
    for (x, y) in [(5.0, 5.0), (9.0, 5.0), (7.0, 8.0)] {
        click(&mut app, x, y);
    }
    assert_eq!(live(&app, &path), before, "open while drawing");
    click(&mut app, 5.0, 5.0);
    let zones = &board_of(&app, &path).zones;
    assert_eq!(zones.len(), 1, "the first corner closes it");
    assert_eq!(zones[0].outline.len(), 3);
    assert_eq!(zones[0].net, 0, "bare board");

    for (x, y) in [(5.0, 10.0), (9.0, 10.0), (9.0, 13.0), (5.0, 13.0)] {
        click(&mut app, x, y);
    }
    right_click(&mut app);
    assert_eq!(board_of(&app, &path).zones.len(), 2, "a right-click closes it");
    assert!(app.interaction_state.pcb_tool.is_some(), "and keeps the tool");
    assert!(app.interaction_state.context_menu.is_none());

    click(&mut app, 20.0, 20.0);
    escape(&mut app);
    escape(&mut app);
    assert!(app.interaction_state.pcb_tool.is_none());
    assert_eq!(board_of(&app, &path).zones.len(), 2, "Esc drops the open one");
    undo(&mut app);
    undo(&mut app);
    assert_eq!(live(&app, &path), before);
}
