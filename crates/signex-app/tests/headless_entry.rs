//! #1933: the public headless entry. Boot without a window under a
//! redirected config root, with an explicit ERC rule profile, run commands by
//! id and read what happened. The override is process-wide, so each scenario
//! runs in a child process of this binary ([`probe`]).

#![allow(
    clippy::disallowed_methods,
    reason = "the test re-runs its own binary as a child process"
)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use signex_app::app::Signex;
use signex_app::config_root::{default_config_root, set_config_root_override};
use signex_app::fonts::catalog::{FontSet, install_before_boot};
use signex_app::headless::{CommandArgs, CommandOutcome, HeadlessBootError};
use signex_app::keymap::AppCommandId;
use signex_app::task_driver::TaskDriver;
use signex_types::project::{ErcRules, RuleProfile};

const PROBE_ENV: &str = "SIGNEX_HEADLESS_ENTRY_PROBE";

fn run_probe(scenario: &str) {
    let output = std::process::Command::new(std::env::current_exe().expect("test binary"))
        .args(["probe", "--exact", "--nocapture", "--test-threads=1"])
        .env(PROBE_ENV, scenario)
        .output()
        .expect("run the probe");
    assert!(
        output.status.success(),
        "scenario {scenario} failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn a_headless_boot_runs_commands_by_id_and_leaves_the_real_config_alone() {
    run_probe("run");
}

#[test]
fn a_headless_boot_is_refused_without_a_config_root_override() {
    run_probe("no_override");
}

#[test]
fn probe() {
    let Ok(scenario) = std::env::var(PROBE_ENV) else {
        return;
    };
    match scenario.as_str() {
        "run" => run(),
        "no_override" => no_override(),
        other => panic!("unknown scenario {other}"),
    }
}

/// `unused_pin` switched off: a rule the machine profile does not change.
fn rules() -> RuleProfile {
    let mut erc = ErcRules::default();
    erc.severity.insert("unused_pin".into(), "off".into());
    RuleProfile::new("Headless", "1", erc)
}

fn id(name: &str) -> AppCommandId {
    AppCommandId::new(name).expect("a valid id")
}

fn no_override() {
    let refused = Signex::boot_headless(Vec::new(), &rules()).err();
    assert_eq!(refused, Some(HeadlessBootError::NoConfigRootOverride));
    // An app booted the GUI way runs nothing through the headless entry.
    let (mut app, _boot) = Signex::new();
    let grid = app.ui_state.grid_visible;
    let (outcome, _task) =
        app.dispatch_command_headless(&id("toggle_visible_grid"), CommandArgs::none());
    assert_eq!(outcome, CommandOutcome::NotHeadless);
    assert_eq!(app.ui_state.grid_visible, grid, "nothing ran");
    // Headless ports without the override: still refused.
    app.ui_state.io = signex_app::io_ports::IoPorts::headless(Default::default());
    let (outcome, _task) =
        app.dispatch_command_headless(&id("toggle_visible_grid"), CommandArgs::none());
    assert_eq!(outcome, CommandOutcome::NotHeadless);
    assert_eq!(app.ui_state.grid_visible, grid, "nothing ran");
}

fn run() {
    let real = default_config_root().expect("the test redirect resolves");
    std::fs::create_dir_all(&real).expect("real root");
    std::fs::write(real.join("prefs.json"), br#"{"grid_visible":true}"#).expect("seed");
    let before = snapshot(&real);

    let isolated = tempfile::tempdir().expect("override root");
    set_config_root_override(isolated.path().to_path_buf()).expect("override");
    install_before_boot(FontSet::BundledOnly).expect("fonts");
    let relative = PathBuf::from("relative/demo.snxprj");
    assert_eq!(
        Signex::boot_headless(vec![relative.clone()], &rules()).err(),
        Some(HeadlessBootError::NotAbsolute(relative))
    );
    // Native dialogs: not a headless app, even with the override set.
    let (mut native, _boot) = Signex::new();
    let (outcome, _task) =
        native.dispatch_command_headless(&id("toggle_visible_grid"), CommandArgs::none());
    assert_eq!(outcome, CommandOutcome::NotHeadless);

    let (mut app, boot) = Signex::boot_headless(Vec::new(), &rules()).expect("boot");
    let mut driver = TaskDriver::new();
    driver.settle(&mut app, boot).expect("boot settled");
    assert_eq!(
        app.ui_state
            .erc_severity_override
            .get(&signex_erc::RuleKind::UnusedPin),
        Some(&signex_erc::Severity::Off),
        "the explicit rule profile is in force"
    );
    assert_eq!(
        app.ui_state.io.opener,
        signex_app::io_ports::Opener::Refuse,
        "launches are refused"
    );
    // Headless dialogs but native launches: still not a headless app.
    app.ui_state.io.opener = signex_app::io_ports::Opener::Native;
    let (outcome, _task) =
        app.dispatch_command_headless(&id("toggle_visible_grid"), CommandArgs::none());
    assert_eq!(outcome, CommandOutcome::NotHeadless);
    app.ui_state.io.opener = signex_app::io_ports::Opener::Refuse;

    let grid_before = app.ui_state.grid_visible;
    let (outcome, task) =
        app.dispatch_command_headless(&id("toggle_visible_grid"), CommandArgs::none());
    assert_eq!(outcome, CommandOutcome::Ran);
    driver.settle(&mut app, task).expect("command settled");
    assert_eq!(app.ui_state.grid_visible, !grid_before, "the command ran");

    // One answered dialog: Open takes the `path` answer, so the open of a
    // file that is not there reports that file.
    let missing = isolated.path().join("zq-missing.snxsch");
    app.headless_answers()
        .expect("headless answers")
        .replace_all([("path".to_string(), missing.display().to_string())]);
    let (outcome, task) = app.dispatch_command_headless(&id("open_document"), CommandArgs::none());
    assert_eq!(outcome, CommandOutcome::Ran);
    driver.settle(&mut app, task).expect("open settled");
    let notice = app
        .document_state
        .error_notice
        .take()
        .expect("the open was tried");
    assert!(
        notice.detail.contains("zq-missing.snxsch"),
        "{}",
        notice.detail
    );

    let (outcome, _task) =
        app.dispatch_command_headless(&id("zq_no_such_command"), CommandArgs::none());
    assert_eq!(outcome, CommandOutcome::UnknownId);
    // ERC needs a schematic, and none is open.
    let (outcome, _task) = app.dispatch_command_headless(&id("run_erc"), CommandArgs::none());
    assert!(
        matches!(outcome, CommandOutcome::Disabled { .. }),
        "{outcome:?}"
    );

    let end = app.end_headless_session();
    driver.settle(&mut app, end).expect("session ended");
    assert!(driver.has_exited());
    let written = std::fs::read_to_string(isolated.path().join("prefs.json")).expect("prefs");
    assert!(written.contains("grid_visible"), "{written}");
    assert_eq!(snapshot(&real), before, "the real config root changed");
}

#[test]
fn the_snapshot_sees_a_folder_only_change() {
    let root = tempfile::tempdir().expect("root");
    let before = snapshot(root.path());
    std::fs::create_dir(root.path().join("recovery")).expect("folder");
    assert_ne!(snapshot(root.path()), before);
}

/// Every file and folder under `root`, with a file's bytes.
fn snapshot(root: &Path) -> BTreeMap<PathBuf, Option<Vec<u8>>> {
    let mut out = BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("read dir") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                out.insert(path.clone(), None);
                stack.push(path);
            } else {
                out.insert(path.clone(), Some(std::fs::read(&path).expect("read")));
            }
        }
    }
    out
}
