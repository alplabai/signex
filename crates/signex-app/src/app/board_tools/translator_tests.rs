//! #2326 slice 6 — Gloss, Board Shape, Keepout and Arrange in Box from the
//! menu run their operand handler through the translator, on the items the
//! menu row acted on before it: the board ends as the handler leaves it on
//! those ids, and a selection holding items the tool does not act on is
//! still accepted rather than refused whole.

use pcb_geom::{ItemRef, PickKind};
use signex_types::pcb::Point;

use super::copper_tests::{app_with_board, board_of, id, live, run, run_operands};
use crate::app::command::CommandOutcome;

/// The board after `command` from operands with `operands` on a fresh app.
fn by_operands(command: &str, operands: &[&str]) -> String {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let outcome = run_operands(&mut app, &path, command, operands);
    assert!(
        matches!(outcome, CommandOutcome::Answered(_)),
        "{command}: {outcome:?}"
    );
    live(&app, &path)
}

fn ids(list: &[u128]) -> String {
    let ids: Vec<String> = list.iter().map(|n| id(*n).to_string()).collect();
    format!("ids={}", ids.join(","))
}

/// `command` from the menu on `selection` leaves the board as its operand
/// handler does on `acted_on`, the items the menu row acted on.
fn menu_matches_operands(command: &str, selection: &[ItemRef], acted_on: &[u128]) {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let before = live(&app, &path);
    app.select_pcb_items(selection);
    run(&mut app, command);
    let after = live(&app, &path);
    assert_ne!(after, before, "{command} changed nothing");
    assert_eq!(after, by_operands(command, &[&ids(acted_on)]), "{command}");
}

fn zone(app: &crate::app::Signex, path: &std::path::Path, n: u128) -> ItemRef {
    let found = board_of(app, path).zones.iter().find(|z| z.uuid == id(n));
    let kind = PickKind::of_zone(found.expect("zone")).expect("a pickable area");
    ItemRef::Zone { zone: id(n), kind }
}

#[test]
fn gloss_from_the_menu_glosses_the_selected_tracks_and_skips_the_rest() {
    // The via and the part are no tracks: the menu row passed over them.
    let selection = [30, 31].map(|n| ItemRef::Track(id(n)));
    let mut selection = selection.to_vec();
    selection.extend([ItemRef::Via(id(20)), ItemRef::Footprint(id(50))]);
    menu_matches_operands("gloss_selected_routes", &selection, &[30, 31]);
}

#[test]
fn board_shape_from_the_menu_uses_the_selected_lines_and_skips_the_via() {
    let mut selection: Vec<ItemRef> = [80, 81, 82, 83].map(|n| ItemRef::Track(id(n))).to_vec();
    selection.push(ItemRef::Via(id(20)));
    menu_matches_operands("board_shape_from_selection", &selection, &[80, 81, 82, 83]);
}

#[test]
fn keepout_from_the_menu_on_a_zone_and_lines_makes_the_zone_a_keepout() {
    // From operands, zones and lines together are refused; the menu row
    // made the zone a keepout and left the lines, and still does.
    let dir = tempfile::tempdir().unwrap();
    let (app, path) = app_with_board(dir.path());
    let mut selection = vec![zone(&app, &path, 10)];
    selection.extend([80, 81, 82, 83].map(|n| ItemRef::Track(id(n))));
    menu_matches_operands("keepout_from_selection", &selection, &[10]);
}

#[test]
fn arrange_in_box_from_two_clicks_is_its_operand_handler_on_the_corners() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    app.select_pcb_items(&[ItemRef::Footprint(id(50)), ItemRef::Footprint(id(60))]);
    run(&mut app, "arrange_in_box");
    assert!(app.pcb_arrange_box_clicked(Point::new(0.0, 20.0)).is_some());
    assert!(
        app.pcb_arrange_box_clicked(Point::new(10.0, 30.0))
            .is_some()
    );
    let want = by_operands(
        "arrange_in_box",
        &[&ids(&[50, 60]), "start=0,20", "end=10,30"],
    );
    assert_eq!(live(&app, &path), want);
}

#[test]
fn a_translated_refusal_is_said_in_messages_as_before() {
    let _ = crate::diagnostics::init_logging();
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let before = live(&app, &path);
    app.select_pcb_items(&[ItemRef::Via(id(20))]);
    let mark = crate::diagnostics::newest_entry_id();
    run(&mut app, "gloss_selected_routes");
    assert_eq!(live(&app, &path), before);
    let said = crate::diagnostics::entries_since(mark).entries;
    assert!(
        said.iter().any(|e| e
            .message
            .contains("nothing is selected that this can act on")),
        "{said:?}"
    );
}
