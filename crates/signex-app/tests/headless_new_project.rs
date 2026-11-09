//! #2232: `new_project_with_profile` creates a project with the dialog's
//! choices and no dialog, through the public headless entry. The config-root
//! override is process-wide, so each scenario runs in a child process.

#![allow(
    clippy::disallowed_methods,
    reason = "the test re-runs its own binary as a child process"
)]

use std::path::PathBuf;

use signex_app::app::Signex;
use signex_app::config_root::set_config_root_override;
use signex_app::fonts::catalog::{FontSet, install_before_boot};
use signex_app::headless::{CommandArgs, CommandOutcome};
use signex_app::keymap::AppCommandId;
use signex_app::task_driver::TaskDriver;
use signex_types::project::{ErcRules, RuleProfile};

const PROBE_ENV: &str = "SIGNEX_HEADLESS_NEW_PROJECT_PROBE";

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
fn the_command_creates_a_project_with_the_profile_and_the_overrides() {
    run_probe("with_profile");
}

#[test]
fn the_command_refuses_bad_operands_and_writes_nothing() {
    run_probe("refusals");
}

#[test]
fn probe() {
    let Ok(scenario) = std::env::var(PROBE_ENV) else {
        return;
    };
    match scenario.as_str() {
        "with_profile" => with_profile(),
        "refusals" => refusals(),
        other => panic!("unknown scenario {other}"),
    }
}

struct Session {
    app: Signex,
    driver: TaskDriver,
    _root: tempfile::TempDir,
}

impl Session {
    fn boot() -> Self {
        let root = tempfile::tempdir().expect("override root");
        set_config_root_override(root.path().to_path_buf()).expect("override");
        install_before_boot(FontSet::BundledOnly).expect("fonts");
        let rules = RuleProfile::new("Headless", "1", ErcRules::default());
        let (mut app, boot) = Signex::boot_headless(Vec::new(), &rules).expect("boot");
        let mut driver = TaskDriver::new();
        driver.settle(&mut app, boot).expect("boot settled");
        Self {
            app,
            driver,
            _root: root,
        }
    }

    fn run(&mut self, operands: &[&str]) -> CommandOutcome {
        let command = AppCommandId::new("new_project_with_profile").expect("id");
        let args = CommandArgs::parse_operands(&command, operands).expect("operands");
        let (outcome, task) = self.app.dispatch_command_headless(&command, args);
        self.driver.settle(&mut self.app, task).expect("settled");
        outcome
    }
}

fn with_profile() {
    let mut session = Session::boot();
    let place = tempfile::tempdir().expect("location");
    let location = format!("location={}", place.path().display());
    let answer = match session.run(&[
        "name=Motor Driver Rev A",
        &location,
        "profile=hobby",
        "overrides=layers=6,clearance=0.12,sheet=A4",
    ]) {
        CommandOutcome::Answered(value) => value,
        other => panic!("not answered: {other:?}"),
    };
    assert_eq!(answer["kind"], "project");
    let path = PathBuf::from(answer["path"].as_str().expect("path"));
    assert_eq!(
        path,
        place
            .path()
            .join("Motor Driver Rev A")
            .join("Motor Driver Rev A.snxprj")
    );
    let data = signex_types::project::parse_project(&path).expect("project");
    let pcb = data.project_defaults.pcb.expect("pcb defaults");
    assert_eq!((pcb.copper_layers, pcb.clearance_mm), (6, 0.12));
    assert_eq!(pcb.track_width_mm, 0.25, "the rest is the hobby profile");
    assert_eq!(data.project_defaults.sheet.expect("sheet").paper_size, "A4");

    // Without the folder, straight into the location.
    let flat = match session.run(&["name=Flat", &location, "make_folder=false"]) {
        CommandOutcome::Answered(value) => value,
        other => panic!("not answered: {other:?}"),
    };
    assert_eq!(
        PathBuf::from(flat["path"].as_str().expect("path")),
        place.path().join("Flat.snxprj")
    );
}

fn refusals() {
    let mut session = Session::boot();
    let place = tempfile::tempdir().expect("location");
    let location = format!("location={}", place.path().display());
    for (operands, want) in [
        (
            vec!["name=a/b", location.as_str()],
            "not a usable project name",
        ),
        (
            vec!["name=P", location.as_str(), "profile=gold"],
            "profile `gold`",
        ),
        (
            vec!["name=P", location.as_str(), "overrides=clearance=9"],
            "clearance",
        ),
    ] {
        match session.run(&operands) {
            CommandOutcome::Disabled { reason } => {
                assert!(reason.contains(want), "{operands:?}: {reason}")
            }
            other => panic!("{operands:?} was not refused: {other:?}"),
        }
    }
    assert_eq!(
        std::fs::read_dir(place.path()).expect("dir").count(),
        0,
        "nothing written"
    );
}
