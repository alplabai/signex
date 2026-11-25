//! #2945 (mockup row 201): `generate_design_notebook` on a project on disk.
//! It answers with the summary once its run lands, writes the report only
//! with `save`, never over an existing one without `update`, gives the same
//! file twice, reads the hand-written notebook's blocks, and the options
//! dialog runs the same code and keeps the report unsaved.

use std::path::{Path, PathBuf};

use serde_json::Value;
use signex_i18n::RenderTranslatable;
use signex_notebook::generate::{Section, generated_path};

use super::project_name_tests::project_app;
use super::{CommandArgs, CommandOutcome};
use crate::app::{NotebookGenerateMsg, Signex};
use crate::keymap::AppCommandId;

fn project_path(app: &Signex) -> PathBuf {
    app.document_state.projects[0].path.clone()
}

fn run(app: &mut Signex, extra: &[&str]) -> CommandOutcome {
    let id = AppCommandId::new("generate_design_notebook").expect("a catalog id");
    let document = format!("document={}", project_path(app).display());
    let mut operands = vec![document.as_str()];
    operands.extend_from_slice(extra);
    let args = CommandArgs::parse_operands(&id, &operands).expect("operands parse");
    app.run_operands_settled(&id, &args).0
}

fn summary(app: &mut Signex, extra: &[&str]) -> Value {
    match run(app, extra) {
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

fn report_file(dir: &Path) -> PathBuf {
    dir.join(generated_path())
}

#[test]
fn the_command_answers_with_the_summary_and_writes_nothing_without_save() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(&dir);
    let value = summary(&mut app, &[]);
    assert_eq!(value["command"], "notebook.generate");
    assert_eq!(value["output"], "notebook/generated/design-report.snxnbk");
    assert_eq!(value["sections"].as_array().map(Vec::len), Some(10));
    assert_eq!(value["written"], false);
    assert_eq!(value["status"], "ok");
    assert!(value["pages"].as_u64().is_some_and(|pages| pages >= 1));
    assert!(!report_file(dir.path()).exists());
}

#[test]
fn save_writes_the_report_and_never_over_one_without_update() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(&dir);
    let value = summary(&mut app, &["save=true"]);
    assert_eq!(value["written"], true);
    let written = std::fs::read_to_string(report_file(dir.path())).expect("the report");
    assert!(written.starts_with("+++\nformat = \"snxnbk/1\"\n"));

    std::fs::write(report_file(dir.path()), "kept").unwrap();
    let why = refusal(run(&mut app, &["save=true"]));
    assert!(why.contains("already exists"), "{why}");
    assert_eq!(
        std::fs::read_to_string(report_file(dir.path())).unwrap(),
        "kept"
    );

    let value = summary(&mut app, &["save=true", "update=true"]);
    assert_eq!(value["written"], true);
    let replaced = std::fs::read_to_string(report_file(dir.path())).unwrap();
    assert_eq!(replaced, written, "two runs give the same notebook");
}

#[test]
fn the_hand_written_notebook_is_read_and_never_changed() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(&dir);
    let notebook = dir.path().join("notebook");
    std::fs::create_dir_all(&notebook).unwrap();
    let text = "#annotation(kind: \"issue\", id: \"ISS-004\", status: \"open\", title: \"Pad size\")[Check it.]\n";
    std::fs::write(notebook.join("design.snxnbk"), text).unwrap();
    let value = summary(&mut app, &["save=true"]);
    assert_eq!(value["open_items"], serde_json::json!(["ISS-004"]));
    assert_eq!(
        std::fs::read_to_string(notebook.join("design.snxnbk")).unwrap(),
        text
    );
}

#[test]
fn what_could_not_be_read_is_named_by_section() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(&dir);
    let value = summary(&mut app, &[]);
    let sections: Vec<&str> = value["not_read"]
        .as_array()
        .expect("not_read")
        .iter()
        .filter_map(|n| n["section"].as_str())
        .collect();
    for section in ["simulation", "outputs", "changelog", "decisions"] {
        assert!(sections.contains(&section), "{section}: {sections:?}");
    }
}

#[test]
fn the_last_results_are_the_newest_kept_check_runs() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(&dir);
    let value = summary(&mut app, &["checks=last"]);
    let kept = |value: &Value| {
        value["sources"]
            .as_array()
            .expect("sources")
            .iter()
            .any(|s| {
                s["section"] == "checks"
                    && s["from"]
                        .as_str()
                        .is_some_and(|f| f.starts_with("check_erc run"))
            })
    };
    assert!(!kept(&value), "no ERC run was kept yet");
    // A run now keeps its reports as the newest.
    summary(&mut app, &["checks=run"]);
    let value = summary(&mut app, &["checks=last"]);
    assert!(kept(&value), "{value}");
}

#[test]
fn bad_operands_are_refused_with_the_reason() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(&dir);
    let why = refusal(run(&mut app, &["sections=summary,appendix"]));
    assert!(why.contains("`appendix`"), "{why}");
    let why = refusal(run(&mut app, &["checks=now"]));
    assert!(why.contains("`run` or `last`"), "{why}");
    let why = refusal(run(&mut app, &["variant=Max"]));
    assert!(why.contains("`Max`"), "{why}");
    let id = AppCommandId::new("generate_design_notebook").expect("a catalog id");
    let why = refusal(
        app.run_operands_settled(
            &id,
            &CommandArgs::parse_operands(&id, &["sections=all"]).unwrap(),
        )
        .0,
    );
    assert!(why.contains("`document`"), "{why}");
}

#[test]
fn the_dialog_runs_the_same_code_and_keeps_the_report_unsaved() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = project_app(&dir);
    let _ = app.update(NotebookGenerateMsg::DialogOpened.into());
    let dialog = app
        .ui_state
        .notebook_generate
        .dialog
        .as_ref()
        .expect("open");
    assert_eq!(dialog.options.sections, Section::ALL.to_vec());
    assert_eq!(dialog.variant, "Base");
    let _ = app.update(NotebookGenerateMsg::SectionToggled(Section::Simulation).into());
    let task = app.update(NotebookGenerateMsg::GenerateClicked.into());
    assert!(
        app.ui_state
            .notebook_generate
            .dialog
            .as_ref()
            .is_some_and(|d| d.running)
    );
    crate::task_driver::TaskDriver::new()
        .settle(&mut app, task)
        .expect("the run lands");
    let state = &app.ui_state.notebook_generate;
    assert!(state.dialog.is_none(), "the dialog closes");
    let pending = state.pending.as_ref().expect("the report waits unsaved");
    assert!(!pending.summary.written);
    assert!(!pending.summary.sections.contains(&"simulation"));
    assert!(!report_file(dir.path()).exists());
}
