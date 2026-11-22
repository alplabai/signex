//! Row m38 (Requirements). The section's own messages are not public, so the
//! row runs through the catalog commands, which share the tab's commit path.
//! Those need a headless app, and its config root is process-wide: each
//! scenario runs in a child process of this binary.

use std::path::PathBuf;

use serde_json::Value;
use signex_app::app::{FileMsg, Message, ProjectId, Signex};
use signex_app::config_root::set_config_root_override;
use signex_app::fonts::catalog::{FontSet, install_before_boot};
use signex_app::headless::{CommandArgs, CommandOutcome};
use signex_app::keymap::AppCommandId;
use signex_app::task_driver::TaskDriver;
use signex_types::project::{ErcRules, RuleProfile};
use tempfile::TempDir;

use super::support::write_project_files;
use crate::settle_support::settle;

const SCENARIO: &str = "SIGNEX_FUNCTION_AUDIT_REQUIREMENTS";

#[allow(
    clippy::disallowed_methods,
    reason = "the scenario re-runs this test binary as a child process"
)]
fn run_scenario(name: &str) {
    let output = std::process::Command::new(std::env::current_exe().expect("test binary"))
        .args([
            "b_project::requirements::requirements_probe",
            "--exact",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(SCENARIO, name)
        .output()
        .expect("run the scenario");
    assert!(
        output.status.success(),
        "scenario {name} failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn requirements_probe() {
    let Ok(scenario) = std::env::var(SCENARIO) else {
        return;
    };
    match scenario.as_str() {
        "add_edit" => add_edit(),
        "toggle_remove" => toggle_remove(),
        "refused" => refused(),
        "run" => run_checks(),
        other => panic!("unknown scenario {other}"),
    }
}

#[test]
fn m38_add_and_a_field_edit_change_the_list_and_undo_takes_them_back() {
    run_scenario("add_edit");
}

#[test]
fn m38_the_enabled_switch_and_remove_change_the_list_and_undo_restores_it() {
    run_scenario("toggle_remove");
}

#[test]
fn m38_a_refused_kind_or_value_changes_nothing() {
    run_scenario("refused");
}

#[test]
fn m38_run_checks_passes_a_clean_requirement_and_skips_one_turned_off() {
    run_scenario("run");
}

struct Session {
    app: Box<Signex>,
    driver: TaskDriver,
    id: ProjectId,
    prj: PathBuf,
    document: String,
    _config: TempDir,
    _project: TempDir,
}

fn session() -> Session {
    let config = TempDir::new().expect("config root");
    set_config_root_override(config.path().to_path_buf()).expect("override");
    install_before_boot(FontSet::BundledOnly).expect("fonts");
    let rules = RuleProfile::new("Audit", "1", ErcRules::default());
    let (mut app, boot) = Signex::boot_headless(Vec::new(), &rules).expect("boot");
    let mut driver = TaskDriver::new();
    driver.settle(&mut app, boot).expect("boot settled");
    let project = TempDir::new().expect("project folder");
    let written = write_project_files(project.path(), &[("root.snxsch", "")]);
    settle(&mut app, Message::File(FileMsg::Opened(Some(written))));
    let loaded = app.document_state.projects.first().expect("project open");
    let (id, prj) = (loaded.id, loaded.path.clone());
    let document = format!("document={}", prj.display());
    Session {
        app,
        driver,
        id,
        prj,
        document,
        _config: config,
        _project: project,
    }
}

impl Session {
    fn run(&mut self, name: &str, operands: &[&str]) -> CommandOutcome {
        let command = AppCommandId::new(name).expect("a catalog command");
        let mut all: Vec<&str> = vec![self.document.as_str()];
        all.extend_from_slice(operands);
        let args = CommandArgs::parse_operands(&command, &all).expect("operands");
        let (outcome, task) = self.app.dispatch_command_headless(&command, args);
        self.driver.settle(&mut self.app, task).expect("settled");
        outcome
    }

    fn edit(&mut self, name: &str, operands: &[&str]) {
        let outcome = self.run(name, operands);
        assert!(
            matches!(outcome, CommandOutcome::Answered(_)),
            "{name} {operands:?}: {outcome:?}"
        );
    }

    fn refused(&mut self, name: &str, operands: &[&str]) {
        let before = self.list();
        let outcome = self.run(name, operands);
        assert!(
            !matches!(outcome, CommandOutcome::Answered(_)),
            "{name} {operands:?} was accepted: {outcome:?}"
        );
        assert_eq!(self.list(), before, "{name} {operands:?} changed the list");
    }

    fn list(&self) -> Vec<Value> {
        self.app.document_state.projects[0]
            .data
            .requirements
            .iter()
            .map(|entry| entry.raw().clone())
            .collect()
    }

    fn dirty(&self) -> bool {
        self.app.document_state.is_document_dirty(&self.prj)
    }

    fn undo(&mut self) {
        let outcome = self.run("undo", &[]);
        assert!(
            !matches!(outcome, CommandOutcome::Disabled { .. }),
            "undo: {outcome:?}"
        );
    }
}

fn add_edit() {
    let mut s = session();
    s.edit("project_add_requirement", &["kind=board_size", "id=size"]);
    assert_eq!(s.list().len(), 1);
    assert_eq!(s.list()[0]["kind"], "board_size");
    assert_eq!(s.list()[0]["id"], "size");
    assert!(s.dirty());
    let added = s.list();

    s.edit(
        "project_set_requirement",
        &["requirement=size", "field=max_width_mm", "value=120"],
    );
    assert_eq!(s.list()[0]["max_width_mm"].as_f64(), Some(120.0));

    s.undo();
    assert_eq!(s.list(), added, "undo takes the field back");
    s.undo();
    assert!(s.list().is_empty(), "undo takes the add back");
    assert!(!s.dirty(), "back at the save point");
}

fn toggle_remove() {
    let mut s = session();
    s.edit("project_add_requirement", &["kind=erc_clean", "id=erc"]);
    let on = s.list();

    s.edit(
        "project_enable_requirement",
        &["requirement=erc", "on=false"],
    );
    assert_eq!(s.list()[0]["enabled"], false);
    s.undo();
    assert_eq!(s.list(), on, "undo turns it back on");

    s.edit("project_remove_requirement", &["requirement=erc"]);
    assert!(s.list().is_empty());
    s.undo();
    assert_eq!(s.list(), on, "undo restores the removed entry");
}

fn refused() {
    let mut s = session();
    s.refused("project_add_requirement", &["kind=no_such_kind"]);
    assert!(!s.dirty(), "a refused add leaves the project clean");

    s.edit("project_add_requirement", &["kind=board_size", "id=size"]);
    for bad in ["value=-5", "value=wide", "value=0"] {
        s.refused(
            "project_set_requirement",
            &["requirement=size", "field=max_width_mm", bad],
        );
    }
    s.refused(
        "project_set_requirement",
        &["requirement=size", "field=currency", "value=USD"],
    );
    s.refused("project_add_requirement", &["kind=erc_clean", "id=size"]);
    s.refused("project_remove_requirement", &["requirement=ghost"]);
}

fn run_checks() {
    let mut s = session();
    s.edit("project_add_requirement", &["kind=erc_clean", "id=erc"]);
    s.edit("project_add_requirement", &["kind=board_size", "id=size"]);
    s.edit(
        "project_enable_requirement",
        &["requirement=size", "on=false"],
    );
    let list = s.list();
    let outcome = s.run("project_run_requirements", &[]);
    assert!(matches!(outcome, CommandOutcome::Ran), "{outcome:?}");

    let tab = s
        .app
        .document_state
        .project_tabs
        .get(&s.id)
        .expect("the tab state");
    assert!(!tab.requirements.running);
    let run = tab.requirements.last_run.as_ref().expect("a finished run");
    let report = run.report.as_ref().expect("the run finished");
    assert!(
        report.passed.iter().any(|rule| rule == "req::erc"),
        "an empty sheet passes erc_clean: {:?}",
        report.passed
    );
    assert!(
        !report.passed.iter().any(|rule| rule == "req::size"),
        "Run checks skips a requirement turned off"
    );
    assert!(report.findings.is_empty(), "{:?}", report.findings);
    assert_eq!(s.list(), list, "a run edits nothing");
}
