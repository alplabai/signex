//! #661, #1934: `assign_net_class` runs the label picker's path from operands,
//! so the pick lands on the active sheet's timeline like a click on the picker.

use signex_types::net_class::NetClass;

use super::class_picker::{assign, child_label_selected};
use crate::app::Signex;
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::keymap::AppCommandId;

fn run(app: &mut Signex, operands: &[&str]) -> CommandOutcome {
    let command = AppCommandId::new("assign_net_class").expect("catalog id");
    let args = CommandArgs::parse_operands(&command, operands).expect("operands parse");
    app.run_operands(&command, &args).0
}

fn classes(app: &Signex) -> Vec<NetClass> {
    app.document_state.projects[0].data.net_classes.clone()
}

fn child_position(app: &Signex) -> usize {
    app.document_state
        .active_engine()
        .expect("active sheet")
        .undo_position()
}

#[test]
fn the_command_is_the_pickers_assignment_and_one_undo_step_of_the_active_sheet() {
    let (mut picked, ..) = child_label_selected("/w/assign-click");
    let project = picked.document_state.active_project.expect("project");
    assign(&mut picked, project, "/LOCAL", "Power");

    let (mut by_command, ..) = child_label_selected("/w/assign-command");
    let before = child_position(&by_command);
    let outcome = run(&mut by_command, &["net_name=/LOCAL", "class=Power"]);

    assert_eq!(outcome, CommandOutcome::Ran);
    assert_eq!(classes(&by_command), classes(&picked), "the click's edit");
    assert_eq!(
        child_position(&by_command),
        before + 1,
        "on the sheet's timeline"
    );
    assert_eq!(child_position(&by_command), child_position(&picked));
    let table = by_command.document_state.projects[0].path.clone();
    assert!(by_command.document_state.is_document_dirty(&table));

    assert!(by_command.undo_active_sheet());
    assert!(
        classes(&by_command)[0].nets.is_empty(),
        "one undo reverts it"
    );
    assert!(!by_command.document_state.is_document_dirty(&table));
}

#[test]
fn no_class_takes_the_net_out_of_its_listed_class() {
    let (mut app, ..) = child_label_selected("/w/assign-none");
    assert_eq!(
        run(&mut app, &["net_name=/LOCAL", "class=Power"]),
        CommandOutcome::Ran
    );
    assert_eq!(classes(&app)[0].nets, ["/LOCAL"]);

    assert_eq!(run(&mut app, &["net_name=/LOCAL"]), CommandOutcome::Ran);
    assert!(classes(&app)[0].nets.is_empty());
}

#[test]
fn the_command_refuses_what_it_cannot_do_and_changes_nothing() {
    let (mut app, ..) = child_label_selected("/w/assign-refuse");
    let reason = |outcome: CommandOutcome| match outcome {
        CommandOutcome::Disabled { reason } => reason,
        other => panic!("expected a refusal, got {other:?}"),
    };
    let before = classes(&app);
    let position = child_position(&app);

    let unknown_net = reason(run(&mut app, &["net_name=NOPE", "class=Power"]));
    assert!(unknown_net.contains("not a net"), "{unknown_net}");
    let unknown_class = reason(run(&mut app, &["net_name=/LOCAL", "class=Nope"]));
    assert!(unknown_class.contains("no net class"), "{unknown_class}");
    let elsewhere = reason(run(
        &mut app,
        &["net_name=/LOCAL", "class=Power", "path=/w/elsewhere.snxsch"],
    ));
    assert!(elsewhere.contains("not an open sheet"), "{elsewhere}");

    assert_eq!(classes(&app), before);
    assert_eq!(child_position(&app), position, "a refusal records no step");

    assert_eq!(
        run(&mut app, &["net_name=/LOCAL", "class=Power"]),
        CommandOutcome::Ran
    );
    let again = reason(run(&mut app, &["net_name=/LOCAL", "class=Power"]));
    assert!(again.contains("already"), "{again}");
    assert_eq!(
        child_position(&app),
        position + 1,
        "the repeat recorded nothing"
    );
}

fn top_sheet(app: &Signex) -> std::path::PathBuf {
    app.document_state.projects[0]
        .path
        .with_file_name("top.snxsch")
}

/// `path=` names any open sheet: the pick lands on that sheet's timeline,
/// the active sheet and tab are untouched, and undoing it there reverts it.
#[test]
fn a_path_operand_assigns_on_another_open_sheet_without_activating_it() {
    let (mut app, ..) = child_label_selected("/w/assign-by-path");
    let (child, top) = (
        app.document_state.active_path.clone().unwrap(),
        top_sheet(&app),
    );
    let (tab, child_at) = (app.document_state.active_tab, child_position(&app));
    let top_at = app.document_state.engines[&top].undo_position();

    let given = format!("path={}", top.display());
    let outcome = run(&mut app, &["net_name=VCC", "class=Power", &given]);

    assert_eq!(outcome, CommandOutcome::Ran, "{outcome:?}");
    assert_eq!(classes(&app)[0].nets, ["VCC"]);
    assert_eq!(app.document_state.engines[&top].undo_position(), top_at + 1);
    assert_eq!(
        child_position(&app),
        child_at,
        "the active sheet is untouched"
    );
    assert_eq!(app.document_state.active_path.as_ref(), Some(&child));
    assert_eq!(app.document_state.active_tab, tab);

    app.document_state.active_path = Some(top);
    assert!(app.undo_active_sheet());
    assert!(
        classes(&app)[0].nets.is_empty(),
        "undone from its own sheet"
    );
}

/// Without operands the command does nothing on a surface that has none:
/// refused with a reason, no panel, tab or focus change.
#[test]
fn without_operands_the_command_is_refused_and_changes_nothing_else() {
    let (mut app, ..) = child_label_selected("/w/assign-bare");
    let command = AppCommandId::new("assign_net_class").expect("catalog id");
    let (path, tab) = (
        app.document_state.active_path.clone(),
        app.document_state.active_tab,
    );
    let front = app
        .document_state
        .dock
        .is_frontmost(crate::panels::PanelKind::Properties);

    let (outcome, task) = app.dispatch_command_reporting(&command, CommandArgs::none(), None);
    let CommandOutcome::Disabled { reason } = outcome else {
        panic!("expected a refusal, got {outcome:?}");
    };
    assert!(
        reason.contains("net_name") && reason.contains("Properties"),
        "{reason}"
    );
    assert_eq!(task.units(), 0);
    let _ = app.dispatch_command(&command, CommandArgs::none(), None);

    assert_eq!(app.document_state.active_path, path);
    assert_eq!(app.document_state.active_tab, tab);
    assert_eq!(
        app.document_state
            .dock
            .is_frontmost(crate::panels::PanelKind::Properties),
        front,
        "no panel opened or closed"
    );
    assert!(classes(&app)[0].nets.is_empty());
}
