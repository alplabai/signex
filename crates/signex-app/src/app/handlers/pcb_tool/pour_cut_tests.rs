//! #2216 — Pour Cutout and Slice Pour through `Signex::update`. Slice Pour:
//! one undo step, the larger piece keeps the pour, a slice that meets no
//! pour changes nothing. #2871 (DECISIONS 2026-11-24) — Pour Cutout places a
//! cutout region of its own as one undo step; the pour's outline is never
//! edited, its fill leaves the region empty, and moving or deleting the
//! region brings the copper back.

use pcb_geom::{ItemRef, PickKind};
use signex_types::pcb::Point;

use super::form_tests::form;
use super::tests::{
    app_with_board, board_of, click, escape, grid_only, live, right_click, run, undo,
};
use crate::app::{Message, Signex};
use crate::panels::pcb_primitive_properties::{PcbPrimitiveFacts, ZoneKind};
use crate::pcb_canvas::PcbCanvasMsg;

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

/// Places a Pour Cutout with `corners`, closed on its first corner, and
/// ends the tool.
fn place_cutout(app: &mut Signex, corners: &[(f64, f64)]) {
    run(app, "place_board_pour_cutout");
    for &(x, y) in corners {
        click(app, x, y);
    }
    click(app, corners[0].0, corners[0].1);
    escape(app);
    assert!(app.interaction_state.pcb_tool.is_none(), "the tool ended");
}

/// A 4 x 1 mm cutout on the 1 mm grid inside the upper half of the pour,
/// clear of the net-3 track along y = 0, its clearance and the outline.
const INSIDE: [(f64, f64); 4] = [(2.0, 2.0), (6.0, 2.0), (6.0, 3.0), (2.0, 3.0)];
/// A point inside [`INSIDE`].
const IN_CUTOUT: (f64, f64) = (4.0, 2.5);

/// The copper area, in mm², the fill of the pour (the first zone) holds.
fn pour_copper(app: &Signex, path: &std::path::Path) -> f64 {
    let board = board_of(app, path);
    pcb_geom::zone_fills(board)
        .of(0)
        .map_or(0.0, |fill| fill.area_mm2())
}

/// The smallest and largest x of a zone's outline.
fn x_span(outline: &[Point]) -> (f64, f64) {
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

/// #2871, old -> new: a cutout across the pour used to cut it into two
/// pours; it now places a region of its own, the pour keeps its outline,
/// and the pour's fill is split around the region.
#[test]
fn a_pour_cutout_across_a_pour_is_a_region_and_the_pour_keeps_its_outline() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    draw_pour(&mut app);
    let pour = board_of(&app, &path).zones[0].clone();
    let before = live(&app, &path);
    let pieces = |app: &Signex| {
        pcb_geom::zone_fills(board_of(app, &path))
            .of(0)
            .map_or(0, |fill| fill.pieces.len())
    };
    assert_eq!(pieces(&app), 2, "the net-3 track splits the pour");

    run(&mut app, "place_board_pour_cutout");
    for (x, y) in [(5.0, -6.0), (6.0, -6.0), (6.0, 6.0), (5.0, 6.0)] {
        click(&mut app, x, y);
    }
    assert_eq!(live(&app, &path), before, "open while drawing");
    click(&mut app, 5.0, -6.0);

    let zones = &board_of(&app, &path).zones;
    assert_eq!(zones.len(), 2, "the pour and the cutout");
    assert_eq!(zones[0], pour, "the pour is not edited");
    let cutout = &zones[1];
    assert!(cutout.is_pour_cutout() && cutout.layer == "TopCopper");
    let span = x_span(&cutout.outline);
    assert!(close(span.0, 5.0) && close(span.1, 6.0), "{span:?}");
    assert_eq!(pieces(&app), 4, "the fill is split around the cutout");
    let session = app.interaction_state.pcb_tool.as_ref().expect("armed");
    assert!(session.points.is_empty(), "ready for the next cutout");

    undo(&mut app);
    assert_eq!(live(&app, &path), before, "one undo step");
}

/// #2871, old -> new: a cutout inside a pour used to become a hole in the
/// pour's outline; it is now a region the fill leaves empty, and the pour
/// gains no hole. A right-click closes it.
#[test]
fn a_pour_cutout_inside_a_pour_leaves_the_fill_empty_and_a_right_click_closes_it() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    grid_only(&mut app);
    draw_pour(&mut app);
    let before = live(&app, &path);
    let full = pour_copper(&app, &path);

    run(&mut app, "place_board_pour_cutout");
    for (x, y) in [(3.0, 2.0), (6.0, 2.0), (6.0, 3.0)] {
        click(&mut app, x, y);
    }
    right_click(&mut app);

    let zones = &board_of(&app, &path).zones;
    assert_eq!(zones.len(), 2);
    assert!(zones[0].holes.is_empty(), "the pour gains no hole");
    assert!(zones[1].is_pour_cutout());
    let left = pour_copper(&app, &path);
    assert!(close(full - left, 1.5), "the 1.5 mm² triangle is empty");
    assert!(app.interaction_state.pcb_tool.is_some(), "the tool stays");
    undo(&mut app);
    assert_eq!(live(&app, &path), before);
}

/// #2871: a click inside the cutout picks it, not the pour under it, and
/// Properties shows a Pour Cutout with no net, its vertices and its area.
#[test]
fn a_click_inside_a_pour_cutout_picks_it_and_shows_its_panel() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    grid_only(&mut app);
    draw_pour(&mut app);
    place_cutout(&mut app, &INSIDE);
    let cutout = board_of(&app, &path).zones[1].uuid;

    click(&mut app, IN_CUTOUT.0, IN_CUTOUT.1);
    let PcbPrimitiveFacts::Zone(f) = form(&app) else {
        panic!("the area panel");
    };
    assert_eq!(
        f.common.target.items,
        vec![ItemRef::Zone {
            zone: cutout,
            kind: PickKind::Regions,
        }]
    );
    assert_eq!(f.kind, Some(ZoneKind::Cutout));
    assert!(!f.is_copper && !f.is_pour);
    let (vertices, area) = f.outline.expect("one area");
    assert_eq!(vertices, 4);
    assert!(close(area, 4.0), "{area}");
}

/// #2871: dragging the cutout off the pour moves it as one step, and the
/// repoured fill holds the copper again; the pour's outline never changes.
#[test]
fn moving_the_cutout_off_the_pour_brings_the_copper_back() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    grid_only(&mut app);
    draw_pour(&mut app);
    let full = pour_copper(&app, &path);
    place_cutout(&mut app, &INSIDE);
    let pour = board_of(&app, &path).zones[0].clone();
    assert!(close(full - pour_copper(&app, &path), 4.0));
    let placed = live(&app, &path);

    // From a grid point on the cutout's lower edge, 10 mm down the board.
    let (from, to) = (Point::new(4.0, 2.0), Point::new(4.0, 12.0));
    let _ = app.update(Message::PcbCanvas(PcbCanvasMsg::FootprintDragged {
        from,
        to,
        tolerance_mm: 0.05,
        leaves_tracks: false,
    }));
    let _ = app.update(Message::PcbCanvas(PcbCanvasMsg::FootprintDropped {
        from,
        to,
    }));

    let zones = &board_of(&app, &path).zones;
    assert_eq!(zones[0], pour, "the pour stays as it was");
    let ys: Vec<f64> = zones[1].outline.iter().map(|p| p.y).collect();
    assert!(
        close(ys[0], 12.0) && close(ys[2], 13.0),
        "moved by 10 mm: {ys:?}"
    );
    assert!(close(pour_copper(&app, &path), full), "the copper is back");
    undo(&mut app);
    assert_eq!(live(&app, &path), placed, "one undo step");
    assert!(close(full - pour_copper(&app, &path), 4.0));
}

/// #2871: deleting the cutout is one step and the repoured fill holds the
/// copper again.
#[test]
fn deleting_the_cutout_brings_the_copper_back() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    grid_only(&mut app);
    draw_pour(&mut app);
    let full = pour_copper(&app, &path);
    place_cutout(&mut app, &INSIDE);
    let placed = live(&app, &path);

    click(&mut app, IN_CUTOUT.0, IN_CUTOUT.1);
    run(&mut app, "delete_selection");
    let zones = &board_of(&app, &path).zones;
    assert_eq!(zones.len(), 1, "the cutout is gone");
    assert!(zones[0].is_pour());
    assert!(close(pour_copper(&app, &path), full), "the copper is back");
    undo(&mut app);
    assert_eq!(live(&app, &path), placed, "one undo step");
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

/// #2871, old -> new: a cutout off every pour used to change nothing and
/// say so; it is now placed like any other, as one step. A slice that meets
/// no pour still changes nothing and says so.
#[test]
fn a_cutout_off_every_pour_is_placed_and_a_slice_off_every_pour_says_so() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    draw_pour(&mut app);
    let pour = board_of(&app, &path).zones[0].clone();
    let needle = "no polygon pour on the active copper layer is cut there";
    let said = crate::diagnostics::logged_count(needle);

    run(&mut app, "place_board_pour_cutout");
    for (x, y) in [(20.0, 20.0), (24.0, 20.0), (24.0, 24.0)] {
        click(&mut app, x, y);
    }
    right_click(&mut app);
    let zones = &board_of(&app, &path).zones;
    assert_eq!(zones.len(), 2, "the cutout is placed");
    assert_eq!(zones[0], pour);
    assert!(zones[1].is_pour_cutout());
    escape(&mut app);
    escape(&mut app);

    let before = live(&app, &path);
    run(&mut app, "place_board_slice_pour");
    click(&mut app, 20.0, -6.0);
    click(&mut app, 20.0, 6.0);
    assert_eq!(live(&app, &path), before);
    assert!(crate::diagnostics::logged_count(needle) > said);
}
