//! #2326 — a board tool run from operands names its items by uuid, all or
//! nothing: one id that names nothing the tool acts on refuses the whole
//! command, with the id in the reason, and the board does not change.

use signex_i18n::RenderTranslatable;
use uuid::Uuid;

use super::copper_tests::{app_with_board, id, live, run_operands};
use crate::app::command::CommandOutcome;

/// `command` with `ids` refused for `unknown`, the board left as it was.
fn refused_for(command: &str, ids: &[Uuid], unknown: Uuid) {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let before = live(&app, &path);
    let list: Vec<String> = ids.iter().map(Uuid::to_string).collect();
    let ids = format!("ids={}", list.join(","));
    let outcome = run_operands(&mut app, &path, command, &[&ids]);
    let CommandOutcome::Disabled { reason } = &outcome else {
        panic!("{command}: {outcome:?}");
    };
    let reason = reason.render_en_us();
    assert!(
        reason.contains(&unknown.to_string()) && reason.contains("nothing was changed"),
        "{command}: {reason}"
    );
    assert_eq!(live(&app, &path), before, "{command} changed the board");
}

#[test]
fn gloss_with_an_unknown_id_is_refused_whole() {
    // Before #2326 the two routes were glossed and the third id skipped.
    let unknown = id(9999);
    refused_for("gloss_selected_routes", &[id(30), id(31), unknown], unknown);
}

#[test]
fn a_board_shape_with_an_id_that_is_no_line_is_refused_whole() {
    // id(20) is a via: on the board, but no line the outline is made of.
    let lines = [id(80), id(81), id(82), id(83), id(20)];
    refused_for("board_shape_from_selection", &lines, id(20));
}

#[test]
fn a_keepout_naming_zones_and_lines_is_refused_whole() {
    // Before, the zone became a keepout and the four lines were skipped.
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let before = live(&app, &path);
    let list: Vec<String> = [id(10), id(80), id(81), id(82), id(83)]
        .iter()
        .map(Uuid::to_string)
        .collect();
    let ids = format!("ids={}", list.join(","));
    let outcome = run_operands(&mut app, &path, "keepout_from_selection", &[&ids]);
    let CommandOutcome::Disabled { reason } = &outcome else {
        panic!("{outcome:?}");
    };
    let reason = reason.render_en_us();
    assert!(reason.contains("not both"), "{reason}");
    assert_eq!(live(&app, &path), before);
}

/// `stitch_vias id=` keeps its pre-#2326 reading: empty comma parts are
/// skipped, and two zones are refused as more than one.
#[test]
fn stitching_a_zone_named_with_a_trailing_comma_runs() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let zone = format!("id={},", id(10));
    let outcome = run_operands(&mut app, &path, "stitch_vias", &[&zone, "pitch=2"]);
    assert!(
        matches!(outcome, CommandOutcome::Answered(_)),
        "{outcome:?}"
    );
    let two = format!("id={},{}", id(10), id(11));
    let outcome = run_operands(&mut app, &path, "stitch_vias", &[&two]);
    assert!(
        matches!(&outcome, CommandOutcome::Disabled { reason } if reason.render_en_us().contains("names one zone")),
        "{outcome:?}"
    );
}

#[test]
fn a_keepout_with_an_unknown_id_is_refused_whole() {
    let unknown = id(9999);
    let lines = [id(80), id(81), id(82), id(83), unknown];
    refused_for("keepout_from_selection", &lines, unknown);
}
