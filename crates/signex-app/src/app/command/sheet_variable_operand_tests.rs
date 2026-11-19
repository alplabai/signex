//! `set_sheet_variable` from operands (the CLI and agent path) and from the
//! Sheet Variables rows in Properties: one call, one engine edit and one undo
//! step each, the same sheet either way.

use std::path::Path;

use signex_types::schematic::SchematicSheet;

use super::{CommandArgs, CommandOutcome};
use crate::app::handlers::editing_commands::net_color_driver::add_sheet_with;
use crate::app::{Message, Signex};
use crate::dock::DockMessage;
use crate::keymap::AppCommandId;
use crate::panels::{PanelMsg, SheetVariableMsg};

fn app_in(dir: &Path) -> Signex {
    let (mut app, _task) = Signex::new();
    add_sheet_with(
        &mut app,
        &dir.join("main.snxsch"),
        SchematicSheet::default(),
    );
    app
}

fn run(app: &mut Signex, operands: &[&str]) -> CommandOutcome {
    let id = AppCommandId::new("set_sheet_variable").expect("a catalog id");
    let args = CommandArgs::parse_operands(&id, operands).expect("operands");
    app.run_operands_settled(&id, &args).0
}

fn refusal(outcome: CommandOutcome) -> String {
    match outcome {
        CommandOutcome::Disabled { reason } => reason,
        other => panic!("not refused: {other:?}"),
    }
}

fn variables(app: &Signex) -> Vec<(String, String)> {
    let engine = app.document_state.active_engine().expect("engine");
    let vars = &engine.document().variables;
    vars.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
}

fn undo(app: &mut Signex) {
    let undo = AppCommandId::new("undo").expect("id");
    let _ = app.dispatch_command_reporting(&undo, CommandArgs::none(), None);
}

fn panel(app: &mut Signex, edit: SheetVariableMsg) {
    let _ = app.update(Message::Dock(DockMessage::Panel(PanelMsg::SheetVariables(
        edit,
    ))));
}

#[test]
fn the_command_sets_and_removes_a_variable_one_undo_step_each() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = app_in(dir.path());
    let CommandOutcome::Answered(answer) = run(&mut app, &["name=GAIN", "value== 2 * 3"]) else {
        panic!("not answered");
    };
    assert_eq!(answer["value"], "= 2 * 3");
    assert_eq!(variables(&app), [("GAIN".into(), "= 2 * 3".into())]);
    assert!(matches!(
        run(&mut app, &["name=GAIN", "remove=true"]),
        CommandOutcome::Answered(_)
    ));
    assert!(variables(&app).is_empty());
    undo(&mut app);
    assert_eq!(variables(&app), [("GAIN".into(), "= 2 * 3".into())]);
    undo(&mut app);
    assert!(variables(&app).is_empty());
}

#[test]
fn the_command_and_the_panel_leave_the_same_sheet() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut by_command = app_in(dir.path());
    run(&mut by_command, &["name=VDD", "value=3.3"]);
    let other = tempfile::tempdir().expect("tempdir");
    let mut by_panel = app_in(other.path());
    panel(&mut by_panel, SheetVariableMsg::NewNameTyped("VDD".into()));
    panel(&mut by_panel, SheetVariableMsg::Added);
    panel(
        &mut by_panel,
        SheetVariableMsg::ValueTyped {
            name: "VDD".into(),
            text: "3.3".into(),
        },
    );
    panel(&mut by_panel, SheetVariableMsg::ValueSubmitted("VDD".into()));
    assert_eq!(variables(&by_command), variables(&by_panel));
    assert_eq!(variables(&by_command), [("VDD".into(), "3.3".into())]);
}

#[test]
fn a_bad_name_an_unknown_removal_and_remove_with_a_value_are_refused() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = app_in(dir.path());
    for (operands, needle) in [
        (vec!["name=2bad", "value=1"], "not a variable name"),
        (vec!["name=NONE", "remove=true"], "has no variable"),
        (vec!["name=X", "value=1", "remove=true"], "takes no `value`"),
    ] {
        let reason = refusal(run(&mut app, &operands));
        assert!(reason.contains(needle), "{operands:?}: {reason}");
    }
    assert!(variables(&app).is_empty());
}
