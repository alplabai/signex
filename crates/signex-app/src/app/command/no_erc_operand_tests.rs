//! #1699: a No-ERC directive's rule list from operands, the agent's and the
//! CLI's path: `place_no_erc rules=..` and `edit_no_erc_rules`. Lists are
//! stored sorted, an edit is one undo step, an unknown rule id is refused.

use super::*;
use crate::app::Tool;
use crate::app::handlers::canvas::placement_tests::{app_with_tool, doc, grid};
use crate::keymap::AppCommandId;

const UNUSED_PIN: &str = "builtin::unused_pin";
const PIN_TYPE: &str = "builtin::pin_type_conflict";

fn command(id: &str) -> AppCommandId {
    AppCommandId::new(id).expect("a catalog id")
}

fn run(app: &mut Signex, id: &str, operands: &[&str]) -> CommandOutcome {
    let id = command(id);
    let args = CommandArgs::parse_operands(&id, operands).expect("operands");
    app.run_operands_settled(&id, &args).0
}

fn undo(app: &mut Signex) {
    let _ = app.dispatch_command_reporting(&command("undo"), CommandArgs::none(), None);
}

fn refused_with(outcome: &CommandOutcome, needle: &str) -> bool {
    matches!(outcome, CommandOutcome::Disabled { reason } if reason.contains(needle))
}

/// `place_no_erc` at an on-grid point with `rules`, on a fresh sheet.
fn place(rules: &str) -> (Signex, CommandOutcome) {
    let mut app = app_with_tool(Tool::Select);
    let at = grid(8.0, 16.0);
    let at = format!("at={},{}", at.x, at.y);
    let operands = [at.as_str(), rules];
    let outcome = run(&mut app, "place_no_erc", &operands);
    (app, outcome)
}

/// The rule list of the sheet's first No-ERC directive.
fn rules(app: &Signex) -> Vec<String> {
    doc(app).no_erc_directives[0].rules.clone()
}

#[test]
fn place_no_erc_stores_its_rules_sorted() {
    let (app, outcome) = place(&format!("rules={UNUSED_PIN},{PIN_TYPE}"));
    assert!(!refused_with(&outcome, ""), "{outcome:?}");
    assert_eq!(rules(&app), vec![PIN_TYPE, UNUSED_PIN]);
}

#[test]
fn place_no_erc_with_an_unknown_rule_is_refused() {
    let (app, outcome) = place("rules=builtin::nonsense");
    assert!(refused_with(&outcome, "not an ERC rule id"), "{outcome:?}");
    assert!(doc(&app).no_erc_directives.is_empty());
}

#[test]
fn edit_no_erc_rules_sets_the_list_in_one_undo_step_and_left_out_is_every_rule() {
    let (mut app, _) = place(&format!("rules={UNUSED_PIN}"));
    let id = doc(&app).no_erc_directives[0].uuid.to_string();
    let id_op = format!("id={id}");
    let set = format!("rules={PIN_TYPE},{UNUSED_PIN},{PIN_TYPE}");

    let outcome = run(&mut app, "edit_no_erc_rules", &[&id_op, &set]);
    let answer = match outcome {
        CommandOutcome::Answered(value) => value,
        other => panic!("not answered: {other:?}"),
    };
    assert_eq!(answer["id"], id.as_str());
    assert_eq!(rules(&app), vec![PIN_TYPE, UNUSED_PIN]);

    undo(&mut app);
    assert_eq!(rules(&app), vec![UNUSED_PIN]);

    let outcome = run(&mut app, "edit_no_erc_rules", &[&id_op]);
    assert!(
        matches!(outcome, CommandOutcome::Answered(_)),
        "{outcome:?}"
    );
    assert!(rules(&app).is_empty());
}

#[test]
fn edit_no_erc_rules_on_an_unknown_id_is_refused() {
    let (mut app, _) = place(&format!("rules={UNUSED_PIN}"));
    let id_op = format!("id={}", uuid::Uuid::now_v7());
    let outcome = run(&mut app, "edit_no_erc_rules", &[&id_op, "rules=*"]);
    assert!(refused_with(&outcome, "No-ERC"), "{outcome:?}");
    assert_eq!(rules(&app), vec![UNUSED_PIN]);
}
