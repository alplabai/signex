//! #2474: each of the 15 base rules is in the rule language and in
//! `signex-drc`, and has a board that passes it and one that breaks it.

use std::collections::BTreeSet;

use signex_drc::{DrcRules, run};
use signex_types::pcb::PcbBoard;

use crate::compile;
use crate::fixture::{BASE_RULES, BaseRule};

fn kinds_on(board: &PcbBoard) -> BTreeSet<&'static str> {
    let compiled = compile(BASE_RULES);
    assert!(
        compiled.diagnostics.is_empty(),
        "{:?}",
        compiled.diagnostics
    );
    assert_eq!(compiled.rules.len(), 9);
    let rules = DrcRules::for_board(board, &[])
        .expect("rules")
        .with_user_rules(board, &[], compiled.rules);
    let report = run(board, &rules);
    assert!(report.skipped.is_empty(), "{:?}", report.skipped);
    report.violations.iter().map(|v| v.kind.rule_id()).collect()
}

fn kinds(rule: BaseRule, broken: bool) -> BTreeSet<&'static str> {
    kinds_on(&rule.board(broken))
}

#[test]
fn every_base_rule_passes_its_clean_board_and_finds_its_broken_one() {
    for rule in BaseRule::ALL {
        assert_eq!(
            kinds(rule, false),
            BTreeSet::new(),
            "{rule:?} passing board"
        );
        assert_eq!(
            kinds(rule, true),
            BTreeSet::from([rule.kind_id()]),
            "{rule:?} broken board"
        );
        for (board, kind) in rule.also_broken() {
            assert_eq!(kinds_on(&board), BTreeSet::from([kind]), "{rule:?} {kind}");
        }
    }
}

#[test]
fn a_track_over_the_maximum_width_is_measured_against_the_maximum() {
    let (board, _) = BaseRule::TrackWidth.also_broken().remove(0);
    let compiled = compile(BASE_RULES);
    let rules =
        DrcRules::for_board(&board, &[])
            .unwrap()
            .with_user_rules(&board, &[], compiled.rules);
    let report = run(&board, &rules);
    let v = &report.violations[0];
    assert_eq!(v.measured_nm, 1_200_000);
    assert_eq!(v.required_nm, 1_000_000);
}

#[test]
fn a_written_base_rule_names_itself_on_its_finding() {
    let board = BaseRule::HoleToHole.board(true);
    let compiled = compile(BASE_RULES);
    let rules =
        DrcRules::for_board(&board, &[])
            .unwrap()
            .with_user_rules(&board, &[], compiled.rules);
    let report = run(&board, &rules);
    let v = &report.violations[0];
    assert_eq!(v.rule.as_deref(), Some("user::holes"));
    assert_eq!(v.required_nm, 500_000);
    assert_eq!(v.measured_nm, 450_000);
}
