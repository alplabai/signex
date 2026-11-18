//! #2339: the Requirements section, driven through `Signex::update` and the
//! catalog. Each edit dirties the `.snxprj`, writes nothing until Save, and
//! is one undo step; a refused one changes nothing and says why; Run checks
//! lands its report in the tab; a save writes the list and reads back the
//! same.

use serde_json::{Value, json};
use signex_types::project::{
    RequirementField, RequirementProblem, parse_project, write_project,
};

use super::RequirementsMsg;
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::state::ProjectId;
use crate::app::{Message, ProjectMsg, ProjectTabMsg, Signex};
use crate::keymap::AppCommandId;

/// A project tab over a project file written to a folder of its own; the
/// folder lives as long as the returned guard.
fn app() -> (Signex, ProjectId, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("a folder");
    let mut app = app_workspace(&dir.path().to_string_lossy(), &["a.snxsch"]);
    app.ui_state.erc_severity_override.clear();
    app.ui_state.pin_matrix_overrides.clear();
    let project = &app.document_state.projects[0];
    write_project(&project.path, &project.data).expect("the project file");
    app.open_project_tab(0);
    let id = app.document_state.projects[0].id;
    (app, id, dir)
}

fn send(app: &mut Signex, id: ProjectId, msg: RequirementsMsg) -> iced::Task<Message> {
    app.update(Message::Project(ProjectMsg::Tab(ProjectTabMsg::Requirements {
        project_id: id,
        msg,
    })))
}

fn stored(app: &Signex) -> Vec<Value> {
    app.document_state.projects[0]
        .data
        .requirements
        .iter()
        .map(|entry| entry.raw().clone())
        .collect()
}

fn on_disk(app: &Signex) -> Vec<u8> {
    std::fs::read(&app.document_state.projects[0].path).expect("the project file")
}

fn dirty(app: &Signex) -> bool {
    let path = &app.document_state.projects[0].path;
    app.document_state.is_document_dirty(path)
}

fn undo(app: &mut Signex, id: ProjectId, back: bool) -> bool {
    app.step_project_tab_history(id, back)
}

fn tab(app: &Signex, id: ProjectId) -> &super::RequirementsTabState {
    &app.document_state.project_tabs[&id].requirements
}

fn add(app: &mut Signex, id: ProjectId, kind: &'static str) {
    let _ = send(app, id, RequirementsMsg::AddPicked { kind });
}

fn type_and_submit(app: &mut Signex, id: ProjectId, field: RequirementField, text: &str) {
    let edited = RequirementsMsg::FieldEdited {
        index: 0,
        field,
        text: text.into(),
    };
    let _ = send(app, id, edited);
    let _ = send(app, id, RequirementsMsg::FieldSubmitted { index: 0, field });
}

fn run(app: &mut Signex, id: &str, operands: &[&str]) -> CommandOutcome {
    let document = format!("document={}", app.document_state.projects[0].path.display());
    let mut all = vec![document.as_str()];
    all.extend_from_slice(operands);
    let command = AppCommandId::new(id).expect("a catalog id");
    let args = CommandArgs::parse_operands(&command, &all).expect("operands parse");
    let (outcome, task) = match app.run_query(&command, &args) {
        Some(ran) => ran,
        None => app.run_operands(&command, &args),
    };
    crate::task_driver::TaskDriver::new()
        .settle(app, task)
        .expect("the task settles");
    outcome
}

fn answer(app: &mut Signex, id: &str, operands: &[&str]) -> Value {
    match run(app, id, operands) {
        CommandOutcome::Answered(value) => value,
        other => panic!("{id} did not answer: {other:?}"),
    }
}

fn refusal(app: &mut Signex, id: &str, operands: &[&str]) -> String {
    match run(app, id, operands) {
        CommandOutcome::Disabled { reason } => reason,
        other => panic!("{id} was not refused: {other:?}"),
    }
}

#[test]
fn an_add_is_one_undo_step_that_dirties_and_writes_nothing() {
    let (mut app, id, _dir) = app();
    let before = on_disk(&app);
    add(&mut app, id, "board_size");
    assert_eq!(
        stored(&app),
        [json!({"id": "board_size", "kind": "board_size",
                "max_width_mm": 100, "max_height_mm": 100})]
    );
    assert!(dirty(&app));
    assert_eq!(on_disk(&app), before, "nothing is written until Save");
    assert!(undo(&mut app, id, true));
    assert!(stored(&app).is_empty());
    assert!(!dirty(&app), "undo returned to the file's content");
    assert!(undo(&mut app, id, false));
    assert_eq!(stored(&app).len(), 1);
}

#[test]
fn a_field_commits_on_enter_and_a_refused_value_stays_with_its_reason() {
    let (mut app, id, _dir) = app();
    add(&mut app, id, "board_size");
    type_and_submit(&mut app, id, RequirementField::MaxWidthMm, "80.5");
    assert_eq!(stored(&app)[0]["max_width_mm"], json!(80.5));
    assert!(tab(&app, id).drafts.is_empty());

    type_and_submit(&mut app, id, RequirementField::MaxWidthMm, "-2");
    assert_eq!(stored(&app)[0]["max_width_mm"], json!(80.5), "refused");
    let state = tab(&app, id);
    assert_eq!(
        state.drafts.get(&(0, RequirementField::MaxWidthMm)).map(String::as_str),
        Some("-2"),
        "the typed text stays in its box"
    );
    assert_eq!(
        state.problem,
        Some((Some(0), RequirementProblem::NotPositive("-2".into())))
    );

    // Undo steps the committed value back and shows the stored values again.
    assert!(undo(&mut app, id, true));
    assert_eq!(stored(&app)[0]["max_width_mm"], json!(100));
    assert!(tab(&app, id).drafts.is_empty());
    assert!(tab(&app, id).problem.is_none());
}

#[test]
fn a_pick_commits_at_once_and_toggle_and_remove_are_steps() {
    let (mut app, id, _dir) = app();
    add(&mut app, id, "erc_clean");
    let picked = RequirementsMsg::FieldPicked {
        index: 0,
        field: RequirementField::Severity,
        value: "warning".into(),
    };
    let _ = send(&mut app, id, picked);
    let allow = RequirementsMsg::FieldPicked {
        index: 0,
        field: RequirementField::AllowWarnings,
        value: "true".into(),
    };
    let _ = send(&mut app, id, allow);
    let _ = send(&mut app, id, RequirementsMsg::EnabledToggled { index: 0, on: false });
    assert_eq!(
        stored(&app),
        [json!({"id": "erc_clean", "kind": "erc_clean", "severity": "warning",
                "allow_warnings": true, "enabled": false})]
    );
    let _ = send(&mut app, id, RequirementsMsg::Removed { index: 0 });
    assert!(stored(&app).is_empty());
    for _ in 0..4 {
        assert!(undo(&mut app, id, true));
    }
    assert_eq!(stored(&app), [json!({"id": "erc_clean", "kind": "erc_clean"})]);
}

#[test]
fn run_checks_lands_its_report_in_the_tab_off_the_ui_thread() {
    let (mut app, id, _dir) = app();
    add(&mut app, id, "board_size");
    let task = send(&mut app, id, RequirementsMsg::RunRequested);
    assert!(tab(&app, id).running, "the run is under way");
    crate::task_driver::TaskDriver::new()
        .settle(&mut app, task)
        .expect("the run settles");
    let state = tab(&app, id);
    assert!(!state.running);
    let report = state
        .last_run
        .as_ref()
        .expect("a run")
        .report
        .as_ref()
        .expect("a report");
    // The project lists no board, so the size cannot be met.
    assert_eq!(report.findings[0].rule, "req::board_size");
    assert!(report.passed.is_empty());
}

#[test]
fn the_commands_edit_through_the_same_path_and_refuse_with_the_reason() {
    let (mut app, id, _dir) = app();
    let value = answer(&mut app, "project_add_requirement", &["kind=bom_cost", "id=cost"]);
    assert_eq!(value["changed"], true);
    assert_eq!(value["requirements"][0]["id"], "cost");
    let value = answer(
        &mut app,
        "project_set_requirement",
        &["requirement=cost", "field=currency", "value=EUR"],
    );
    assert_eq!(value["requirements"][0]["currency"], "EUR");
    let value = answer(
        &mut app,
        "project_enable_requirement",
        &["requirement=cost", "on=false"],
    );
    assert_eq!(value["requirements"][0]["enabled"], false);

    let reason = refusal(
        &mut app,
        "project_set_requirement",
        &["requirement=cost", "field=quantity", "value=0"],
    );
    assert!(reason.contains("whole number"), "{reason}");
    let reason = refusal(&mut app, "project_remove_requirement", &["requirement=size"]);
    assert!(reason.contains("no requirement `size`"), "{reason}");
    let reason = refusal(&mut app, "project_add_requirement", &["kind=rail_budget"]);
    assert!(reason.contains("rail_budget"), "{reason}");

    let settings = answer(&mut app, "project_settings", &[]);
    assert_eq!(settings["requirements"][0]["id"], "cost");

    assert!(matches!(
        run(&mut app, "project_run_requirements", &[]),
        CommandOutcome::Ran
    ));
    assert!(tab(&app, id).last_run.is_some(), "the run landed in the tab");

    let value = answer(&mut app, "project_remove_requirement", &["requirement=cost"]);
    assert_eq!(value["requirements"], json!([]));
    // Each edit was one step: four undos reach the empty list again.
    for _ in 0..4 {
        assert!(undo(&mut app, id, true));
    }
    assert!(stored(&app).is_empty());
    assert!(!undo(&mut app, id, true));
}

#[test]
fn a_save_writes_the_edited_list_and_reads_back_the_same() {
    let (mut app, id, _dir) = app();
    add(&mut app, id, "parts_state");
    type_and_submit(&mut app, id, RequirementField::States, "released, in_review");
    add(&mut app, id, "net_class");
    let path = app.document_state.projects[0].path.clone();
    let task = app.save_project_at_path(&path).expect("the save starts");
    crate::task_driver::TaskDriver::new()
        .settle(&mut app, task)
        .expect("the save settles");
    assert!(!dirty(&app));
    let again = parse_project(&path).expect("the saved project");
    assert_eq!(again.requirements, app.document_state.projects[0].data.requirements);
    assert_eq!(
        again.requirements[0].raw()["states"],
        json!(["released", "in_review"])
    );
}

#[test]
fn the_section_renders_every_kind_an_unreadable_entry_drafts_and_a_run() {
    let (mut app, id, _dir) = app();
    for kind in signex_types::project::REQUIREMENT_KINDS {
        add(&mut app, id, kind);
    }
    app.document_state.projects[0]
        .data
        .requirements
        .push(serde_json::from_value(json!({"id": "rails", "kind": "rail_budget"})).unwrap());
    let _ = app.update(Message::Project(ProjectMsg::Tab(
        ProjectTabMsg::SectionSelected {
            project_id: id,
            section: crate::app::ProjectSection::Requirements,
        },
    )));
    let task = send(&mut app, id, RequirementsMsg::RunRequested);
    crate::task_driver::TaskDriver::new()
        .settle(&mut app, task)
        .expect("the run settles");
    type_and_submit(&mut app, id, RequirementField::Id, "");
    assert!(tab(&app, id).problem.is_some(), "an empty id is refused");
    let _ = app.view_project_tab(id);
}
