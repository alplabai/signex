//! #1291 (mockup row 206): the Design Checklist tab, driven through
//! `Signex::update` (the GUI path) and the catalog (the headless path).
//! Project ▸ Design Checklist opens one tab per project; every edit dirties
//! the `.snxprj`, writes nothing until Save and is one undo step; a refused
//! one changes nothing and says why; a linked check that passes offers Mark
//! done and the owner ticks it; Run checks lands its report in the tab; a
//! save writes the checklist and reads back the same.

use serde_json::{Value, json};
use signex_i18n::RenderTranslatable;
use signex_types::check::{CheckReport, Finding, Severity};
use signex_types::project::{
    ChecklistField, ChecklistLink, ChecklistTemplate, ItemState, LinkResult, RequirementEntry,
    TITLE_BLOCK_REQUIRED, TemplateRef, parse_project, write_project,
};

use super::ChecklistMsg;
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::state::ProjectId;
use crate::app::{Message, ProjectMsg, ProjectTabMsg, Signex, TabKind};
use crate::keymap::AppCommandId;

/// A project over a project file written to a folder of its own, its
/// Project tab active; the folder lives as long as the returned guard.
pub(super) fn app() -> (Signex, ProjectId, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("a folder");
    let mut app = app_workspace(&dir.path().to_string_lossy(), &["a.snxsch"]);
    let project = &app.document_state.projects[0];
    write_project(&project.path, &project.data).expect("the project file");
    app.open_project_tab(0);
    let id = app.document_state.projects[0].id;
    (app, id, dir)
}

pub(super) fn send(app: &mut Signex, id: ProjectId, msg: ChecklistMsg) -> iced::Task<Message> {
    app.update(Message::Project(ProjectMsg::Tab(
        ProjectTabMsg::Checklist {
            project_id: id,
            msg,
        },
    )))
}

pub(super) fn settle(app: &mut Signex, task: iced::Task<Message>) {
    crate::task_driver::TaskDriver::new()
        .settle(app, task)
        .expect("the task settles");
}

pub(super) fn items(app: &Signex) -> Vec<Value> {
    app.document_state.projects[0]
        .data
        .design_checklist_value()
        .and_then(|value| value.get("items"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

fn on_disk(app: &Signex) -> Vec<u8> {
    std::fs::read(&app.document_state.projects[0].path).expect("the project file")
}

pub(super) fn dirty(app: &Signex) -> bool {
    let path = &app.document_state.projects[0].path;
    app.document_state.is_document_dirty(path)
}

pub(super) fn undo(app: &mut Signex, id: ProjectId, back: bool) -> bool {
    app.step_project_tab_history(id, back)
}

pub(super) fn tab(app: &Signex, id: ProjectId) -> &super::ChecklistTabState {
    &app.document_view.project_tabs[&id].checklist
}

pub(super) fn start(app: &mut Signex, id: ProjectId, template: ChecklistTemplate) {
    let picked = ChecklistMsg::TemplatePicked(TemplateRef::Builtin(template));
    let _ = send(app, id, picked);
    let _ = send(app, id, ChecklistMsg::StartPressed);
}

/// Run catalog command `id` on the project with `operands`, as a headless
/// run does: a command that waits for its file work is answered once it ran.
pub(super) fn run(app: &mut Signex, id: &str, operands: &[&str]) -> CommandOutcome {
    let document = format!("document={}", app.document_state.projects[0].path.display());
    let mut all = vec![document.as_str()];
    all.extend_from_slice(operands);
    let command = AppCommandId::new(id).expect("a catalog id");
    let args = CommandArgs::parse_operands(&command, &all).expect("operands parse");
    let (outcome, task) = match app.run_query(&command, &args) {
        Some(ran) => ran,
        None => app.run_operands_settled(&command, &args),
    };
    settle(app, task);
    outcome
}

pub(super) fn answer(app: &mut Signex, id: &str, operands: &[&str]) -> Value {
    match run(app, id, operands) {
        CommandOutcome::Answered(value) => value,
        other => panic!("{id} did not answer: {other:?}"),
    }
}

pub(super) fn refusal(app: &mut Signex, id: &str, operands: &[&str]) -> String {
    match run(app, id, operands) {
        CommandOutcome::Disabled { reason } => reason.render_en_us(),
        other => panic!("{id} was not refused: {other:?}"),
    }
}

#[test]
fn the_menu_opens_one_tab_per_project_and_focuses_it_again() {
    let (mut app, id, _dir) = app();
    let open = || {
        Message::Project(ProjectMsg::Tab(ProjectTabMsg::ChecklistOpenRequested {
            project: None,
        }))
    };
    let _ = app.update(open());
    let docs = &app.document_state;
    let active = &docs.tabs[docs.active_tab];
    assert!(matches!(active.kind, TabKind::DesignChecklist(p) if p == id));
    assert!(
        active.title.contains("Design Checklist"),
        "{}",
        active.title
    );
    let count = docs.tabs.len();
    app.open_project_tab(0);
    let _ = app.update(open());
    let docs = &app.document_state;
    assert_eq!(docs.tabs.len(), count, "the open tab is focused again");
    assert!(matches!(
        docs.tabs[docs.active_tab].kind,
        TabKind::DesignChecklist(p) if p == id
    ));
    let _ = app.view_design_checklist_tab(id);
}

#[test]
fn a_start_is_one_undo_step_that_dirties_and_writes_nothing() {
    let (mut app, id, _dir) = app();
    let before = on_disk(&app);
    start(&mut app, id, ChecklistTemplate::PrototypeBuild);
    let stored = items(&app);
    assert_eq!(stored.len(), 6);
    assert_eq!(stored[0]["title"], "ERC passes with no errors");
    assert_eq!(stored[0]["link"], json!({"kind": "erc"}));
    assert_eq!(stored[0]["state"], "open");
    assert!(dirty(&app));
    assert_eq!(on_disk(&app), before, "nothing is written until Save");
    assert!(undo(&mut app, id, true));
    assert!(items(&app).is_empty());
    assert!(!dirty(&app), "undo returned to the file's content");
    assert!(undo(&mut app, id, false));
    assert_eq!(items(&app).len(), 6);
}

#[test]
fn an_added_item_is_selected_and_its_title_commits_on_enter() {
    let (mut app, id, _dir) = app();
    start(&mut app, id, ChecklistTemplate::Blank);
    let _ = send(&mut app, id, ChecklistMsg::AddPressed);
    assert_eq!(items(&app)[0]["title"], "New item");
    assert_eq!(tab(&app, id).selected.as_deref(), Some("1"));
    let typed = ChecklistMsg::FieldEdited {
        item: "1".into(),
        field: ChecklistField::Title,
        text: "Check the via sizes".into(),
    };
    let _ = send(&mut app, id, typed);
    assert_eq!(
        items(&app)[0]["title"],
        "New item",
        "typing commits nothing"
    );
    let submitted = ChecklistMsg::FieldSubmitted {
        item: "1".into(),
        field: ChecklistField::Title,
    };
    let _ = send(&mut app, id, submitted);
    assert_eq!(items(&app)[0]["title"], "Check the via sizes");
    assert!(tab(&app, id).drafts.is_empty());
    let _ = app.view_design_checklist_tab(id);
}

#[test]
fn waived_asks_for_a_reason_and_will_not_save_without_one() {
    let (mut app, id, _dir) = app();
    start(&mut app, id, ChecklistTemplate::PrototypeBuild);
    let _ = send(&mut app, id, ChecklistMsg::ItemSelected("4".into()));
    let _ = send(
        &mut app,
        id,
        ChecklistMsg::WaiveStarted { item: "4".into() },
    );
    let _ = app.view_design_checklist_tab(id);
    let _ = send(&mut app, id, ChecklistMsg::WaiveSubmitted);
    assert_eq!(items(&app)[3]["state"], "open", "refused");
    assert_eq!(
        tab(&app, id).problem,
        Some(signex_types::project::ChecklistProblem::NeedsReason)
    );
    assert!(tab(&app, id).waiving.is_some(), "the reason box stays");
    let _ = send(
        &mut app,
        id,
        ChecklistMsg::ReasonEdited("No enclosure".into()),
    );
    let _ = send(&mut app, id, ChecklistMsg::WaiveSubmitted);
    assert_eq!(items(&app)[3]["state"], "waived");
    assert_eq!(items(&app)[3]["reason"], "No enclosure");
    assert!(tab(&app, id).waiving.is_none());
    let _ = app.view_design_checklist_tab(id);
}

#[test]
fn a_passing_linked_check_offers_mark_done_and_the_owner_ticks_it() {
    let (mut app, id, _dir) = app();
    start(&mut app, id, ChecklistTemplate::PrototypeBuild);
    // Item 2 links the title block.
    let read = answer(&mut app, "project_checklist", &[]);
    assert_eq!(read["items"][1]["result"], "fails");
    assert_eq!(read["items"][1]["check_passes"], false);
    for key in TITLE_BLOCK_REQUIRED {
        app.document_state.projects[0]
            .data
            .parameters
            .insert(key.into(), "set".into());
    }
    let read = answer(&mut app, "project_checklist", &[]);
    assert_eq!(read["items"][1]["result"], "passes");
    assert_eq!(read["items"][1]["check_passes"], true);
    assert_eq!(
        read["items"][1]["state"], "open",
        "the item does not tick itself"
    );
    let _ = app.view_design_checklist_tab(id);
    let done = ChecklistMsg::StatePicked {
        item: "2".into(),
        state: ItemState::Done,
    };
    let _ = send(&mut app, id, done);
    let read = answer(&mut app, "project_checklist", &[]);
    assert_eq!(read["items"][1]["state"], "done");
    assert_eq!(read["items"][1]["check_passes"], false);
    assert_eq!(read["stages"]["schematic"]["done"], 1);
}

#[test]
fn run_checks_lands_its_report_in_the_tab_off_the_ui_thread() {
    let (mut app, id, _dir) = app();
    start(&mut app, id, ChecklistTemplate::PrototypeBuild);
    let task = send(&mut app, id, ChecklistMsg::RunPressed);
    assert!(tab(&app, id).running, "the run is under way");
    settle(&mut app, task);
    let state = tab(&app, id);
    assert!(!state.running);
    let report = state.report().expect("a finished run");
    // The project lists no board, so its DRC check cannot pass.
    assert!(
        report
            .findings
            .iter()
            .any(|finding| finding.rule == "req::checklist::drc"),
        "{:?}",
        report.findings
    );
    let read = answer(&mut app, "project_checklist", &[]);
    assert_eq!(read["items"][2]["result"], "fails");
    assert!(read["last_run"].is_string());
    let _ = app.view_design_checklist_tab(id);
}

#[test]
fn the_link_rule_is_the_one_the_requirement_checks_name() {
    let (app, _id, _dir) = app();
    let entry = RequirementEntry::read(json!({"id": "REQ-1", "kind": "erc_clean"}));
    let rule = signex_req::entry_rule(&entry, 0);
    let finding = Finding {
        rule: rule.clone(),
        severity: Severity::Error,
        message: String::new(),
        evidence: json!({}),
        items: Vec::new(),
        sheet: String::new(),
        key: rule,
    };
    let report = CheckReport::new(
        "requirements",
        vec![finding],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    let link = ChecklistLink::Requirement { id: "REQ-1".into() };
    let data = &app.document_state.projects[0].data;
    assert_eq!(link.result(data, Some(&report)), LinkResult::Fails(1));
}

#[test]
fn the_commands_edit_through_the_same_path_and_refuse_with_the_reason() {
    let (mut app, id, _dir) = app();
    let read = answer(&mut app, "project_checklist", &[]);
    assert_eq!(read["started"], false);
    let value = answer(
        &mut app,
        "project_start_checklist",
        &["template=prototype_build"],
    );
    assert_eq!(value["changed"], true);
    assert_eq!(value["items"].as_array().map(Vec::len), Some(6));
    let value = answer(
        &mut app,
        "project_add_checklist_item",
        &["stage=layout", "text=Panel rails"],
    );
    assert_eq!(value["item"], "7");
    let value = answer(
        &mut app,
        "project_set_checklist_item",
        &["item=7", "field=owner", "value=jb"],
    );
    assert_eq!(value["items"][6]["owner"], "jb");
    let reason = refusal(
        &mut app,
        "project_set_checklist_state",
        &["item=7", "state=waived"],
    );
    assert!(reason.contains("reason"), "{reason}");
    let value = answer(
        &mut app,
        "project_set_checklist_state",
        &["item=7", "state=waived", "reason=Not panelised"],
    );
    assert_eq!(value["items"][6]["state"], "waived");
    assert_eq!(value["counts"]["waived"], 1);
    let reason = refusal(
        &mut app,
        "project_set_checklist_item",
        &["item=7", "field=link", "value=bom"],
    );
    assert!(reason.contains("bom"), "{reason}");
    let reason = refusal(&mut app, "project_start_checklist", &["template=blank"]);
    assert!(reason.contains("already"), "{reason}");
    let reason = refusal(&mut app, "project_remove_checklist_item", &["item=9"]);
    assert!(reason.contains("no item `9`"), "{reason}");
    let value = answer(&mut app, "project_remove_checklist_item", &["item=7"]);
    assert_eq!(value["items"].as_array().map(Vec::len), Some(6));

    assert!(matches!(
        run(&mut app, "project_run_checklist", &[]),
        CommandOutcome::Ran
    ));
    assert!(
        tab(&app, id).last_run.is_some(),
        "the run landed in the tab"
    );

    // Each edit was one step: five undos reach the project without one.
    for _ in 0..5 {
        assert!(undo(&mut app, id, true));
    }
    assert!(items(&app).is_empty());
    assert!(!undo(&mut app, id, true));
}

#[test]
fn a_save_writes_the_checklist_and_reads_back_the_same() {
    let (mut app, id, _dir) = app();
    start(&mut app, id, ChecklistTemplate::ReleaseReview);
    let done = ChecklistMsg::StatePicked {
        item: "1".into(),
        state: ItemState::Done,
    };
    let _ = send(&mut app, id, done);
    let path = app.document_state.projects[0].path.clone();
    let task = app.save_project_at_path(&path).expect("the save starts");
    settle(&mut app, task);
    assert!(!dirty(&app));
    let again = parse_project(&path).expect("the saved project");
    assert_eq!(
        again.design_checklist(),
        app.document_state.projects[0].data.design_checklist()
    );
    let checklist = again.design_checklist().unwrap().expect("a checklist");
    assert_eq!(checklist.items.len(), 12);
    assert_eq!(checklist.items[0].state, ItemState::Done);
}
