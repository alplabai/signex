//! #1291 (mockup row 206), past the first edits: the filter chips with their
//! counts and Mine, an item's history, Save as template, Start and Reset
//! from a saved template, Export CSV and Import from another project. Each
//! is driven through `Signex::update` with the view laid out and clicked
//! (the GUI path) and through the catalog commands (the headless path).
//! Only `the_users_templates_are_saved_listed_started_and_reset` writes the
//! template library, so tests sharing the config folder do not race on it.

use iced::Task;
use serde_json::json;
use signex_types::project::{
    ChecklistField, ChecklistTemplate, ItemState, TemplateRef, write_project,
};

use super::tests::{answer, app, dirty, items, refusal, send, settle, start, tab, undo};
use super::{ChecklistFilter, ChecklistMsg, ChecklistNotice, SavedTemplates};
use crate::app::state::ProjectId;
use crate::app::{Message, Signex};
use crate::io_ports::{HeadlessAnswers, IoPorts};
use crate::layout_probe::{lay_out_texts, messages_for_events};

/// Wide and tall enough for the bar, the table and the item pane at once.
const WINDOW: iced::Size = iced::Size::new(2400.0, 1600.0);

/// `text` without Fluent's isolation marks around its arguments.
fn plain(text: &str) -> String {
    text.replace(['\u{2068}', '\u{2069}'], "")
}

/// Is a text reading `label` in the laid-out tab?
pub(super) fn shows(app: &Signex, id: ProjectId, label: &str) -> bool {
    lay_out_texts(app.view_design_checklist_tab(id), WINDOW)
        .iter()
        .any(|text| plain(&text.text) == label)
}

/// Click the text `label` of the laid-out tab and run what it publishes.
pub(super) fn click(app: &mut Signex, id: ProjectId, label: &str) -> Task<Message> {
    let texts = lay_out_texts(app.view_design_checklist_tab(id), WINDOW);
    let at = texts
        .iter()
        .find(|text| plain(&text.text) == label)
        .unwrap_or_else(|| panic!("no {label:?} in {texts:#?}"))
        .bounds
        .center();
    let press = [
        iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)),
        iced::Event::Mouse(iced::mouse::Event::ButtonReleased(
            iced::mouse::Button::Left,
        )),
    ];
    let messages = messages_for_events(app.view_design_checklist_tab(id), WINDOW, at, &press);
    assert!(!messages.is_empty(), "{label:?} published nothing");
    let tasks: Vec<Task<Message>> = messages.into_iter().map(|m| app.update(m)).collect();
    Task::batch(tasks)
}

/// The app with the user known as `j.berg`, so nothing asks git.
pub(super) fn app_as_jberg() -> (Signex, ProjectId, tempfile::TempDir) {
    let (mut app, id, dir) = app();
    app.ui_state.validation_panels.identity = crate::panels::UserIdentity::Known("j.berg".into());
    (app, id, dir)
}

#[test]
fn the_chips_count_their_items_and_mine_shows_the_users_own() {
    let (mut app, id, _dir) = app_as_jberg();
    start(&mut app, id, ChecklistTemplate::PrototypeBuild);
    let owner = ChecklistMsg::FieldEdited {
        item: "2".into(),
        field: ChecklistField::Owner,
        text: "J.Berg".into(),
    };
    let _ = send(&mut app, id, owner);
    let _ = send(
        &mut app,
        id,
        ChecklistMsg::FieldSubmitted {
            item: "2".into(),
            field: ChecklistField::Owner,
        },
    );
    let done = ChecklistMsg::StatePicked {
        item: "1".into(),
        state: ItemState::Done,
    };
    let _ = send(&mut app, id, done);
    for label in ["All 6", "Open 5", "Done 1", "Waived 0", "Mine 1"] {
        assert!(shows(&app, id, label), "{label}");
    }
    let _ = click(&mut app, id, "Mine 1");
    assert_eq!(tab(&app, id).filter, ChecklistFilter::Mine);
    assert!(shows(&app, id, "Title block filled in"), "item 2 is mine");
    assert!(
        !shows(&app, id, "ERC passes with no errors"),
        "item 1 is not"
    );

    let read = answer(&mut app, "project_checklist", &["filter=mine"]);
    assert_eq!(read["user"], "j.berg");
    assert_eq!(
        read["chips"],
        json!({"all": 6, "open": 5, "done": 1, "waived": 0, "mine": 1})
    );
    assert_eq!(read["shown"], json!(["2"]));
    assert_eq!(read["items"][1]["mine"], true);
    let read = answer(
        &mut app,
        "project_checklist",
        &["filter=mine", "owner=m.demir"],
    );
    assert_eq!(read["chips"]["mine"], 0);
    assert_eq!(read["shown"], json!([]));
    let reason = refusal(&mut app, "project_checklist", &["filter=late"]);
    assert!(reason.contains("late"), "{reason}");
}

#[test]
fn every_edit_writes_a_history_line_with_who_and_when() {
    let (mut app, id, _dir) = app_as_jberg();
    start(&mut app, id, ChecklistTemplate::PrototypeBuild);
    let waive = ChecklistMsg::WaiveStarted { item: "3".into() };
    let _ = send(&mut app, id, waive);
    let _ = send(
        &mut app,
        id,
        ChecklistMsg::ReasonEdited("No board yet".into()),
    );
    let _ = send(&mut app, id, ChecklistMsg::WaiveSubmitted);
    let history = items(&app)[2]["history"].clone();
    assert_eq!(history[0]["event"], "created");
    assert_eq!(history[0]["value"], "prototype_build");
    assert_eq!(history[0]["by"], "j.berg");
    assert_eq!(history[1]["event"], "waived");
    assert_eq!(history[1]["value"], "No board yet");
    assert!(history[1]["on"].is_string());

    let _ = send(&mut app, id, ChecklistMsg::ItemSelected("3".into()));
    assert!(shows(&app, id, "History"));
    assert!(shows(&app, id, "Created from the template Prototype build"));
    assert!(shows(&app, id, "Waived: No board yet"));
    // An undo takes the line back with the edit.
    assert!(undo(&mut app, id, true));
    assert_eq!(items(&app)[2]["history"].as_array().map(Vec::len), Some(1));
}

#[test]
fn the_users_templates_are_saved_listed_started_and_reset() {
    let name = format!("Lab review {}", uuid::Uuid::now_v7());
    let (mut app, id, _dir) = app_as_jberg();
    start(&mut app, id, ChecklistTemplate::PrototypeBuild);
    let _ = send(&mut app, id, ChecklistMsg::AddPressed);
    // More ▸ Save as template…, named, saved off the UI thread.
    let _ = click(&mut app, id, "More");
    assert!(tab(&app, id).more_open);
    let _ = click(&mut app, id, "Save as template…");
    assert_eq!(tab(&app, id).template_name.as_deref(), Some("ProjectA"));
    let _ = send(&mut app, id, ChecklistMsg::TemplateNameEdited(name.clone()));
    let task = send(&mut app, id, ChecklistMsg::SaveTemplateSubmitted);
    settle(&mut app, task);
    let state = tab(&app, id);
    assert_eq!(
        state.notice,
        Some(ChecklistNotice::TemplateSaved(name.clone()))
    );
    assert!(
        state
            .saved
            .list()
            .iter()
            .any(|t| t.name == name && t.items.len() == 7)
    );
    assert!(state.template_name.is_none());

    // The headless list names it among the user's templates.
    let listed = answer(&mut app, "project_checklist_templates", &[]);
    let key = format!("user:{name}");
    let saved = listed["saved"].as_array().cloned().unwrap_or_default();
    assert!(
        saved
            .iter()
            .any(|t| t["template"] == key.as_str() && t["items"] == 7),
        "{listed}"
    );
    assert_eq!(listed["builtin"][0]["template"], "release_review");

    // Another project starts from it in the tab: Your templates lists it.
    let (mut other, other_id, _other_dir) = app_as_jberg();
    let task = other.open_design_checklist_tab(other_id);
    settle(&mut other, task);
    assert!(matches!(
        tab(&other, other_id).saved,
        SavedTemplates::Read(_)
    ));
    assert!(shows(&other, other_id, "Your templates"));
    let pick = ChecklistMsg::TemplatePicked(TemplateRef::Saved(name.clone()));
    let _ = send(&mut other, other_id, pick);
    let _ = send(&mut other, other_id, ChecklistMsg::StartPressed);
    assert_eq!(items(&other).len(), 7);
    assert_eq!(items(&other)[6]["title"], "New item");

    // Reset from template puts its items back, as one undo step.
    let _ = send(
        &mut other,
        other_id,
        ChecklistMsg::Removed { item: "7".into() },
    );
    let _ = click(&mut other, other_id, "More");
    let _ = click(&mut other, other_id, "Reset from template");
    assert_eq!(items(&other).len(), 7);
    assert!(undo(&mut other, other_id, true));
    assert_eq!(items(&other).len(), 6);

    // Headless: start a third project from it, reset it, save one more.
    let (mut third, _third_id, _third_dir) = app_as_jberg();
    let template = format!("template={key}");
    let started = answer(&mut third, "project_start_checklist", &[template.as_str()]);
    assert_eq!(started["template"], key.as_str());
    assert_eq!(started["items"].as_array().map(Vec::len), Some(7));
    let _ = answer(&mut third, "project_remove_checklist_item", &["item=1"]);
    let reset = answer(&mut third, "project_reset_checklist", &[]);
    assert_eq!(reset["items"].as_array().map(Vec::len), Some(7));
    assert_eq!(reset["changed"], true);
    let again = format!("name={name} copy");
    let saved = answer(
        &mut third,
        "project_save_checklist_template",
        &[again.as_str()],
    );
    assert_eq!(saved["template"], format!("user:{name} copy"));
    let reason = refusal(
        &mut third,
        "project_start_checklist",
        &["template=user:Nobody"],
    );
    assert!(reason.contains("Nobody"), "{reason}");
}

#[test]
fn a_reset_and_a_saved_start_are_refused_with_the_reason() {
    let (mut app, id, _dir) = app_as_jberg();
    let reason = refusal(&mut app, "project_reset_checklist", &[]);
    assert!(reason.contains("no design checklist"), "{reason}");
    let reason = refusal(&mut app, "project_save_checklist_template", &["name=X"]);
    assert!(reason.contains("no design checklist"), "{reason}");
    let reason = refusal(
        &mut app,
        "project_start_checklist",
        &["template=user:Not saved anywhere 0"],
    );
    assert!(reason.contains("Not saved anywhere 0"), "{reason}");
    let _ = send(&mut app, id, ChecklistMsg::AddPressed);
    let reason = refusal(&mut app, "project_reset_checklist", &[]);
    assert!(reason.contains("not started from a template"), "{reason}");
    // From the tab: Save as template with the name cleared says why.
    let _ = send(&mut app, id, ChecklistMsg::SaveTemplateStarted);
    let _ = send(&mut app, id, ChecklistMsg::TemplateNameEdited("  ".into()));
    let task = send(&mut app, id, ChecklistMsg::SaveTemplateSubmitted);
    settle(&mut app, task);
    assert_eq!(
        tab(&app, id).problem,
        Some(signex_types::project::ChecklistProblem::EmptyName)
    );
    assert!(shows(&app, id, "A template needs a name."));
}

#[test]
fn export_csv_writes_the_items_from_the_menu_and_the_command() {
    let (mut app, id, dir) = app_as_jberg();
    start(&mut app, id, ChecklistTemplate::PrototypeBuild);
    let answers = HeadlessAnswers::default();
    app.ui_state.io = IoPorts::headless(answers.clone());
    let menu_file = dir.path().join("from-menu.csv");
    answers.replace_all([("path".to_string(), menu_file.display().to_string())]);
    let _ = click(&mut app, id, "More");
    let task = click(&mut app, id, "Export CSV…");
    settle(&mut app, task);
    let written = std::fs::read_to_string(&menu_file).expect("the CSV");
    let lines: Vec<&str> = written.lines().collect();
    assert_eq!(
        lines[0],
        "id,stage,state,title,owner,link,reason,notes,updated"
    );
    assert_eq!(lines.len(), 7);
    assert!(lines[1].starts_with("1,schematic,open,ERC passes with no errors,,erc,"));

    let file = dir.path().join("from-command.csv");
    let path = format!("path={}", file.display());
    let value = answer(&mut app, "project_export_checklist_csv", &[path.as_str()]);
    assert_eq!(value["rows"], 6);
    assert_eq!(std::fs::read_to_string(&file).expect("the CSV"), written);
}

#[test]
fn import_from_another_project_starts_from_its_items() {
    // The project to import from: a checklist with one waived item, saved.
    let (mut source, source_id, source_dir) = app_as_jberg();
    start(&mut source, source_id, ChecklistTemplate::PrototypeBuild);
    let _ = send(
        &mut source,
        source_id,
        ChecklistMsg::WaiveStarted { item: "1".into() },
    );
    let _ = send(
        &mut source,
        source_id,
        ChecklistMsg::ReasonEdited("Debug".into()),
    );
    let _ = send(&mut source, source_id, ChecklistMsg::WaiveSubmitted);
    let project = &source.document_state.projects[0];
    write_project(&project.path, &project.data).expect("the source project");
    let from = project.path.clone();

    // The menu: the dialog picks it, the items come in open.
    let (mut app, id, _dir) = app_as_jberg();
    let answers = HeadlessAnswers::default();
    app.ui_state.io = IoPorts::headless(answers.clone());
    answers.replace_all([("path".to_string(), from.display().to_string())]);
    let task = click(&mut app, id, "Import from another project…");
    settle(&mut app, task);
    let imported = items(&app);
    assert_eq!(imported.len(), 6);
    assert_eq!(imported[0]["state"], "open", "states are not imported");
    assert_eq!(imported[0]["history"][0]["event"], "imported");
    assert_eq!(imported[0]["history"][0]["value"], "ProjectA");
    assert!(dirty(&app));
    assert!(undo(&mut app, id, true));
    assert!(items(&app).is_empty(), "one undo step");

    // The command, and its refusals.
    let path = format!("path={}", from.display());
    let value = answer(&mut app, "project_import_checklist", &[path.as_str()]);
    assert_eq!(value["items"].as_array().map(Vec::len), Some(6));
    let reason = refusal(&mut app, "project_import_checklist", &[path.as_str()]);
    assert!(reason.contains("already"), "{reason}");
    let (mut empty, _empty_id, _empty_dir) = app_as_jberg();
    let bare = format!("path={}", empty.document_state.projects[0].path.display());
    let (mut other, _other_id, _other_dir) = app_as_jberg();
    let reason = refusal(&mut other, "project_import_checklist", &[bare.as_str()]);
    assert!(reason.contains("has no design checklist"), "{reason}");
    let missing = format!("path={}", source_dir.path().join("gone.snxprj").display());
    let reason = refusal(&mut empty, "project_import_checklist", &[missing.as_str()]);
    assert!(reason.contains("could not be read"), "{reason}");
}
