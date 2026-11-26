//! End to end (#2936): a board footprint moved, turned and flipped by id
//! through the real `signex-cli` binary, each answered with the footprint as
//! it then is, and a locked footprint refused with the reason.

mod support;

use std::path::Path;

use serde_json::{Value, json};
use signex_types::pcb::PcbBoard;
use support::{jsonl, pins_fixture, run_cli, snapshot, temp_root, without_journals};

const R1: &str = "00000000-0000-0000-0000-000000000007";
const U2: &str = "00000000-0000-0000-0000-000000000008";

/// R1 at (10, 10) and U2, locked, at (30, 10), each with one pad.
fn write_board(path: &Path) {
    let part = |id: &str, name: &str, x: f64, locked: bool| {
        json!({
            "uuid": id, "reference": name, "position": {"x": x, "y": 10.0},
            "layer": "TopCopper", "locked": locked,
            "pads": [{
                "uuid": uuid::Uuid::now_v7(), "number": "1", "pad_type": "smd",
                "shape": "rect", "position": {"x": 1.0, "y": 0.0}, "size": {"x": 1.0, "y": 0.5},
                "drill": null, "layers": ["TopCopper"]
            }]
        })
    };
    let board: PcbBoard = serde_json::from_value(json!({
        "uuid": uuid::Uuid::now_v7(),
        "footprints": [part(R1, "R1", 10.0, false), part(U2, "U2", 30.0, true)]
    }))
    .expect("a board");
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

fn edit(id: &str, args: Value) -> Value {
    let mut args = args;
    args["document"] = json!("board.snxpcb");
    json!({"id": id, "args": args})
}

#[test]
fn a_footprint_is_moved_turned_and_flipped_by_id_and_a_locked_one_is_refused() {
    let (_guard, root) = temp_root();
    let fx = pins_fixture(&root);
    write_board(&fx.dir.join("board.snxpcb"));
    set_board(&fx.project, "board.snxpcb");
    let before = without_journals(snapshot(&root));
    let out = run_cli(
        &["run", fx.project.to_str().unwrap()],
        jsonl(&[
            json!({"id": "open_document", "args": {"path": "board.snxpcb"}}),
            edit("set_footprint_position", json!({"id": R1, "at": "20,-15"})),
            edit(
                "set_footprint_rotation",
                json!({"designator": "R1", "rotation": 90}),
            ),
            edit("set_footprint_side", json!({"id": R1, "side": "bottom"})),
            edit("set_footprint_position", json!({"id": U2, "at": "0,0"})),
        ]),
    );
    let moved = &out.lines[1];
    assert_eq!(moved["status"], "answered", "{}", out.stdout);
    assert_eq!(moved["data"]["id"], R1, "{}", out.stdout);
    assert_eq!(moved["data"]["designator"], "R1", "{}", out.stdout);
    assert_eq!(
        moved["data"]["position"],
        json!({"x": 20.0, "y": -15.0}),
        "{}",
        out.stdout
    );
    assert_eq!(moved["data"]["changed"], true, "{}", out.stdout);
    let turned = &out.lines[2];
    assert_eq!(turned["status"], "answered", "{}", out.stdout);
    assert_eq!(turned["data"]["rotation"], 90.0, "{}", out.stdout);
    let flipped = &out.lines[3];
    assert_eq!(flipped["status"], "answered", "{}", out.stdout);
    assert_eq!(flipped["data"]["side"], "bottom", "{}", out.stdout);
    assert_eq!(
        flipped["data"]["position"], moved["data"]["position"],
        "a flip keeps the place: {}",
        out.stdout
    );
    let locked = &out.lines[4];
    assert_eq!(locked["status"], "refused", "{}", out.stdout);
    let reason = locked["reason"].as_str().unwrap_or_default();
    assert!(reason.contains("U2 is locked"), "{reason}");
    assert_eq!(
        without_journals(snapshot(&root)),
        before,
        "nothing written without --save"
    );
}
