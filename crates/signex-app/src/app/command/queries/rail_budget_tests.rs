//! #2943: `project_set_rail` sets a power rail's voltage and tolerance as
//! one undo step of the project's history, and `query_rail_budget` answers
//! every rail of the project off the UI thread: its voltage, source, loads,
//! total and headroom, from the parts' rail parameters on a closed sheet.

use std::path::PathBuf;

use serde_json::{Value, json};
use signex_i18n::RenderTranslatable;
use signex_types::format::SnxSchematic;
use signex_types::schematic::Symbol;
use uuid::Uuid;

use crate::app::Signex;
use crate::app::command::project_name_tests::{child, project_app};
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::keymap::AppCommandId;

fn project_path(app: &Signex) -> PathBuf {
    app.document_state.projects[0].path.clone()
}

/// A part `reference` with the rail parameters `fields`.
fn part(reference: &str, fields: &[(&str, &str)]) -> Symbol {
    Symbol {
        uuid: Uuid::now_v7(),
        lib_id: "Device:U".into(),
        reference: reference.into(),
        value: reference.into(),
        unit: 1,
        fields: fields
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect(),
        ..Symbol::default()
    }
}

/// The closed child sheet with two rails' sources and loads.
fn write_child_with_rails(dir: &tempfile::TempDir) {
    let mut sheet = child();
    sheet.symbols.push(part(
        "U1",
        &[("Rail Supply +3V3", "1A"), ("Rail Load +5V", "0.2")],
    ));
    sheet
        .symbols
        .push(part("U2", &[("Rail Load +3V3", "250mA")]));
    sheet.symbols.push(part("U3", &[("Rail Supply +5V", "2")]));
    let text = SnxSchematic::new(sheet).write_string().expect("encode");
    std::fs::write(dir.path().join("child.snxsch"), text).expect("write sheet");
}

fn run(app: &mut Signex, id: &str, operands: &[&str]) -> CommandOutcome {
    let command = AppCommandId::new(id).expect("a catalog id");
    let document = format!("document={}", project_path(app).display());
    let mut all = vec![document.as_str()];
    all.extend_from_slice(operands);
    let args = CommandArgs::parse_operands(&command, &all).expect("operands parse");
    let ran = match app.run_query(&command, &args) {
        Some(ran) => ran,
        None => app.run_operands(&command, &args),
    };
    match ran {
        (CommandOutcome::Deferred { ticket }, task) => app.settle_deferred(ticket, task).0,
        (outcome, task) => {
            crate::task_driver::TaskDriver::new()
                .settle(app, task)
                .expect("the task settles");
            outcome
        }
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
fn a_rail_edit_is_one_undo_step_that_dirties_and_a_refused_one_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(&dir);
    let path = project_path(&app);
    let on_disk = std::fs::read(&path).ok();
    let set = answer(
        &mut app,
        "project_set_rail",
        &["net=+3V3", "voltage=3.3", "tolerance_percent=5"],
    );
    assert_eq!(
        set,
        json!({"rails": {"+3V3": {"voltage": 3.3, "tolerance_percent": 5}},
               "unreadable": null, "changed": true})
    );
    assert!(app.document_state.is_document_dirty(&path));
    assert_eq!(std::fs::read(&path).ok(), on_disk, "nothing is written");
    let reason = refusal(&mut app, "project_set_rail", &["net=+3V3", "voltage=0"]);
    assert!(reason.contains("not a voltage"), "{reason}");
    let reason = refusal(&mut app, "project_set_rail", &["net=+1V8", "remove=true"]);
    assert!(reason.contains("no rail +1V8"), "{reason}");

    let id = app.document_state.projects[0].id;
    assert!(app.step_project_tab_history(id, true), "one step to undo");
    assert_eq!(app.document_state.projects[0].data.rails_value(), None);
    assert!(
        !app.step_project_tab_history(id, true),
        "the refusals made none"
    );
    assert!(app.step_project_tab_history(id, false));
    let rails = app.document_state.projects[0].data.rails().unwrap();
    assert_eq!(rails["+3V3"].tolerance_percent, Some(5.0));
}

#[test]
fn the_rail_budget_answers_every_rail_from_the_closed_sheets_parts() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(&dir);
    write_child_with_rails(&dir);
    let _ = answer(&mut app, "project_set_rail", &["net=+3V3", "voltage=3.3"]);
    let _ = answer(&mut app, "project_set_rail", &["net=+5V", "voltage=5"]);
    let command = AppCommandId::new("query_rail_budget").expect("a catalog id");
    let document = format!("document={}", project_path(&app).display());
    let args = CommandArgs::parse_operands(&command, &[document.as_str()]).expect("operands");
    let (outcome, task) = app.run_query(&command, &args).expect("a query");
    let CommandOutcome::Deferred { ticket } = outcome else {
        panic!("the parts are read off the UI thread: {outcome:?}");
    };
    let CommandOutcome::Answered(value) = app.settle_deferred(ticket, task).0 else {
        panic!("not answered");
    };
    assert_eq!(value["count"], 2, "{value}");
    assert_eq!(value["project"], project_path(&app).display().to_string());
    let three = &value["rails"][0];
    assert_eq!(three["net"], "+3V3");
    assert_eq!(three["voltage"], 3.3);
    assert_eq!(
        three["source"],
        json!({"designator": "U1", "current_a": 1.0})
    );
    assert_eq!(
        three["loads"],
        json!([{"designator": "U2", "current_a": 0.25}])
    );
    assert_eq!(three["total_a"], 0.25);
    assert_eq!(three["headroom_a"], 0.75);
    assert_eq!(three["problems"], json!([]));
    let five = &value["rails"][1];
    assert_eq!(five["net"], "+5V");
    assert_eq!(five["source"]["designator"], "U3");
    assert_eq!(five["loads"][0]["designator"], "U1");
    assert!(
        !app.document_state.projects[0]
            .data
            .rails()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn the_rail_budget_names_a_project_and_says_what_it_could_not_read() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(&dir);
    let reason = refusal(
        &mut app,
        "project_set_rail",
        &["net=+3V3", "voltage=3.3", "remove=true"],
    );
    assert!(reason.contains("`remove`"), "{reason}");
    let value = answer(&mut app, "query_rail_budget", &[]);
    assert_eq!(value["count"], 0, "no rail listed or named: {value}");
    assert_eq!(value["rails"], json!([]));
}
