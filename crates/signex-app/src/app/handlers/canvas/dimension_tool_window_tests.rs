//! A dimension's first point belongs to its window (mockup row 157): a click
//! from another window drops it and starts a new dimension there.

use std::path::Path;

use signex_types::schematic::{Point, SchDimension, SchDrawing};

use super::dimension_tool_pause_tests::{
    app_on_sheet_a, click_in, dimensions_on, held_points, pause_in, show_tab, tab_key,
    undocked_over,
};
use super::placement_tests::click;
use crate::app::{Message, Signex, WindowMsg};
use crate::canvas::CanvasEvent;

const A_POINT: (f64, f64) = (2.54, 2.54);
const B_FROM: (f64, f64) = (25.4, 25.4);
const B_TO: (f64, f64) = (50.8, 25.4);
const B_LINE: (f64, f64) = (38.1, 38.1);

fn point((x, y): (f64, f64)) -> Point {
    Point::new(x, y)
}

fn only_dimension(app: &Signex, path: &Path) -> SchDimension {
    let sheet = app.document_state.engines[path].document();
    let mut found = sheet.drawings.iter().filter_map(|d| match d {
        SchDrawing::Dimension(d) => Some(d.clone()),
        _ => None,
    });
    let dimension = found.next().expect("one dimension");
    assert!(found.next().is_none(), "only one dimension");
    dimension
}

fn double_click_in(app: &mut Signex, window: iced::window::Id, (x, y): (f64, f64)) {
    let _ = app.update(Message::CanvasEventInWindow {
        window_id: window,
        event: CanvasEvent::DoubleClicked {
            world_x: x,
            world_y: y,
            screen_x: 0.0,
            screen_y: 0.0,
        },
    });
}

/// One click on A, two on B: the two hold B's points and place nothing,
/// since a dimension takes three clicks; the third places B's alone, on B.
#[test]
fn one_click_on_a_then_two_on_b_uses_only_bs_points() {
    let (mut app, paths) = app_on_sheet_a(&["b.snxsch"]);
    let window = undocked_over(&mut app, &paths[1]);
    click(&mut app, A_POINT);
    click_in(&mut app, window, B_FROM);
    click_in(&mut app, window, B_TO);
    assert_eq!(dimensions_on(&app, &paths[0]), 0, "nothing lands on A");
    assert_eq!(
        dimensions_on(&app, &paths[1]),
        0,
        "two clicks place nothing"
    );
    let tool = &app.interaction_state.annotation_tool;
    assert_eq!(tool.points, vec![point(B_FROM), point(B_TO)]);
    click_in(&mut app, window, B_LINE);
    assert_eq!(dimensions_on(&app, &paths[0]), 0, "nothing lands on A");
    let placed = only_dimension(&app, &paths[1]);
    assert_eq!((placed.from, placed.to), (point(B_FROM), point(B_TO)));
}

#[test]
fn a_quick_second_click_on_b_is_still_bs_own_point() {
    let (mut app, paths) = app_on_sheet_a(&["b.snxsch"]);
    let window = undocked_over(&mut app, &paths[1]);
    click(&mut app, A_POINT);
    click_in(&mut app, window, B_FROM);
    double_click_in(&mut app, window, B_TO);
    let tool = &app.interaction_state.annotation_tool;
    assert_eq!(tool.points, vec![point(B_FROM), point(B_TO)]);
}

#[test]
fn a_click_in_main_drops_the_point_of_a_window() {
    let (mut app, paths) = app_on_sheet_a(&["b.snxsch"]);
    let window = undocked_over(&mut app, &paths[1]);
    click_in(&mut app, window, B_FROM);
    assert!(
        app.interaction_state.canvases[&window]
            .annotation_draft
            .is_some()
    );
    click(&mut app, A_POINT);
    let state = &app.interaction_state;
    assert_eq!(state.annotation_tool.points, vec![point(A_POINT)]);
    assert_eq!(state.dimension_points_window(), Some(None));
    assert!(state.canvases[&window].annotation_draft.is_none());
}

#[test]
fn a_click_in_a_window_clears_mains_preview_of_the_dropped_point() {
    let (mut app, paths) = app_on_sheet_a(&["b.snxsch"]);
    let window = undocked_over(&mut app, &paths[1]);
    click(&mut app, A_POINT);
    assert!(app.interaction_state.canvas.annotation_draft.is_some());
    click_in(&mut app, window, B_FROM);
    let state = &app.interaction_state;
    assert!(state.canvas.annotation_draft.is_none());
    let draft = state.canvases[&window].annotation_draft.as_ref();
    assert_eq!(draft.expect("B's preview").points, vec![point(B_FROM)]);
}

#[test]
fn the_dropped_dimension_takes_its_override_and_flip_with_it() {
    let (mut app, paths) = app_on_sheet_a(&["b.snxsch"]);
    let window = undocked_over(&mut app, &paths[1]);
    click(&mut app, A_POINT);
    let style = &mut app.interaction_state.annotation_tool.dimension;
    style.override_text = Some("typed".to_string());
    style.flipped = true;
    click_in(&mut app, window, B_FROM);
    let style = &app.interaction_state.annotation_tool.dimension;
    assert_eq!(style.override_text, None);
    assert!(!style.flipped);
}

#[test]
fn the_owner_ends_with_the_points() {
    let (mut app, paths) = app_on_sheet_a(&["b.snxsch"]);
    let window = undocked_over(&mut app, &paths[1]);
    click_in(&mut app, window, B_FROM);
    assert_eq!(
        app.interaction_state.dimension_points_window(),
        Some(Some(window))
    );
    click_in(&mut app, window, B_TO);
    click_in(&mut app, window, B_LINE);
    assert_eq!(dimensions_on(&app, &paths[1]), 1);
    assert_eq!(app.interaction_state.dimension_points_window(), None);
    click(&mut app, A_POINT);
    click(&mut app, B_TO);
    assert_eq!(held_points(&app), 2, "main's clicks build one dimension");
    assert_eq!(app.interaction_state.dimension_points_window(), Some(None));
}

#[test]
fn a_tab_switch_in_main_keeps_the_point_of_a_window() {
    let (mut app, paths) = app_on_sheet_a(&["b.snxsch", "c.snxsch"]);
    let window = undocked_over(&mut app, &paths[1]);
    click_in(&mut app, window, B_FROM);
    show_tab(&mut app, &paths[2]);
    assert_eq!(held_points(&app), 1, "as a wire in that window stays");
    assert!(
        app.interaction_state.canvases[&window]
            .annotation_draft
            .is_some()
    );
}

#[test]
fn closing_a_window_ends_the_dimension_begun_there() {
    let (mut app, paths) = app_on_sheet_a(&["b.snxsch"]);
    let window = undocked_over(&mut app, &paths[1]);
    click_in(&mut app, window, B_FROM);
    let _ = app.update(Message::Window(WindowMsg::SecondaryWindowClosed(window)));
    assert_eq!(held_points(&app), 0);
    assert_eq!(app.interaction_state.dimension_points_window(), None);
}

#[test]
fn a_double_click_elsewhere_does_not_finish_a_paused_dimension() {
    let (mut app, paths) = app_on_sheet_a(&["b.snxsch", "c.snxsch"]);
    let window = undocked_over(&mut app, &paths[1]);
    let other = undocked_over(&mut app, &paths[2]);
    pause_in(&mut app, window);
    double_click_in(&mut app, other, A_POINT);
    assert_eq!(held_points(&app), 2, "the paused points stay");
    assert_eq!(dimensions_on(&app, &paths[2]), 0);
    assert_eq!(dimensions_on(&app, &paths[1]), 0);
}

#[test]
fn a_tab_pause_in_main_over_a_windows_point_places_nothing_with_it() {
    let (mut app, paths) = app_on_sheet_a(&["b.snxsch"]);
    let window = undocked_over(&mut app, &paths[1]);
    click_in(&mut app, window, B_FROM);
    tab_key(&mut app);
    tab_key(&mut app);
    click(&mut app, A_POINT);
    let tool = &app.interaction_state.annotation_tool;
    assert_eq!(tool.points, vec![point(A_POINT)]);
    assert_eq!(dimensions_on(&app, &paths[0]), 0);
}
