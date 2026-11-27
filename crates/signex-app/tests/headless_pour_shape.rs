//! #2578 / #2579: a pour's corners and hatched fill set through the public
//! headless entry (`set_pour_corners`, `set_pour_fill`), on the board the
//! command names, then saved: the file carries the keys and `snxpcb/1.12`.
//! The config-root override is process-wide, so the scenario runs in a child
//! process of this binary ([`probe`]).

#![allow(
    clippy::disallowed_methods,
    reason = "the test re-runs its own binary as a child process"
)]

use std::path::Path;

use serde_json::{Value, json};
use signex_app::app::Signex;
use signex_app::config_root::set_config_root_override;
use signex_app::fonts::catalog::{FontSet, install_before_boot};
use signex_app::headless::{CommandArgs, CommandOutcome};
use signex_app::keymap::AppCommandId;
use signex_app::task_driver::TaskDriver;
use signex_types::pcb::PcbBoard;
use signex_types::project::{ErcRules, RuleProfile};

const PROBE_ENV: &str = "SIGNEX_HEADLESS_POUR_SHAPE_PROBE";
const BOARD: &str = "0193a2f0-7b1c-7cde-8123-0000000000b1";
const POUR: &str = "0193a2f0-7b1c-7cde-8123-0000000000b2";

#[test]
fn a_headless_run_shapes_and_hatches_a_pour_and_saves_it() {
    let output = std::process::Command::new(std::env::current_exe().expect("test binary"))
        .args(["probe", "--exact", "--nocapture", "--test-threads=1"])
        .env(PROBE_ENV, "shape")
        .output()
        .expect("run the probe");
    assert!(
        output.status.success(),
        "the probe failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn probe() {
    if std::env::var(PROBE_ENV).is_ok() {
        shape();
    }
}

struct Session {
    app: Box<Signex>,
    driver: TaskDriver,
}

impl Session {
    fn boot() -> Self {
        let rules = RuleProfile::new("Headless", "1", ErcRules::default());
        let (mut app, boot) = Signex::boot_headless(Vec::new(), &rules).expect("boot");
        let mut driver = TaskDriver::new();
        driver.settle(&mut app, boot).expect("boot settled");
        Self { app, driver }
    }

    fn run(&mut self, name: &str, operands: &[&str]) -> CommandOutcome {
        let command = AppCommandId::new(name).expect("a valid id");
        let args = if operands.is_empty() {
            CommandArgs::none()
        } else {
            CommandArgs::parse_operands(&command, operands).expect("operands")
        };
        let (outcome, task) = self.app.dispatch_command_headless(&command, args);
        self.driver.settle(&mut self.app, task).expect("settled");
        outcome
    }

    fn answer(&mut self, name: &str, operands: &[&str]) -> Value {
        match self.run(name, operands) {
            CommandOutcome::Answered(value) => value,
            other => panic!("{name} {operands:?} was not answered: {other:?}"),
        }
    }

    fn open(&mut self, path: &Path) {
        self.app
            .headless_answers()
            .expect("headless answers")
            .replace_all([("path".to_string(), path.display().to_string())]);
        let outcome = self.run("open_document", &[]);
        assert!(
            !matches!(outcome, CommandOutcome::Disabled { .. }),
            "open: {outcome:?}"
        );
    }
}

/// A board holding one 10 x 10 mm pour on the top copper and nothing else.
fn write_board(path: &Path) {
    let board: PcbBoard = serde_json::from_value(json!({
        "uuid": BOARD,
        "setup": {"clearance": 0.2},
        "zones": [{
            "uuid": POUR,
            "layer": "TopCopper",
            "fill_type": "pour",
            "outline": [
                {"x": 0.0, "y": 0.0}, {"x": 10.0, "y": 0.0},
                {"x": 10.0, "y": 10.0}, {"x": 0.0, "y": 10.0}
            ]
        }]
    }))
    .expect("board");
    let text = signex_types::format::SnxPcb::new(board)
        .write_string()
        .expect("board text");
    std::fs::write(path, text).expect("board file");
}

fn shape() {
    let root = tempfile::tempdir().expect("override root");
    set_config_root_override(root.path().to_path_buf()).expect("override");
    install_before_boot(FontSet::BundledOnly).expect("fonts");
    let path = root.path().join("board.snxpcb");
    write_board(&path);
    let mut session = Session::boot();
    session.open(&path);
    let document = format!("document={}", path.display());
    let ids = format!("ids={POUR}");

    let answer = session.answer(
        "set_pour_corners",
        &[&document, &ids, "corners=chamfer", "corner_size=1"],
    );
    let pour = &answer["pours"][0];
    assert_eq!(pour["zone"], POUR);
    assert_eq!(pour["corners"], "chamfer");
    // Four 0.5 mm² corners cut off the 100 mm² pour.
    let area = pour["area_mm2"].as_f64().expect("area");
    assert!((area - 98.0).abs() < 1e-6, "{area}");

    let answer = session.answer("set_pour_fill", &[&document, &ids, "fill=hatched"]);
    let pour = &answer["pours"][0];
    assert_eq!(pour["fill"], "hatched");
    assert!(pour["area_mm2"].as_f64().expect("area") < 98.0);

    session.run("save_document", &[&document]);
    let saved = std::fs::read_to_string(&path).expect("saved board");
    assert!(saved.starts_with("format = \"snxpcb/1.12\""), "{saved}");
    assert!(saved.contains("corners = \"chamfer\""), "{saved}");
    assert!(saved.contains("fill_mode = \"hatched\""), "{saved}");

    // One undo step each, on the named board.
    session.run("undo", &[&document]);
    let answer = session.answer("repour_all", &[&document]);
    let pour = &answer["pours"][0];
    assert!((pour["area_mm2"].as_f64().expect("area") - 98.0).abs() < 1e-6);
}
