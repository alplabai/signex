//! End to end (#2470): a board routed net by net through the real
//! `signex-cli` binary, headless. A route between two pads goes around a pad
//! of another net and answers with its tracks; a via lands where it is
//! named; both are kept by `save_document`; `--dry-run` routes without
//! changing or writing anything.

mod support;

use std::path::Path;

use serde_json::{Value, json};
use support::{jsonl, pins_fixture, run_cli, snapshot, temp_root, without_journals};
use uuid::Uuid;

/// A footprint `P<n>` holding one round 1 mm top pad at (`x`, `y`).
fn pad(n: u128, x: f64, y: f64, net: (u32, &str)) -> Value {
    json!({
        "uuid": Uuid::from_u128(0x100 + n), "reference": format!("P{n}"), "value": "",
        "footprint_id": "Pad", "layer": "TopCopper", "rotation": 0.0,
        "position": {"x": x, "y": y},
        "pads": [{
            "uuid": Uuid::from_u128(0x200 + n), "number": "1", "pad_type": "smd",
            "shape": "circle", "position": {"x": 0.0, "y": 0.0}, "size": {"x": 1.0, "y": 1.0},
            "drill": null, "layers": ["TopCopper"], "net": {"number": net.0, "name": net.1}
        }]
    })
}

/// Net A's pads P1 at (0, 0) and P2 at (10, 0), net B's pad P3 between
/// them; a 0.5 mm grid, 0.25 mm tracks and a 0.2 mm clearance.
fn write_board(path: &Path) {
    let board: signex_types::pcb::PcbBoard = serde_json::from_value(json!({
        "uuid": Uuid::from_u128(1),
        "setup": {"grid_size": 0.5, "trace_width": 0.25, "clearance": 0.2},
        "nets": [{"number": 1, "name": "A"}, {"number": 2, "name": "B"}],
        "footprints": [
            pad(1, 0.0, 0.0, (1, "A")),
            pad(2, 10.0, 0.0, (1, "A")),
            pad(3, 5.0, 0.0, (2, "B"))
        ]
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

fn open_board() -> Value {
    json!({"id": "open_document", "args": {"path": "board.snxpcb"}})
}

fn route_a() -> Value {
    json!({"id": "place_board_route", "args": {
        "document": "board.snxpcb", "start": "P1.1", "end": "P2.1"
    }})
}

#[test]
fn a_route_and_a_via_from_the_command_line_answer_what_they_made_and_save() {
    let (_guard, root) = temp_root();
    let fx = pins_fixture(&root);
    write_board(&fx.dir.join("board.snxpcb"));
    set_board(&fx.project, "board.snxpcb");
    let project = fx.project.to_str().unwrap();
    let out = run_cli(
        &["run", project, "--save"],
        jsonl(&[
            open_board(),
            route_a(),
            json!({"id": "place_board_via", "args": {
                "document": "board.snxpcb", "at": [20, 20], "net": "A"
            }}),
            json!({"id": "save_document", "args": {"document": "board.snxpcb"}}),
        ]),
    );
    assert_eq!(out.code, Some(0), "{}\n{}", out.stdout, out.stderr);
    let route = &out.lines[1];
    assert_eq!(route["status"], "answered", "{}", out.stdout);
    assert_eq!(route["data"]["dry_run"], false);
    assert_eq!(route["data"]["nets"], json!(["A"]));
    assert_eq!(route["data"]["layer"], "TopCopper");
    let tracks = route["data"]["tracks"].as_array().expect("tracks");
    assert!(tracks.len() >= 3, "around P3, not through: {}", out.stdout);
    assert_eq!(tracks[0]["start"], json!([0.0, 0.0]), "{}", out.stdout);
    let via = &out.lines[2];
    assert_eq!(via["status"], "answered", "{}", out.stdout);
    assert_eq!(via["data"]["vias"][0]["at"], json!([20.0, 20.0]));
    assert_eq!(via["data"]["vias"][0]["net"], "A");

    // The saved board holds the route and the via.
    let again = run_cli(
        &["run", project],
        jsonl(&[
            open_board(),
            json!({"id": "query_board_nets", "args": {"document": "board.snxpcb", "net": "A"}}),
        ]),
    );
    let net = &again.lines[1]["data"]["nets"][0];
    assert_eq!(
        net["tracks"].as_u64(),
        Some(tracks.len() as u64),
        "{}",
        again.stdout
    );
    assert_eq!(net["vias"], 1, "{}", again.stdout);
}

#[test]
fn the_dry_run_flag_routes_without_changing_or_writing_anything() {
    let (_guard, root) = temp_root();
    let fx = pins_fixture(&root);
    write_board(&fx.dir.join("board.snxpcb"));
    set_board(&fx.project, "board.snxpcb");
    let before = without_journals(snapshot(&root));
    let out = run_cli(
        &["run", fx.project.to_str().unwrap(), "--dry-run"],
        jsonl(&[open_board(), route_a()]),
    );
    let route = &out.lines[1];
    assert_eq!(route["status"], "answered", "{}", out.stdout);
    assert_eq!(route["data"]["dry_run"], true);
    assert!(
        route["data"]["tracks"]
            .as_array()
            .is_some_and(|t| !t.is_empty()),
        "{}",
        out.stdout
    );
    assert_eq!(route["changed"], json!([]), "{}", out.stdout);
    assert_eq!(without_journals(snapshot(&root)), before, "nothing written");
}

#[test]
fn a_route_the_board_cannot_take_is_refused_with_the_reason() {
    let (_guard, root) = temp_root();
    let fx = pins_fixture(&root);
    write_board(&fx.dir.join("board.snxpcb"));
    set_board(&fx.project, "board.snxpcb");
    let out = run_cli(
        &["run", fx.project.to_str().unwrap(), "--keep-going"],
        jsonl(&[
            open_board(),
            json!({"id": "place_board_track", "args": {
                "document": "board.snxpcb", "start": "P1.1", "end": "P3.1"
            }}),
        ]),
    );
    assert_eq!(out.code, Some(1), "{}", out.stdout);
    let refused = &out.lines[1];
    assert_eq!(refused["status"], "refused", "{}", out.stdout);
    let reason = refused["reason"].as_str().unwrap_or_default();
    assert!(reason.contains("P3.1 is on net B"), "{reason}");
}
