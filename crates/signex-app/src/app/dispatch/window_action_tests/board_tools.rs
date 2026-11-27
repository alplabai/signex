//! #2629: every board Tools row, Repour and Tune picked in an undocked
//! board window edit that window's board, never the board the main window
//! shows.

use std::path::{Path, PathBuf};

use serde_json::json;
use signex_types::pcb::PcbBoard;
use uuid::Uuid;

use super::panel_scope::{app_with_main, run_in, undock};
use super::*;

/// A board with one teardrop and one unfilled pour, both on GND.
fn board_with_a_teardrop_and_an_unfilled_pour(n: u128) -> PcbBoard {
    let outline = json!([{"x": 0.0, "y": 0.0}, {"x": 4.0, "y": 0.0}, {"x": 4.0, "y": 4.0}]);
    serde_json::from_value(json!({
        "uuid": Uuid::from_u128(n),
        "nets": [{"number": 1, "name": "GND"}],
        "zones": [
            {"uuid": Uuid::from_u128(n + 1), "net": 1, "net_name": "GND",
             "layer": "TopCopper", "outline": outline, "fill_type": "teardrop"},
            {"uuid": Uuid::from_u128(n + 2), "net": 1, "net_name": "GND",
             "layer": "TopCopper", "outline": outline, "fill_type": "pour",
             "unfilled": true}
        ]
    }))
    .expect("a board")
}

/// Boards `a` (shown in the main window) and `b` (undocked), each with a
/// teardrop and an unfilled pour.
fn two_boards() -> (Signex, PathBuf, PathBuf, iced::window::Id) {
    let mut app = app_with_main();
    let (a, b) = (PathBuf::from("a.snxpcb"), PathBuf::from("b.snxpcb"));
    for (n, path) in [(100, &a), (200, &b)] {
        let board = board_with_a_teardrop_and_an_unfilled_pour(n);
        app.open_pcb_tab(path.clone(), "board".into(), board);
    }
    app.document_state.active_tab = app.document_state.tabs.len() - 2;
    let window = undock(&mut app, &b);
    assert_eq!(app.active_tab_path().as_ref(), Some(&a));
    (app, a, b, window)
}

fn teardrops(app: &Signex, path: &Path) -> usize {
    let board = app.document_state.pcb_board(path).expect("board");
    board.zones.iter().filter(|z| z.is_teardrop()).count()
}

fn unfilled_pours(app: &Signex, path: &Path) -> usize {
    let board = app.document_state.pcb_board(path).expect("board");
    board
        .zones
        .iter()
        .filter(|z| z.is_pour() && z.unfilled)
        .count()
}

#[test]
fn remove_teardrops_in_a_board_window_removes_that_boards_teardrops_only() {
    let (mut app, a, b, window) = two_boards();
    run_in(&mut app, "remove_teardrops", window);
    assert_eq!(teardrops(&app, &b), 0, "the window's board");
    assert_eq!(
        teardrops(&app, &a),
        1,
        "the main window's board is left alone"
    );
    assert_eq!(app.active_tab_path().as_ref(), Some(&a));
}

#[test]
fn repour_all_in_a_board_window_fills_that_boards_pours_only() {
    let (mut app, a, b, window) = two_boards();
    run_in(&mut app, "repour_all", window);
    assert_eq!(unfilled_pours(&app, &b), 0, "the window's board");
    assert_eq!(
        unfilled_pours(&app, &a),
        1,
        "the main window's board is left alone"
    );
}

/// The bug's other face: with a sheet in the main window the rows did
/// nothing at all. Now they reach the window's board whatever main shows.
#[test]
fn remove_teardrops_in_a_board_window_runs_while_main_shows_no_board() {
    let mut app = app_with_main();
    let b = PathBuf::from("b.snxpcb");
    let board = board_with_a_teardrop_and_an_unfilled_pour(300);
    app.open_pcb_tab(b.clone(), "board".into(), board);
    let window = undock(&mut app, &b);
    run_in(&mut app, "remove_teardrops", window);
    assert_eq!(teardrops(&app, &b), 0);
}
