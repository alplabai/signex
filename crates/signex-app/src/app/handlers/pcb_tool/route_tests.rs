//! #2373 — the Track tool through the app: a run on the active copper layer
//! with the wire corner rule, Space to flip the bend, a via on a layer
//! change, one undo step per click, and Esc to end the run and then the
//! tool.

use signex_types::pcb::Point;
use signex_types::pcb_layer::{CopperLayer, PcbLayer};

use super::tests::{app_with_board, board_of, click, escape, has_ghost, hover, live, run, undo};
use crate::app::{Message, PcbMsg, PcbViewMsg};

fn legs(app: &crate::app::Signex, path: &std::path::Path) -> Vec<(Point, Point)> {
    board_of(app, path)
        .segments
        .iter()
        .skip(1)
        .map(|s| (s.start, s.end))
        .collect()
}

#[test]
fn a_track_run_starts_on_the_net_under_it_and_adds_its_legs_per_click() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let before = live(&app, &path);
    run(&mut app, "place_board_track");
    click(&mut app, 2.1, 0.2);
    assert_eq!(
        live(&app, &path),
        before,
        "the first click only starts the run"
    );
    hover(&mut app, 5.2, 4.1);
    assert!(has_ghost(&app), "the next legs follow the pointer");

    click(&mut app, 5.2, 4.1);
    assert_eq!(
        legs(&app, &path),
        [
            (Point::new(2.0, 0.0), Point::new(5.0, 0.0)),
            (Point::new(5.0, 0.0), Point::new(5.0, 4.0)),
        ],
        "horizontal first, on the grid"
    );
    let added = &board_of(&app, &path).segments[1];
    assert_eq!(
        (added.layer.as_str(), added.width, added.net),
        ("TopCopper", 0.3, 3)
    );

    click(&mut app, 8.0, 4.0);
    assert_eq!(legs(&app, &path).len(), 3, "the run goes on from its end");
    undo(&mut app);
    assert_eq!(legs(&app, &path).len(), 2, "one undo per click");
    undo(&mut app);
    assert_eq!(live(&app, &path), before);
}

#[test]
fn space_flips_the_bend_of_the_run() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    run(&mut app, "place_board_track");
    click(&mut app, 2.0, 0.0);
    run(&mut app, "rotate_counterclockwise");
    click(&mut app, 5.0, 4.0);
    assert_eq!(
        legs(&app, &path),
        [
            (Point::new(2.0, 0.0), Point::new(2.0, 4.0)),
            (Point::new(2.0, 4.0), Point::new(5.0, 4.0)),
        ],
        "vertical first after the flip"
    );
}

#[test]
fn a_layer_change_mid_run_adds_a_via_with_the_next_legs_in_one_step() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let before = live(&app, &path);
    run(&mut app, "place_board_track");
    click(&mut app, 2.0, 0.0);
    let bottom = PcbLayer::Copper(CopperLayer::Bottom);
    let _ = app.update(Message::Pcb(PcbMsg::View(PcbViewMsg::ActiveLayerPicked(
        bottom,
    ))));
    click(&mut app, 2.0, 3.0);
    let board = board_of(&app, &path);
    assert_eq!(board.vias.len(), 1);
    assert_eq!(board.vias[0].position, Point::new(2.0, 0.0));
    assert_eq!(board.vias[0].net, 3, "the via carries the run's net");
    assert_eq!(board.segments[1].layer, "BottomCopper");
    assert_eq!(board.segments[1].net, 3);
    undo(&mut app);
    assert_eq!(live(&app, &path), before, "via and legs are one step");
}

#[test]
fn esc_ends_the_run_then_the_tool_and_a_click_on_the_end_ends_the_run() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    run(&mut app, "place_board_track");
    click(&mut app, 2.0, 0.0);
    click(&mut app, 2.0, 0.0);
    let session = app
        .interaction_state
        .pcb_tool
        .as_ref()
        .expect("still armed");
    assert!(
        session.points.is_empty(),
        "a click on the run's end ends it"
    );

    click(&mut app, 2.0, 0.0);
    escape(&mut app);
    let session = app
        .interaction_state
        .pcb_tool
        .as_ref()
        .expect("still armed");
    assert!(session.points.is_empty(), "the first Esc ends the run");
    escape(&mut app);
    assert!(
        app.interaction_state.pcb_tool.is_none(),
        "the second the tool"
    );
    assert_eq!(board_of(&app, &path).segments.len(), 1, "nothing was added");
}

#[test]
fn track_needs_a_copper_layer_active() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let overlay = PcbLayer::Technical(signex_types::pcb_layer::TechnicalLayer::SilkscreenTop);
    let _ = app.update(Message::Pcb(PcbMsg::View(PcbViewMsg::ActiveLayerPicked(
        overlay,
    ))));
    run(&mut app, "place_board_track");
    click(&mut app, 2.0, 0.0);
    click(&mut app, 5.0, 0.0);
    assert_eq!(board_of(&app, &path).segments.len(), 1, "nothing is routed");
}
