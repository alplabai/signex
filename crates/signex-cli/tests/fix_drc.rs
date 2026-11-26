//! End to end (#2940): the DRC fixes through the real `signex-cli` binary,
//! headless. `fix_drc` moves a track clear of another net and a check
//! afterwards finds nothing; `--dry-run` answers with the fix and changes
//! nothing; `--save` writes the fixed board; a run that does not save
//! writes nothing.

mod support;

use std::path::Path;

use serde_json::{Value, json};
use signex_types::pcb::{NetDef, Point, Segment};
use support::{jsonl, pins_fixture, run_cli, snapshot, temp_root, without_journals};

const GND_TRACK: u128 = 0x2001;

/// A board whose GND and VCC tracks are 0.1 mm apart, under the Standard
/// setup's 0.15 mm clearance.
fn write_board(path: &Path) {
    let mut board = signex_types::project::SetupProfile::Standard
        .pcb(2)
        .new_board();
    board.nets = vec![
        NetDef {
            number: 1,
            name: "GND".into(),
        },
        NetDef {
            number: 2,
            name: "VCC".into(),
        },
    ];
    board.segments = [(1, 0.0), (2, 0.35)]
        .into_iter()
        .map(|(net, y)| Segment {
            locked: false,
            attributes: Default::default(),
            uuid: uuid::Uuid::from_u128(0x2000 + u128::from(net)),
            start: Point::new(0.0, y),
            end: Point::new(10.0, y),
            width: 0.25,
            layer: "TopCopper".into(),
            net,
            unknown_columns: Default::default(),
        })
        .collect();
    let text = signex_types::format::SnxPcb::new(board)
        .write_string()
        .expect("encode");
    std::fs::write(path, text).expect("write board");
}

/// List `filename` as the fixture project's board.
fn set_board(project: &Path, filename: &str) {
    let mut data = signex_types::project::parse_project(project).expect("project");
    data.pcb_file = Some(filename.into());
    signex_types::project::write_project(project, &data).expect("project");
}

fn open() -> Value {
    json!({"id": "open_document", "args": {"path": "board.snxpcb"}})
}

fn fix() -> Value {
    json!({"id": "fix_drc", "args": {"document": "board.snxpcb", "rule": "clearance"}})
}

fn check() -> Value {
    json!({"id": "check_drc", "args": {"document": "board.snxpcb"}})
}

fn clearance_findings(check: &Value) -> usize {
    check["data"]["findings"].as_array().map_or(0, |all| {
        all.iter()
            .filter(|f| f["rule"] == "builtin::clearance")
            .count()
    })
}

#[test]
fn fix_drc_clears_a_clearance_finding_headless_and_writes_nothing_unsaved() {
    let (_guard, root) = temp_root();
    let fx = pins_fixture(&root);
    write_board(&fx.dir.join("board.snxpcb"));
    set_board(&fx.project, "board.snxpcb");
    let before = without_journals(snapshot(&root));
    let out = run_cli(
        &["run", fx.project.to_str().unwrap()],
        jsonl(&[open(), check(), fix(), check()]),
    );
    assert_eq!(clearance_findings(&out.lines[1]), 1, "{}", out.stdout);
    let fixed = &out.lines[2];
    assert_eq!(fixed["status"], "answered", "{}", out.stdout);
    let data = &fixed["data"];
    assert_eq!(data["changed"], true, "{}", out.stdout);
    assert_eq!(data["fixed"].as_array().map(Vec::len), Some(1));
    assert_eq!(data["fixed"][0]["rule"], "builtin::clearance");
    assert_eq!(data["left"], json!([]));
    assert_eq!(data["remaining"]["count"], 0, "{}", out.stdout);
    assert_eq!(clearance_findings(&out.lines[3]), 0, "{}", out.stdout);
    assert_eq!(without_journals(snapshot(&root)), before, "nothing written");
}

#[test]
fn a_dry_run_answers_with_the_fix_and_the_finding_stays() {
    let (_guard, root) = temp_root();
    let fx = pins_fixture(&root);
    write_board(&fx.dir.join("board.snxpcb"));
    set_board(&fx.project, "board.snxpcb");
    let out = run_cli(
        &["run", fx.project.to_str().unwrap(), "--dry-run"],
        jsonl(&[open(), fix(), check()]),
    );
    let data = &out.lines[1]["data"];
    assert_eq!(out.lines[1]["status"], "answered", "{}", out.stdout);
    assert_eq!(data["dry_run"], true, "{}", out.stdout);
    assert_eq!(data["changed"], false);
    assert_eq!(data["fixed"].as_array().map(Vec::len), Some(1));
    assert_eq!(clearance_findings(&out.lines[2]), 1, "{}", out.stdout);
}

#[test]
fn a_saved_fix_is_in_the_board_file() {
    let (_guard, root) = temp_root();
    let fx = pins_fixture(&root);
    let path = fx.dir.join("board.snxpcb");
    write_board(&path);
    set_board(&fx.project, "board.snxpcb");
    let save = json!({"id": "save_document", "args": {"document": "board.snxpcb"}});
    let out = run_cli(
        &["run", fx.project.to_str().unwrap(), "--save"],
        jsonl(&[open(), fix(), save]),
    );
    assert_eq!(out.lines[1]["status"], "answered", "{}", out.stdout);
    assert_eq!(out.lines[2]["status"], "ran", "{}", out.stdout);
    let text = std::fs::read_to_string(&path).expect("the board");
    let board = signex_types::format::SnxPcb::parse(&text)
        .expect("it reads")
        .board;
    let gnd = board
        .segments
        .iter()
        .find(|s| s.uuid == uuid::Uuid::from_u128(GND_TRACK))
        .expect("the GND track");
    // 0.05 mm short of 0.15 mm: 0.051 mm away from VCC.
    assert_eq!((gnd.start.y, gnd.end.y), (-0.051, -0.051));
}
