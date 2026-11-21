//! Caner 2026-11-21: Tab pauses a board tool the way it pauses a sheet
//! placement. While paused, what the next click adds stays where it was
//! (pointer moves and canvas clicks do nothing) and the Placement Paused
//! banner shows over the board; Tab or Enter goes on with the edited
//! values, Esc puts the previous ones back. Driven through `Signex::update`.

use std::path::Path;

use signex_types::pcb::PcbBoard;

use super::form_tests::{panel, tab, typed};
use super::tests::{app_with_board, board_of, click, escape, hover, live, run};
use crate::app::Signex;
use crate::panels::PcbPrimitiveMsg;
use crate::panels::pcb_primitive_properties::{PrimitiveField, PrimitiveTarget};

/// The main window's id, set so the banner check can name the window.
fn main_window(app: &mut Signex) -> iced::window::Id {
    let id = app
        .ui_state
        .main_window_id
        .unwrap_or_else(iced::window::Id::unique);
    app.ui_state.main_window_id = Some(id);
    id
}

fn preview(app: &Signex) -> PcbBoard {
    app.board_tool_preview().expect("a preview on the pointer")
}

fn drawn(app: &Signex) -> String {
    serde_json::to_string(&preview(app)).unwrap()
}

#[test]
fn tab_freezes_the_route_on_the_pointer_and_tab_again_lets_it_follow() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let window = main_window(&mut app);
    run(&mut app, "place_board_route");
    click(&mut app, 2.0, 0.0);
    hover(&mut app, 2.0, 4.0);
    let frozen = drawn(&app);
    assert!(!preview(&app).segments.is_empty(), "the leg follows");
    assert!(!app.board_tool_paused_in_window(window));

    tab(&mut app);
    assert!(app.is_board_tool_paused());
    assert!(app.board_tool_paused_in_window(window), "the paused mark");
    hover(&mut app, 6.0, 5.0);
    assert_eq!(drawn(&app), frozen, "the paused route ignores the pointer");
    let before = live(&app, &path);
    let points = app.own_board_tool().map(|s| s.points.len());
    click(&mut app, 6.0, 5.0);
    assert_eq!(live(&app, &path), before, "a paused click adds nothing");
    assert_eq!(app.own_board_tool().map(|s| s.points.len()), points);

    tab(&mut app);
    assert!(!app.is_board_tool_paused());
    assert!(!app.board_tool_paused_in_window(window));
    hover(&mut app, 6.0, 5.0);
    assert_ne!(drawn(&app), frozen, "the route follows again");
}

/// Esc while paused mid-run: the width typed and the net picked in the
/// pause are put back on the run being drawn.
#[test]
fn esc_in_the_pause_puts_the_runs_width_and_net_back() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let window = main_window(&mut app);
    run(&mut app, "place_board_track");
    click(&mut app, 2.0, 0.0);
    hover(&mut app, 2.0, 4.0);
    let width_net = |app: &Signex| {
        let leg = &preview(app).segments[0];
        (leg.width, leg.net)
    };
    let before = width_net(&app);
    assert_eq!(before.1, 3, "the run takes the net of the track");

    tab(&mut app);
    typed(&mut app, &path, PrimitiveField::Width, "0.6");
    pick_net(&mut app, &path, 0);
    assert_eq!(width_net(&app).1, 0, "the net picked in the pause");
    escape(&mut app);
    assert!(!app.is_board_tool_paused(), "Esc ends the pause");
    assert!(!app.board_tool_paused_in_window(window));
    assert_eq!(width_net(&app), before, "the width and net before the pause");
    click(&mut app, 2.0, 4.0);
    let tracks = &board_of(&app, &path).segments;
    let placed = tracks.last().expect("the leg lands");
    assert_eq!((placed.width, placed.net), before);
}

/// The other tools share the pause: a via on the pointer holds still and
/// the banner shows.
#[test]
fn a_paused_via_holds_still_and_shows_the_paused_mark() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _path) = app_with_board(dir.path());
    let window = main_window(&mut app);
    run(&mut app, "place_board_via");
    hover(&mut app, 4.0, 4.0);
    tab(&mut app);
    assert!(app.board_tool_paused_in_window(window), "the paused mark");
    hover(&mut app, 7.0, 7.0);
    let at = preview(&app).vias[0].position;
    assert_eq!((at.x, at.y), (4.0, 4.0), "the via stays where it was");
    escape(&mut app);
    assert!(!app.board_tool_paused_in_window(window));
    hover(&mut app, 7.0, 7.0);
    let at = preview(&app).vias[0].position;
    assert_eq!((at.x, at.y), (7.0, 7.0), "and follows once resumed");
}

fn pick_net(app: &mut Signex, path: &Path, net: u32) {
    panel(
        app,
        PcbPrimitiveMsg::NetPicked {
            target: PrimitiveTarget::placement(path.to_path_buf()),
            net,
        },
    );
}
