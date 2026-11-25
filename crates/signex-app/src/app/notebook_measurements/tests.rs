//! The measurement commands on a project on disk (mockup row 199): each
//! reads the notebook off the UI thread and answers once it lands, the
//! writes go to the project's files, and what cannot be done is refused
//! with the reason.

use std::path::{Path, PathBuf};

use serde_json::Value;
use signex_i18n::RenderTranslatable;
use signex_types::format::SnxSchematic;
use signex_types::schematic::{SchematicSheet, Symbol};
use uuid::Uuid;

use super::design::values_of;
use crate::app::Signex;
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::keymap::AppCommandId;

const NOTEBOOK: &str = "+++\nformat = \"snxnbk/1\"\n+++\n#import \"@signex/notebook:0.1.0\": *\n= Bring-up\n\n#include \"chapters/bench.snxnbk\"\n";

const BENCH: &str = "+++\nformat = \"snxnbk/1\"\n+++\n= Bench\n\n#measurement(id: \"MEAS-002\", target: net(\"+3V3\"), quantity: \"ripple\", value: 41.0, unit: \"mV\", limit: (max: 30), timestamp: \"2026-10-02\")\n#measurement(id: \"MEAS-005\", target: net(\"+3V3\"), quantity: \"ripple\", value: 27.0, unit: \"mV\", limit: (max: 30), timestamp: \"2026-10-09\", taken-with: (\"L1\": \"3.3 µH\"))\n";

fn part(reference: &str, value: &str) -> Symbol {
    Symbol {
        uuid: Uuid::now_v7(),
        lib_id: "Device:L".into(),
        reference: reference.into(),
        value: value.into(),
        unit: 1,
        ..Symbol::default()
    }
}

/// A project whose one sheet holds L1 = 4.7 µH and C15 = 22 µF, with the
/// notebook and its chapter written.
fn project(dir: &tempfile::TempDir) -> (Signex, PathBuf) {
    let mut sheet = SchematicSheet::default();
    sheet.symbols.push(part("L1", "4.7 µH"));
    sheet.symbols.push(part("C15", "22 µF"));
    let text = SnxSchematic::new(sheet).write_string().expect("encode");
    std::fs::write(dir.path().join("top.snxsch"), text).expect("write sheet");
    let notebook = dir.path().join("notebook");
    std::fs::create_dir_all(notebook.join("chapters")).expect("folders");
    std::fs::write(notebook.join("design.snxnbk"), NOTEBOOK).expect("notebook");
    std::fs::write(notebook.join("chapters/bench.snxnbk"), BENCH).expect("chapter");
    let app = app_workspace(&dir.path().to_string_lossy(), &["top.snxsch"]);
    let project = app.document_state.projects[0].path.clone();
    (app, project)
}

/// Run `id` with `operands` after `document=<project>`, its read settled.
fn run(app: &mut Signex, project: &Path, id: &str, operands: &[&str]) -> CommandOutcome {
    let command = AppCommandId::new(id).expect("a catalog id");
    let document = format!("document={}", project.display());
    let mut all = vec![document.as_str()];
    all.extend_from_slice(operands);
    let args = CommandArgs::parse_operands(&command, &all).expect("operands parse");
    let ran = match app.run_query(&command, &args) {
        Some(ran) => ran,
        None => app.run_operands(&command, &args),
    };
    match ran {
        (CommandOutcome::Deferred { ticket }, task) => app.settle_deferred(ticket, task).0,
        (outcome, _) => outcome,
    }
}

fn answer(outcome: CommandOutcome) -> Value {
    match outcome {
        CommandOutcome::Answered(value) => value,
        other => panic!("not answered: {other:?}"),
    }
}

fn refusal(outcome: CommandOutcome) -> String {
    match outcome {
        CommandOutcome::Disabled { reason } => reason.render_en_us(),
        other => panic!("not refused: {other:?}"),
    }
}

#[test]
fn the_measurements_answer_reads_the_notebook_and_its_chapters() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, project) = project(&dir);
    let found = answer(run(&mut app, &project, "notebook_measurements", &[]));
    assert_eq!(found["notebook"], "notebook/design.snxnbk");
    assert_eq!(found["summary"]["readings"], 2);
    assert_eq!(found["summary"]["fail"], 1);
    assert_eq!(found["readings"][0]["file"], "chapters/bench.snxnbk");
    assert_eq!(found["load_problems"], serde_json::json!([]));
}

#[test]
fn a_part_value_changed_since_makes_the_reading_stale() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, project) = project(&dir);
    let found = answer(run(&mut app, &project, "notebook_measurements", &[]));
    assert_eq!(found["summary"]["stale_readings"], 1, "L1 is 4.7 µH now");
    assert_eq!(found["readings"][1]["changes"][0]["now"], "4.7 µH");
}

#[test]
fn the_timeline_answers_for_a_net_and_refuses_two_targets_or_a_bad_pin() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, project) = project(&dir);
    let line = answer(run(&mut app, &project, "notebook_timeline", &["net=+3V3"]));
    assert_eq!(
        line["timeline"]["entries"][0]["id"], "MEAS-005",
        "newest first"
    );
    assert_eq!(line["timeline"]["fails"], 1);
    assert_eq!(line["quantities"], serde_json::json!(["ripple"]));
    let two = refusal(run(
        &mut app,
        &project,
        "notebook_timeline",
        &["net=+3V3", "part=L1"],
    ));
    assert!(two.contains("not several"), "{two}");
    let pin = refusal(run(&mut app, &project, "notebook_timeline", &["pin=U3"]));
    assert!(pin.contains("`U3.3`"), "{pin}");
    let none = refusal(run(&mut app, &project, "notebook_timeline", &[]));
    assert!(none.contains("`net`, `part` or `pin`"), "{none}");
}

#[test]
fn the_timeline_exports_as_csv() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, project) = project(&dir);
    let csv = dir.path().join("ripple.csv");
    let path = format!("path={}", csv.display());
    let done = answer(run(
        &mut app,
        &project,
        "export_notebook_timeline",
        &["net=+3V3", path.as_str()],
    ));
    assert_eq!(done["readings"], 2);
    let written = std::fs::read_to_string(&csv).expect("the CSV");
    assert!(written.starts_with(signex_notebook::measurement::CSV_HEADER));
    assert_eq!(written.lines().count(), 3);
}

#[test]
fn mark_still_valid_writes_the_current_values_into_the_notebook() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, project) = project(&dir);
    let marked = answer(run(
        &mut app,
        &project,
        "mark_notebook_attachment_valid",
        &["id=MEAS-005"],
    ));
    assert_eq!(marked["file"], "notebook/chapters/bench.snxnbk");
    let chapter = dir.path().join("notebook/chapters/bench.snxnbk");
    let text = std::fs::read_to_string(chapter).expect("the chapter");
    assert!(text.contains("taken-with: (\"L1\": \"4.7 µH\")"), "{text}");
    let found = answer(run(&mut app, &project, "notebook_measurements", &[]));
    assert_eq!(found["summary"]["stale_readings"], 0);
    let nothing = refusal(run(
        &mut app,
        &project,
        "mark_notebook_attachment_valid",
        &["id=MEAS-002"],
    ));
    assert!(nothing.contains("stores no part values"), "{nothing}");
}

#[test]
fn an_attached_file_is_copied_into_the_project_and_its_figure_added() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, project) = project(&dir);
    let outside = tempfile::tempdir().unwrap();
    let capture = outside.path().join("ripple 2A.csv");
    std::fs::write(&capture, "t,v\n0,1\n1,2\n").expect("the capture");
    let path = format!("path={}", capture.display());
    let done = answer(run(
        &mut app,
        &project,
        "attach_notebook_file",
        &[path.as_str(), "part=C15", "text=Ripple at 2 A"],
    ));
    assert_eq!(
        done["copied_to"],
        "notebook/attachments/scope/ripple-2A.csv"
    );
    let copied = dir.path().join("notebook/attachments/scope/ripple-2A.csv");
    assert_eq!(
        std::fs::read_to_string(copied).expect("copied"),
        "t,v\n0,1\n1,2\n"
    );
    let notebook =
        std::fs::read_to_string(dir.path().join("notebook/design.snxnbk")).expect("the notebook");
    let call = "#attachment(\"scope/ripple-2A.csv\", caption: \"Ripple at 2 A\", target: component(\"C15\"), taken-with: (\"C15\": \"22 µF\"))";
    assert!(notebook.ends_with(&format!("\n\n{call}\n")), "{notebook}");
    let listed = answer(run(&mut app, &project, "notebook_attachments", &[]));
    assert_eq!(listed["summary"]["attachments"], 1);
    assert_eq!(listed["attachments"][0]["present"], true);
}

#[test]
fn a_project_without_a_notebook_is_refused_with_the_reason() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, project) = project(&dir);
    std::fs::remove_file(dir.path().join("notebook/design.snxnbk")).expect("remove");
    let reason = refusal(run(&mut app, &project, "notebook_measurements", &[]));
    assert!(
        reason.contains("notebook/design.snxnbk cannot be read"),
        "{reason}"
    );
}

#[test]
fn a_document_of_no_open_project_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = project(&dir);
    let elsewhere = dir.path().join("elsewhere.snxprj");
    let reason = refusal(run(&mut app, &elsewhere, "notebook_measurements", &[]));
    assert!(reason.contains("elsewhere.snxprj"), "{reason}");
}

#[test]
fn the_design_values_leave_out_power_and_unannotated_parts() {
    let mut sheet = SchematicSheet::default();
    sheet.symbols.push(part("L1", "4.7 µH"));
    sheet.symbols.push(part("R?", "10k"));
    sheet.symbols.push(Symbol {
        is_power: true,
        ..part("#PWR01", "VCC")
    });
    let values = values_of([&sheet]);
    assert_eq!(values.get("L1"), Some("4.7 µH"));
    assert_eq!(values.get("R?"), None);
    assert_eq!(values.get("#PWR01"), None);
}
