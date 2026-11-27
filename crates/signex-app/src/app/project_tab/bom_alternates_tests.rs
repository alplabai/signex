//! #2938: the BOM alternates commands, driven through the catalog as the
//! CLI and a running Signex run them. Each edit is one step of the
//! project's General history, taken back by the Project tab's Undo; it
//! dirties the `.snxprj`, writes nothing until Save, and a save writes the
//! list and reads back the same. A refused edit changes nothing and says
//! why.

use serde_json::{Value, json};
use signex_i18n::RenderTranslatable;
use signex_types::project::{parse_project, write_project};

use crate::app::Signex;
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::state::ProjectId;
use crate::keymap::AppCommandId;

const LIBRARY: &str = "019df45d-af5e-7a00-8c3b-dd3bf89b40ae";
const LINE_ROW: &str = "019df45d-af5e-7a00-8c3b-000000000001";
const ALT_A: &str = "019df45d-af5e-7a00-8c3b-00000000000a";
const ALT_B: &str = "019df45d-af5e-7a00-8c3b-00000000000b";

fn line() -> String {
    format!("line={LIBRARY}/{LINE_ROW}")
}

fn key(row: &str) -> String {
    format!("{LIBRARY}/{row}")
}

/// A project tab over a project file written to a folder of its own.
fn app() -> (Signex, ProjectId, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("a folder");
    let mut app = app_workspace(&dir.path().to_string_lossy(), &["a.snxsch"]);
    let project = &app.document_state.projects[0];
    write_project(&project.path, &project.data).expect("the project file");
    app.open_project_tab(0);
    let id = app.document_state.projects[0].id;
    (app, id, dir)
}

fn run(app: &mut Signex, id: &str, operands: &[&str]) -> CommandOutcome {
    let document = format!("document={}", app.document_state.projects[0].path.display());
    let mut all = vec![document.as_str()];
    all.extend_from_slice(operands);
    let command = AppCommandId::new(id).expect("a catalog id");
    let args = CommandArgs::parse_operands(&command, &all).expect("operands parse");
    let (outcome, task) = app.run_operands(&command, &args);
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
        CommandOutcome::Disabled { reason } => reason.render_en_us(),
        other => panic!("{id} was not refused: {other:?}"),
    }
}

fn alternate(row: &str) -> [String; 3] {
    [line(), format!("library={LIBRARY}"), format!("part={row}")]
}

fn operands(owned: &[String]) -> Vec<&str> {
    owned.iter().map(String::as_str).collect()
}

fn on_disk(app: &Signex) -> Vec<u8> {
    std::fs::read(&app.document_state.projects[0].path).expect("the project file")
}

fn dirty(app: &Signex) -> bool {
    let path = &app.document_state.projects[0].path;
    app.document_state.is_document_dirty(path)
}

#[test]
fn each_edit_is_one_undo_step_that_dirties_and_writes_nothing() {
    let (mut app, id, _dir) = app();
    let before = on_disk(&app);
    let value = answer(&mut app, "bom_add_alternate", &operands(&alternate(ALT_A)));
    assert_eq!(
        value,
        json!({"line": key(LINE_ROW), "alternates": [key(ALT_A)], "changed": true})
    );
    let value = answer(&mut app, "bom_add_alternate", &operands(&alternate(ALT_B)));
    assert_eq!(value["alternates"], json!([key(ALT_A), key(ALT_B)]));
    let mut order = alternate(ALT_B).to_vec();
    order.push("position=1".into());
    let value = answer(&mut app, "bom_set_alternate_order", &operands(&order));
    assert_eq!(value["alternates"], json!([key(ALT_B), key(ALT_A)]));
    let value = answer(
        &mut app,
        "bom_remove_alternate",
        &operands(&alternate(ALT_A)),
    );
    assert_eq!(value["alternates"], json!([key(ALT_B)]));
    assert!(dirty(&app));
    assert_eq!(on_disk(&app), before, "nothing is written until Save");

    // Four edits, four steps of the Project tab's Undo.
    for _ in 0..4 {
        assert!(app.step_project_tab_history(id, true));
    }
    assert!(
        app.document_state.projects[0]
            .data
            .bom_alternates
            .is_empty()
    );
    assert!(!dirty(&app), "undo returned to the file's content");
    assert!(!app.step_project_tab_history(id, true));
    assert!(app.step_project_tab_history(id, false));
    let data = &app.document_state.projects[0].data;
    assert_eq!(data.bom_alternates_of(&key(LINE_ROW)), [key(ALT_A)]);
}

#[test]
fn a_refused_edit_changes_nothing_and_says_why() {
    let (mut app, _id, _dir) = app();
    answer(&mut app, "bom_add_alternate", &operands(&alternate(ALT_A)));
    let before = app.document_state.projects[0].data.bom_alternates.clone();
    let reason = refusal(&mut app, "bom_add_alternate", &operands(&alternate(ALT_A)));
    assert!(reason.contains("already an alternate"), "{reason}");
    let reason = refusal(
        &mut app,
        "bom_add_alternate",
        &operands(&alternate(LINE_ROW)),
    );
    assert!(reason.contains("the line's own part"), "{reason}");
    let reason = refusal(
        &mut app,
        "bom_add_alternate",
        &[
            "line=R1",
            &format!("library={LIBRARY}"),
            &format!("part={ALT_B}"),
        ],
    );
    assert!(reason.contains("no BOM line key"), "{reason}");
    let reason = refusal(
        &mut app,
        "bom_add_alternate",
        &[&line(), "library=Passives", "part=R-10K"],
    );
    assert!(reason.contains("is not open"), "{reason}");
    let reason = refusal(
        &mut app,
        "bom_remove_alternate",
        &operands(&alternate(ALT_B)),
    );
    assert!(reason.contains("no alternate of line"), "{reason}");
    let mut order = alternate(ALT_A).to_vec();
    order.push("position=3".into());
    let reason = refusal(&mut app, "bom_set_alternate_order", &operands(&order));
    assert!(reason.contains("`position` is 1 to 1"), "{reason}");
    assert_eq!(app.document_state.projects[0].data.bom_alternates, before);
}

#[test]
fn a_save_writes_the_alternates_and_reads_back_the_same() {
    let (mut app, _id, _dir) = app();
    answer(&mut app, "bom_add_alternate", &operands(&alternate(ALT_A)));
    answer(&mut app, "bom_add_alternate", &operands(&alternate(ALT_B)));
    let path = app.document_state.projects[0].path.clone();
    let task = app.save_project_at_path(&path).expect("the save starts");
    crate::task_driver::TaskDriver::new()
        .settle(&mut app, task)
        .expect("the save settles");
    assert!(!dirty(&app));
    let again = parse_project(&path).expect("the saved project");
    assert_eq!(
        again.bom_alternates,
        app.document_state.projects[0].data.bom_alternates
    );
    assert_eq!(
        again.bom_alternates_of(&key(LINE_ROW)),
        [key(ALT_A), key(ALT_B)]
    );
}
