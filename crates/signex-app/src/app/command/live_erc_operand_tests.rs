//! `toggle_live_erc` (#1288): the key or palette row flips the live ERC
//! preference, and a CLI run or an agent sets it with `enabled` and reads
//! back the answer.

use super::{CommandArgs, CommandOutcome};
use crate::app::Signex;
use crate::keymap::{AppCommandId, OperandKind, metadata_for_str};

fn command() -> AppCommandId {
    AppCommandId::new("toggle_live_erc").expect("a catalog id")
}

fn answered(outcome: CommandOutcome) -> serde_json::Value {
    match outcome {
        CommandOutcome::Answered(value) => value,
        other => panic!("not answered: {other:?}"),
    }
}

fn run(app: &mut Signex, operands: &[&str]) -> CommandOutcome {
    let args = CommandArgs::parse_operands(&command(), operands).expect("operands");
    app.run_operands_settled(&command(), &args).0
}

#[test]
fn the_catalog_row_takes_an_optional_enabled() {
    let meta = metadata_for_str("toggle_live_erc").expect("row");
    assert_eq!(meta.operands.len(), 1);
    assert_eq!(meta.operands[0].name, "enabled");
    assert_eq!(meta.operands[0].kind, OperandKind::Bool);
    assert!(!meta.operands[0].required);
    assert!(!meta.flags.gui_only, "a CLI run may set it");
}

#[test]
fn enabled_sets_the_preference_and_answers_what_it_left() {
    let (mut app, _task) = Signex::new();
    app.ui_state.live_erc.enabled = true;
    let off = answered(run(&mut app, &["enabled=false"]));
    assert_eq!(
        off,
        serde_json::json!({ "enabled": false, "changed": true })
    );
    assert!(!app.ui_state.live_erc.enabled);
    let again = answered(run(&mut app, &["enabled=false"]));
    assert_eq!(
        again,
        serde_json::json!({ "enabled": false, "changed": false })
    );
    let on = answered(run(&mut app, &["enabled=true"]));
    assert_eq!(on["enabled"], true);
    assert!(app.ui_state.live_erc.enabled);
}

/// The palette and a key send no operands: the bridge flips the preference,
/// as the operand form does without `enabled`.
#[test]
fn the_palette_row_flips_it_like_the_bare_operand_form() {
    let (mut app, _task) = Signex::new();
    app.ui_state.live_erc.enabled = true;
    let _ = app.dispatch_command(&command(), CommandArgs::none(), None);
    assert!(!app.ui_state.live_erc.enabled, "the GUI path");
    let flipped = answered(run(&mut app, &[]));
    assert_eq!(flipped["enabled"], true);
    assert!(app.ui_state.live_erc.enabled);
}

/// A headless run with no operands still answers with the preference.
#[test]
fn a_headless_run_answers_with_the_preference() {
    let (mut app, _task) = Signex::new();
    app.ui_state.live_erc.enabled = false;
    let (outcome, _task) = app.run_headless_command_in_test(&command(), CommandArgs::none());
    assert_eq!(answered(outcome)["enabled"], true);
    let args = CommandArgs::parse_operands(&command(), &["enabled=false"]).expect("operands");
    let (outcome, _task) = app.run_headless_command_in_test(&command(), args);
    assert_eq!(
        answered(outcome),
        serde_json::json!({ "enabled": false, "changed": true })
    );
}
