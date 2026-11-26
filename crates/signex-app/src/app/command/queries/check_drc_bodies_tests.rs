//! #2939: the component height and 3D body clearance rules, each on a board
//! that passes it and one that breaks it, through `check_drc` and Run DRC:
//! both find the same, and a finding names its parts by designator.

use serde_json::Value;
use signex_drc_dsl::fixture::BaseRule;
use signex_types::pcb::PcbBoard;
use signex_types::project::ProjectRules;

use super::check_drc_tests::{
    BOARD, dsl, open_board, report, rules_and_items, run_drc, run_drc_rules_and_items, set_rules,
    write_board,
};
use crate::app::command::project_name_tests::project_app;

/// Two parts whose 2 mm courtyards are 1 mm apart, standing `heights` mm
/// tall (their stored Height).
fn two_parts(heights: [f64; 2]) -> PcbBoard {
    let mut board = BaseRule::Courtyard.board(false);
    for (footprint, height) in board.footprints.iter_mut().zip(heights) {
        footprint.component.height_mm = height;
    }
    board
}

/// `check_drc`'s report for `board` under `rules`, after checking that Run
/// DRC finds the same.
fn checked(rules: &str, board: PcbBoard) -> Value {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(&dir);
    let path = dir.path().join(BOARD);
    write_board(&path, board);
    app.document_state.projects[0].data.pcb_file = Some(BOARD.into());
    set_rules(
        &mut app,
        ProjectRules {
            drc: dsl(rules),
            ..ProjectRules::default()
        },
    );
    let found = report(&mut app, &path, &[]);
    open_board(&mut app, &path);
    let ran = run_drc(&mut app, &path);
    assert_eq!(
        rules_and_items(&found),
        run_drc_rules_and_items(&ran),
        "{found}"
    );
    found
}

fn rules_of(report: &Value) -> Vec<String> {
    rules_and_items(report)
        .into_iter()
        .map(|(rule, _)| rule)
        .collect()
}

const HEIGHT: &str = "rule tall on component require height <= 5mm then error";

#[test]
fn a_part_over_the_height_limit_is_found_and_one_under_it_passes() {
    assert!(rules_of(&checked(HEIGHT, two_parts([3.0, 4.0]))).is_empty());
    let found = checked(HEIGHT, two_parts([3.0, 7.0]));
    assert_eq!(rules_of(&found), ["user::tall"]);
    let finding = &found["findings"][0];
    assert_eq!(
        finding["evidence"]["designators"],
        serde_json::json!(["U2"])
    );
    assert_eq!(finding["evidence"]["measured_nm"], 7_000_000);
    assert_eq!(finding["evidence"]["required_nm"], 5_000_000);
}

#[test]
fn two_bodies_closer_than_the_limit_are_found_and_farther_ones_pass() {
    let wide = "rule bodies on component require clearance >= 1.5mm then error";
    let found = checked(wide, two_parts([2.0, 2.0]));
    assert_eq!(rules_of(&found), ["user::bodies"]);
    let finding = &found["findings"][0];
    assert_eq!(
        finding["evidence"]["designators"],
        serde_json::json!(["U1", "U2"])
    );
    assert_eq!(finding["evidence"]["measured_nm"], 1_000_000);
    let narrow = "rule bodies on component require clearance >= 0.5mm then error";
    assert!(rules_of(&checked(narrow, two_parts([2.0, 2.0]))).is_empty());
}

#[test]
fn a_board_with_no_component_rule_reports_as_before() {
    let found = checked("", two_parts([3.0, 70.0]));
    assert!(rules_of(&found).is_empty(), "{found}");
}
