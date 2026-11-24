//! #2216 — Pour Cutout and Slice Pour through `Signex::update`: each cut is
//! one undo step, the larger piece keeps the pour, and a cut that meets no
//! pour changes nothing.

use super::tests::{
    app_with_board, board_of, click, escape, grid_only, live, right_click, run, undo,
};
use crate::app::Signex;

/// Draws a pour over (1, -4)..(9, 4) on the top copper and ends the tool.
/// Its first corner is off the net-3 track, so it has no net.
pub(super) fn draw_pour(app: &mut Signex) {
    run(app, "place_board_polygon_pour");
    for (x, y) in [(1.0, -4.0), (9.0, -4.0), (9.0, 4.0), (1.0, 4.0)] {
        click(app, x, y);
    }
    right_click(app);
    escape(app);
    assert!(app.interaction_state.pcb_tool.is_none(), "the tool ended");
}

/// The smallest and largest x of a zone's outline.
fn x_span(outline: &[signex_types::pcb::Point]) -> (f64, f64) {
    let min = outline.iter().map(|p| p.x).fold(f64::INFINITY, f64::min);
    let max = outline
        .iter()
        .map(|p| p.x)
        .fold(f64::NEG_INFINITY, f64::max);
    (min, max)
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

#[test]
fn a_pour_cutout_across_a_pour_leaves_two_pours_in_one_step() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    draw_pour(&mut app);
    let pour = board_of(&app, &path).zones[0].uuid;
    let before = live(&app, &path);

    run(&mut app, "place_board_pour_cutout");
    for (x, y) in [(5.0, -6.0), (6.0, -6.0), (6.0, 6.0), (5.0, 6.0)] {
        click(&mut app, x, y);
    }
    assert_eq!(live(&app, &path), before, "open while drawing");
    click(&mut app, 5.0, -6.0);

    let zones = &board_of(&app, &path).zones;
    assert_eq!(zones.len(), 2, "the cutout cuts the pour in two");
    assert_eq!(zones[0].uuid, pour, "the larger piece keeps the pour");
    assert!(zones.iter().all(|z| z.is_pour() && z.layer == "TopCopper"));
    let (left, right) = (x_span(&zones[0].outline), x_span(&zones[1].outline));
    assert!(close(left.0, 1.0) && close(left.1, 5.0), "{left:?}");
    assert!(close(right.0, 6.0) && close(right.1, 9.0), "{right:?}");
    let session = app.interaction_state.pcb_tool.as_ref().expect("armed");
    assert!(session.points.is_empty(), "ready for the next cutout");

    undo(&mut app);
    assert_eq!(live(&app, &path), before, "one undo step");
}

#[test]
fn a_pour_cutout_inside_a_pour_is_a_hole_and_a_right_click_closes_it() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    grid_only(&mut app);
    draw_pour(&mut app);
    let before = live(&app, &path);

    run(&mut app, "place_board_pour_cutout");
    for (x, y) in [(3.0, 2.0), (6.0, 2.0), (6.0, 3.0)] {
        click(&mut app, x, y);
    }
    right_click(&mut app);

    let zones = &board_of(&app, &path).zones;
    assert_eq!(zones.len(), 1);
    assert_eq!(zones[0].holes.len(), 1, "the cutout is a hole");
    assert!(app.interaction_state.pcb_tool.is_some(), "the tool stays");
    undo(&mut app);
    assert_eq!(live(&app, &path), before);
}

#[test]
fn slice_pour_cuts_the_pour_in_two_along_the_line() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    draw_pour(&mut app);
    let pour = board_of(&app, &path).zones[0].uuid;
    let before = live(&app, &path);

    run(&mut app, "place_board_slice_pour");
    click(&mut app, 3.0, -6.0);
    assert_eq!(live(&app, &path), before, "one point is no line");
    click(&mut app, 3.0, 6.0);

    let zones = &board_of(&app, &path).zones;
    assert_eq!(zones.len(), 2);
    assert_eq!(zones[0].uuid, pour, "the larger piece keeps the pour");
    let (larger, smaller) = (x_span(&zones[0].outline), x_span(&zones[1].outline));
    assert!(close(larger.0, 3.0) && close(larger.1, 9.0), "{larger:?}");
    assert!(
        close(smaller.0, 1.0) && close(smaller.1, 3.0),
        "{smaller:?}"
    );

    undo(&mut app);
    assert_eq!(live(&app, &path), before, "one undo step");
}

#[test]
fn a_cut_that_meets_no_pour_changes_nothing_and_says_so() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    draw_pour(&mut app);
    let before = live(&app, &path);
    let needle = "no polygon pour on the active copper layer is cut there";
    let said = crate::diagnostics::logged_count(needle);

    run(&mut app, "place_board_pour_cutout");
    for (x, y) in [(20.0, 20.0), (24.0, 20.0), (24.0, 24.0)] {
        click(&mut app, x, y);
    }
    right_click(&mut app);
    assert_eq!(live(&app, &path), before);

    run(&mut app, "place_board_slice_pour");
    click(&mut app, 20.0, -6.0);
    click(&mut app, 20.0, 6.0);
    assert_eq!(live(&app, &path), before);
    assert!(crate::diagnostics::logged_count(needle) >= said + 2);
}
