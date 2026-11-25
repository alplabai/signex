//! End to end (#2470): the area tools through the real `signex-cli` binary,
//! without a window. A pour and a keepout named by their board, layer,
//! corners and net answer with the area as JSON (the pour with its fill),
//! a layer the area cannot take is refused with the reason, and the areas
//! reach the file only through `save_document`.

mod support;

use std::path::Path;

use serde_json::json;
use signex_types::pcb::NetDef;
use support::{jsonl, pins_fixture, run_cli, snapshot, temp_root, without_journals};

/// A two-layer board with the nets GND and VCC, listed as the project's
/// board.
fn write_board(dir: &Path, project: &Path) {
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
    let text = signex_types::format::SnxPcb::new(board)
        .write_string()
        .expect("encode");
    std::fs::write(dir.join("board.snxpcb"), text).expect("write board");
    let mut data = signex_types::project::parse_project(project).expect("project");
    data.pcb_file = Some("board.snxpcb".into());
    signex_types::project::write_project(project, &data).expect("project");
}

fn areas() -> String {
    jsonl(&[
        json!({"id": "open_document", "args": {"path": "board.snxpcb"}}),
        json!({"id": "place_board_polygon_pour", "args": {
            "document": "board.snxpcb", "layer": "TopCopper",
            "corners": "1,1;9,1;9,9;1,9", "net": "GND"}}),
        json!({"id": "place_board_keepout", "args": {
            "document": "board.snxpcb", "layer": "TopKeepout",
            "corners": "20,20;24,20;24,24"}}),
        json!({"id": "place_board_region", "args": {
            "document": "board.snxpcb", "layer": "TopSilkscreen",
            "corners": "1,1;2,1;2,2"}}),
    ])
}

#[test]
fn the_area_tools_answer_with_their_areas_and_write_nothing_unsaved() {
    let (_guard, root) = temp_root();
    let fx = pins_fixture(&root);
    write_board(&fx.dir, &fx.project);
    let before = without_journals(snapshot(&root));
    let out = run_cli(&["run", fx.project.to_str().unwrap()], areas());

    assert_ne!(out.lines[0]["status"], "refused", "{}", out.stdout);
    let pour = &out.lines[1];
    assert_eq!(pour["status"], "answered", "{}", out.stdout);
    assert_eq!(pour["data"]["kind"], "pour", "{}", out.stdout);
    assert_eq!(pour["data"]["layer"], "TopCopper", "{}", out.stdout);
    assert_eq!(pour["data"]["net_name"], "GND", "{}", out.stdout);
    assert_eq!(
        pour["data"]["fill"]["zone"], pour["data"]["id"],
        "{}",
        out.stdout
    );
    assert!(pour["data"]["fill"]["pieces"].is_u64(), "{}", out.stdout);

    let keepout = &out.lines[2];
    assert_eq!(keepout["status"], "answered", "{}", out.stdout);
    assert_eq!(keepout["data"]["kind"], "keepout", "{}", out.stdout);
    assert_eq!(keepout["data"]["net"], 0, "{}", out.stdout);

    let region = &out.lines[3];
    assert_eq!(region["status"], "refused", "{}", out.stdout);
    let reason = region["reason"].as_str().unwrap_or_default();
    assert!(reason.contains("copper layer"), "{reason}");

    assert_eq!(without_journals(snapshot(&root)), before, "nothing written");
}

#[test]
fn saved_areas_read_back_from_the_board_file() {
    let (_guard, root) = temp_root();
    let fx = pins_fixture(&root);
    write_board(&fx.dir, &fx.project);
    let mut input = areas();
    input.push_str(&jsonl(&[json!({
        "id": "save_document", "args": {"document": "board.snxpcb"}
    })]));
    let out = run_cli(&["run", fx.project.to_str().unwrap(), "--save"], input);

    assert_eq!(
        out.lines[4]["status"], "ran",
        "{}\n{}",
        out.stdout, out.stderr
    );
    let text = std::fs::read_to_string(fx.dir.join("board.snxpcb")).expect("the saved board");
    let board = signex_types::format::SnxPcb::parse(&text)
        .expect("reads back")
        .board;
    assert_eq!(board.zones.len(), 2, "the pour and the keepout");
    assert!(board.zones[0].is_pour());
    assert_eq!(board.zones[0].net_name, "GND");
    assert_eq!(board.zones[1].layer, "TopKeepout");
}
