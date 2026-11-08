//! #661, #1934: `assign_net_class` runs the label picker's path from operands,
//! so the pick lands on the active sheet's timeline like a click on the picker.

use signex_types::net_class::NetClass;

use super::class_picker::{assign, child_label_selected};
use crate::app::Signex;
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::keymap::AppCommandId;

/// A platform-absolute project directory for fixture `name`.
fn dir(name: &str) -> String {
    let path = std::env::temp_dir().join(format!("signex-{name}"));
    path.to_string_lossy().into_owned()
}

fn run(app: &mut Signex, operands: &[&str]) -> CommandOutcome {
    let command = AppCommandId::new("assign_net_class").expect("catalog id");
    // A caller names its document; these tests name the active sheet unless
    // they name another.
    let active = app
        .document_state
        .active_path
        .as_ref()
        .map(|path| format!("document={}", path.display()));
    let mut named: Vec<&str> = operands.to_vec();
    if !operands.iter().any(|o| o.starts_with("document=")) {
        named.extend(active.as_deref());
    }
    let args = CommandArgs::parse_operands(&command, &named).expect("operands parse");
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
    let (mut picked, ..) = child_label_selected(&dir("assign-click"));
    let project = picked.document_state.active_project.expect("project");
    assign(&mut picked, project, "/LOCAL", "Power");

    let (mut by_command, ..) = child_label_selected(&dir("assign-command"));
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
    let (mut app, ..) = child_label_selected(&dir("assign-none"));
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
    let (mut app, ..) = child_label_selected(&dir("assign-refuse"));
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
    let missing = format!(
        "document={}",
        std::path::Path::new(&dir("assign-elsewhere"))
            .join("x.snxsch")
            .display()
    );
    let elsewhere = reason(run(&mut app, &["net_name=/LOCAL", "class=Power", &missing]));
    assert!(
        elsewhere.contains("not an open or referenced"),
        "{elsewhere}"
    );

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

/// The same by the sheet's id (#2172): found through the document list, the
/// active sheet, tab and the interaction state are untouched.
#[test]
fn a_document_id_assigns_on_another_open_sheet_without_activating_it() {
    let (mut app, ..) = child_label_selected(&dir("assign-by-id"));
    let top = top_sheet(&app);
    let id = app.document_state.engines[&top].document().uuid;
    let (active, tab) = (
        app.document_state.active_path.clone(),
        app.document_state.active_tab,
    );
    let top_at = app.document_state.engines[&top].undo_position();

    let given = format!("document={id}");
    let outcome = run(&mut app, &["net_name=VCC", "class=Power", &given]);

    assert_eq!(outcome, CommandOutcome::Ran, "{outcome:?}");
    assert_eq!(classes(&app)[0].nets, ["VCC"]);
    assert_eq!(app.document_state.engines[&top].undo_position(), top_at + 1);
    assert_eq!(app.document_state.active_path, active);
    assert_eq!(app.document_state.active_tab, tab);
}

/// `document=` names any open sheet: the pick lands on that sheet's timeline,
/// the active sheet and tab are untouched, and undoing it there reverts it.
#[test]
fn a_path_operand_assigns_on_another_open_sheet_without_activating_it() {
    let (mut app, ..) = child_label_selected(&dir("assign-by-path"));
    let (child, top) = (
        app.document_state.active_path.clone().unwrap(),
        top_sheet(&app),
    );
    let (tab, child_at) = (app.document_state.active_tab, child_position(&app));
    let top_at = app.document_state.engines[&top].undo_position();

    let given = format!("document={}", top.display());
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
    let (mut app, ..) = child_label_selected(&dir("assign-bare"));
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

fn did_nothing_lines() -> usize {
    crate::diagnostics::recent_entries()
        .iter()
        .filter(|entry| entry.message.contains("did nothing"))
        .count()
}

/// Only the missing-operand refusal is written to the Messages panel by
/// `dispatch_command`; every other command is exactly as on trunk.
#[test]
fn only_the_operand_refusal_is_logged_by_a_key_or_palette_dispatch() {
    let _ = crate::diagnostics::init_logging();
    let (mut app, ..) = child_label_selected(&dir("assign-log"));
    let id = |name| AppCommandId::new(name).expect("catalog id");

    let before = did_nothing_lines();
    let _ = app.dispatch_command(&id("assign_net_class"), CommandArgs::none(), None);
    assert_eq!(
        did_nothing_lines(),
        before + 1,
        "the refusal says why, once"
    );

    let before = did_nothing_lines();
    for name in ["undo", "redo", "leave_sheet"] {
        let _ = app.dispatch_command(&id(name), CommandArgs::none(), None);
    }
    assert_eq!(
        did_nothing_lines(),
        before,
        "nothing new for other commands"
    );
}
