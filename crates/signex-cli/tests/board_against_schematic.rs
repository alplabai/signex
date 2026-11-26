//! End to end (#2462, #2460): through the real `signex-cli` binary,
//! `check_board_against_schematic` refuses a board that is not open, then
//! answers every difference of the opened board with an id, `board_status`
//! counts the same differences, and neither writes a file.

mod support;

use std::path::Path;

use serde_json::json;
use support::{jsonl, pins_fixture, run_cli, snapshot, temp_root, without_journals};

/// A board on the Standard setup holding only TP3, linked to a part the
/// design does not have: it is extra on the board.
fn write_board(path: &Path) {
    let mut board = signex_types::project::SetupProfile::Standard
        .pcb(2)
        .new_board();
    let mut tp3: signex_types::pcb::Footprint = serde_json::from_value(json!({
        "uuid": uuid::Uuid::from_u128(0xf3), "reference": "TP3", "value": "TestPoint",
        "position": {"x": 0.0, "y": 0.0}, "layer": "TopCopper"
    }))
    .expect("a footprint");
    tp3.schematic_link = Some(signex_types::pcb::SymbolLink::new(
        signex_types::schematic::InstancePath::root(),
        [uuid::Uuid::from_u128(0x999)],
    ));
    board.footprints.push(tp3);
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

#[test]
fn the_check_answers_each_difference_and_board_status_counts_them() {
    let (_guard, root) = temp_root();
    let fx = pins_fixture(&root);
    write_board(&fx.dir.join("board.snxpcb"));
    set_board(&fx.project, "board.snxpcb");
    let before = without_journals(snapshot(&root));
    let board = json!({"document": "board.snxpcb"});
    let out = run_cli(
        &["run", fx.project.to_str().unwrap(), "--keep-going"],
        jsonl(&[
            json!({"id": "check_board_against_schematic", "args": board}),
            json!({"id": "open_document", "args": {"path": "board.snxpcb"}}),
            json!({"id": "check_board_against_schematic", "args": board}),
            json!({"id": "board_status", "args": board}),
        ]),
    );

    let closed = &out.lines[0];
    assert_eq!(closed["status"], "refused", "{}", out.stdout);
    let reason = closed["reason"].as_str().unwrap_or_default();
    assert!(reason.contains("is not open"), "{reason}");

    let check = &out.lines[2];
    assert_eq!(check["status"], "answered", "{}", out.stdout);
    let data = &check["data"];
    assert_eq!(data["matches"], false, "{}", out.stdout);
    let differences = data["differences"].as_array().expect("differences");
    assert_eq!(data["count"], differences.len(), "{}", out.stdout);
    let extra = differences
        .iter()
        .find(|row| row["kind"] == "extra_on_board")
        .expect("TP3 is extra on the board");
    assert_eq!(extra["object"]["designator"], "TP3", "{}", out.stdout);
    assert_eq!(extra["default"], "skip", "{}", out.stdout);
    for row in differences {
        let id = row["id"].as_str().expect("an id");
        assert_eq!(id.len(), 16, "{id}");
    }

    let status = &out.lines[3];
    assert_eq!(status["status"], "answered", "{}", out.stdout);
    assert_eq!(status["data"]["parts"]["total"], 1, "{}", out.stdout);
    assert_eq!(status["data"]["schematic"]["matches"], false);
    assert_eq!(
        status["data"]["schematic"]["differences"], data["count"],
        "{}",
        out.stdout
    );

    assert_eq!(without_journals(snapshot(&root)), before, "nothing written");
}
