//! #1689-#1693: the `project_*` commands run the tab's commit path (one
//! General step each), answer with their section as JSON, and refuse what
//! the tab refuses, with the reason.

use std::path::PathBuf;

use signex_i18n::RenderTranslatable;

use serde_json::Value;

use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::state::ProjectId;
use crate::app::{Message, ProjectMsg, ProjectTabMsg, Signex};
use crate::keymap::AppCommandId;

fn app() -> (Signex, ProjectId) {
    let dir = std::env::temp_dir().join(format!("signex-prj-cmd-{}", uuid::Uuid::now_v7()));
    let mut app = app_workspace(&dir.to_string_lossy(), &["a.snxsch", "b.snxsch"]);
    app.ui_state.erc_severity_override.clear();
    app.ui_state.pin_matrix_overrides.clear();
    let id = app.document_state.projects[0].id;
    (app, id)
}

fn run(app: &mut Signex, id: &str, operands: &[&str]) -> CommandOutcome {
    let document = format!("document={}", app.document_state.projects[0].path.display());
    let mut all = vec![document.as_str()];
    all.extend_from_slice(operands);
    let command = AppCommandId::new(id).expect("a catalog id");
    let args = CommandArgs::parse_operands(&command, &all).expect("operands parse");
    match app.run_query(&command, &args) {
        Some((outcome, _)) => outcome,
        None => app.run_operands(&command, &args).0,
    }
}

fn answer(app: &mut Signex, id: &str, operands: &[&str]) -> Value {
    match run(app, id, operands) {
        CommandOutcome::Answered(value) => value,
        other => panic!("{id} did not answer: {other:?}"),
    }
}

fn refusal(app: &mut Signex, id: &str, operands: &[&str]) -> String {
    match run(app, id, operands) {
        CommandOutcome::Disabled { reason } => reason.render_en_us(),
        other => panic!("{id} was not refused: {other:?}"),
    }
}

#[test]
fn project_settings_answers_every_section() {
    let (mut app, _) = app();
    let value = answer(&mut app, "project_settings", &[]);
    assert_eq!(value["sheets"][0]["file"], "a.snxsch");
    assert_eq!(value["sheets"][1]["page"], 2);
    assert_eq!(value["variants"]["active"], "Base");
    assert!(value["libraries"].as_array().is_some_and(Vec::is_empty));
    assert_eq!(value["annotation"]["annotate_undo"], Value::Null);
    assert_eq!(value["annotation"]["erc_rules"], "preferences");
    assert_eq!(value["version_control"]["commit_on_save"], false);
}

/// The command and the Move down button make the same change.
#[test]
fn project_move_sheet_matches_the_tab_button() {
    let (mut by_command, _) = app();
    let value = answer(
        &mut by_command,
        "project_move_sheet",
        &["sheet=a.snxsch", "direction=down"],
    );
    assert_eq!(value["changed"], true);
    assert_eq!(value["sheets"][0]["file"], "b.snxsch");

    let (mut by_click, id) = app();
    let _ = by_click.update(Message::Project(ProjectMsg::Tab(
        ProjectTabMsg::SheetMoved {
            project_id: id,
            filename: "a.snxsch".into(),
            step: signex_types::project::ListMove::Down,
        },
    )));
    assert_eq!(
        by_command.document_state.projects[0].data.sheet_order(),
        by_click.document_state.projects[0].data.sheet_order()
    );

    let refused = refusal(
        &mut by_command,
        "project_move_sheet",
        &["sheet=missing.snxsch", "direction=up"],
    );
    assert!(refused.contains("no sheet"), "{refused}");
    let refused = refusal(
        &mut by_command,
        "project_move_sheet",
        &["sheet=a.snxsch", "direction=left"],
    );
    assert!(refused.contains("up or down"), "{refused}");
}

#[test]
fn variant_commands_are_undo_steps_and_refuse_a_taken_name() {
    let (mut app, id) = app();
    answer(&mut app, "project_add_variant", &["name=Lite"]);
    let value = answer(&mut app, "project_set_active_variant", &["name=lite"]);
    assert_eq!(value["variants"]["active"], "Lite");
    let refused = refusal(&mut app, "project_add_variant", &["name=LITE"]);
    assert!(refused.contains("already has a variant"), "{refused}");
    let value = answer(
        &mut app,
        "project_rename_variant",
        &["name=Lite", "value=Mini"],
    );
    assert_eq!(value["variants"]["defined"][0], "Mini");
    assert_eq!(value["variants"]["active"], "Mini");
    let value = answer(&mut app, "project_set_active_variant", &[]);
    assert_eq!(value["variants"]["active"], "Base");
    let value = answer(&mut app, "project_remove_variant", &["name=Mini"]);
    assert!(
        value["variants"]["defined"]
            .as_array()
            .is_some_and(Vec::is_empty)
    );
    // Five steps: add, pick, rename, Base, remove.
    for _ in 0..5 {
        assert!(app.step_project_tab_history(id, true));
    }
    assert!(!app.step_project_tab_history(id, true));
}

#[test]
fn project_move_library_names_the_stored_path() {
    let (mut app, _) = app();
    app.document_state.projects[0].data.libraries = ["x", "y"]
        .iter()
        .map(|name| signex_types::project::LibraryEntry {
            path: PathBuf::from(format!("{name}.snxlib")),
            kind: signex_types::project::LibraryEntryKind::ProjectLocal,
            library_id: None,
            unknown: Default::default(),
        })
        .collect();
    let value = answer(
        &mut app,
        "project_move_library",
        &["library=y.snxlib", "direction=up"],
    );
    assert_eq!(value["libraries"][0]["path"], "y.snxlib");
    assert_eq!(value["libraries"][0]["kind"], "project_local");
    assert_eq!(value["libraries"][0]["state"], "not_mounted");
    let value = answer(
        &mut app,
        "project_move_library",
        &["library=y.snxlib", "direction=up"],
    );
    assert_eq!(value["changed"], false, "already first");
    let refused = refusal(
        &mut app,
        "project_move_library",
        &["library=z.snxlib", "direction=up"],
    );
    assert!(refused.contains("no library"), "{refused}");
}

#[test]
fn project_set_annotate_undo_stores_and_clears_the_choice() {
    let (mut app, _) = app();
    let value = answer(&mut app, "project_set_annotate_undo", &["value=per_sheet"]);
    assert_eq!(value["annotation"]["annotate_undo"], "per_sheet");
    assert_eq!(value["annotation"]["annotate_undo_effective"], "per_sheet");
    let refused = refusal(&mut app, "project_set_annotate_undo", &["value=each"]);
    assert!(refused.contains("per_sheet"), "{refused}");
    let value = answer(&mut app, "project_set_annotate_undo", &[]);
    assert_eq!(value["annotation"]["annotate_undo"], Value::Null);
}

#[test]
fn project_set_version_control_refuses_without_a_repository() {
    let (mut app, _) = app();
    let folder = app.document_state.projects[0]
        .path
        .parent()
        .unwrap()
        .to_path_buf();
    app.document_state
        .project_fs
        .facts
        .set_exists(crate::app::state::project_fs::git_marker(&folder), false);
    let refused = refusal(&mut app, "project_set_version_control", &["on=true"]);
    assert!(refused.contains("Enable Version Control"), "{refused}");
    let value = answer(&mut app, "project_settings", &[]);
    assert_eq!(value["version_control"]["repository"], "absent");
    app.document_state
        .project_fs
        .facts
        .set_exists(crate::app::state::project_fs::git_marker(&folder), true);
    let value = answer(&mut app, "project_set_version_control", &["on=true"]);
    assert_eq!(value["version_control"]["commit_on_save"], true);
    assert_eq!(value["version_control"]["repository"], "present");
}

/// From the palette or a key, with no operands, an edit points at the tab.
#[test]
fn an_edit_without_operands_names_the_project_tab() {
    let (mut app, _) = app();
    let command = AppCommandId::new("project_add_variant").unwrap();
    let (outcome, _) = app.dispatch_command_reporting(&command, CommandArgs::none(), None);
    assert!(
        matches!(&outcome, CommandOutcome::Disabled { reason } if reason.render_en_us().contains("Project tab")),
        "{outcome:?}"
    );
    assert!(
        app.document_state.projects[0]
            .data
            .variant_definitions
            .is_empty()
    );
}
