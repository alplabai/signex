//! #2474: each of the 15 base rules, through `check_drc` and Run DRC: the
//! board that passes it gives no finding, the board that breaks it a
//! finding of that rule alone, and both commands find the same.

use signex_drc_dsl::fixture::{BASE_RULES, BaseRule};
use signex_types::pcb::PcbBoard;
use signex_types::project::ProjectRules;

use super::check_drc_tests::{
    BOARD, dsl, open_board, report, rules_and_items, run_drc, run_drc_rules_and_items, set_rules,
    write_board,
};
use crate::app::command::project_name_tests::project_app;

/// `(check_drc's rules, Run DRC's kinds of finding)` for `board`; both
/// commands must find the same.
fn both(label: &str, board: PcbBoard) -> (Vec<String>, Vec<String>) {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(&dir);
    let path = dir.path().join(BOARD);
    write_board(&path, board);
    app.document_state.projects[0].data.pcb_file = Some(BOARD.into());
    set_rules(
        &mut app,
        ProjectRules {
            drc: dsl(BASE_RULES),
            ..ProjectRules::default()
        },
    );
    let checked = report(&mut app, &path, &[]);
    open_board(&mut app, &path);
    let ran = run_drc(&mut app, &path);
    let found = rules_and_items(&checked);
    assert_eq!(found, run_drc_rules_and_items(&ran), "{label}: {checked}");
    let mut kinds: Vec<String> = ran
        .violations
        .iter()
        .map(|v| v.kind.rule_id().to_string())
        .collect();
    kinds.dedup();
    let mut rules: Vec<String> = found.into_iter().map(|(rule, _)| rule).collect();
    rules.dedup();
    (rules, kinds)
}

#[test]
fn every_base_rule_passes_and_fails_through_check_drc_and_run_drc() {
    for rule in BaseRule::ALL {
        let label = format!("{rule:?}");
        let (clean, _) = both(&label, rule.board(false));
        assert!(clean.is_empty(), "{rule:?} passing board: {clean:?}");
        let (broken, kinds) = both(&label, rule.board(true));
        assert_eq!(broken, [rule.rule_id()], "{rule:?} broken board");
        assert_eq!(kinds, [rule.kind_id()], "{rule:?} broken board");
        for (board, kind) in rule.also_broken() {
            let label = format!("{rule:?} {kind}");
            let (broken, kinds) = both(&label, board);
            assert_eq!(broken, [rule.rule_id()], "{label}");
            assert_eq!(kinds, [kind], "{label}");
        }
    }
}
