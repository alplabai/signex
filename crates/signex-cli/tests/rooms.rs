//! End to end (#2457, #2941, #2935): the rooms commands through the real
//! `signex-cli` binary, headless. Create Rooms from Sheets reads the
//! project's sheets and makes one room per sheet placement, a room is
//! placed from its corners, Arrange in Room packs a room's parts and Auto
//! Place places them; each answers with JSON, needs only `edit` and writes
//! nothing without `--save`.

mod support;

use std::path::Path;

use serde_json::{Value, json};
use signex_types::pcb::{Footprint, Point};
use signex_types::schematic::{ChildSheet, SchematicSheet};
use support::{jsonl, pins_fixture, run_cli, snapshot, temp_root, without_journals};
use uuid::Uuid;

const SHEET_A: u128 = 0xA;
const SHEET_B: u128 = 0xB;

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

/// Replace the fixture's root sheet with one placing `dcdc.snxsch` (sheet
/// A) and `mcu.snxsch` (sheet B), and write both.
fn write_hierarchy(dir: &Path) {
    let child = |n: u128, file: &str| -> ChildSheet {
        serde_json::from_value(json!({
            "uuid": id(n), "name": file.trim_end_matches(".snxsch"), "filename": file,
            "position": {"x": 0.0, "y": 0.0}, "size": [20.0, 10.0]
        }))
        .expect("a child sheet")
    };
    let save = |file: &str, sheet: SchematicSheet| {
        signex_engine::Engine::new_with_path(sheet, Some(dir.join(file)))
            .expect("engine")
            .save()
            .expect("sheet saved");
    };
    save(
        "main.snxsch",
        SchematicSheet {
            uuid: id(0x100),
            child_sheets: vec![child(SHEET_A, "dcdc.snxsch"), child(SHEET_B, "mcu.snxsch")],
            ..Default::default()
        },
    );
    for (n, file) in [(0x101, "dcdc.snxsch"), (0x102, "mcu.snxsch")] {
        let sheet = SchematicSheet {
            uuid: id(n),
            ..Default::default()
        };
        save(file, sheet);
    }
}

fn part(n: u128, x: f64, y: f64, sheet: u128) -> Footprint {
    serde_json::from_value(json!({
        "uuid": id(n), "reference": format!("R{n}"),
        "position": {"x": x, "y": y}, "layer": "TopCopper",
        "pads": [{"uuid": id(n + 500), "number": "1", "pad_type": "smd", "shape": "circle",
                  "position": {"x": 0.0, "y": 0.0}, "size": {"x": 2.0, "y": 2.0},
                  "drill": null, "layers": ["TopCopper"], "net": null}],
        "schematic_link": {"sheet": [id(sheet)], "units": [id(100 + n)]}
    }))
    .expect("a footprint")
}

/// A 60 x 40 mm board: R1 and R2 on sheet A, R3 on sheet B below it.
fn write_board(path: &Path) {
    let mut board = signex_types::project::SetupProfile::Standard
        .pcb(2)
        .new_board();
    board.outline = [(0.0, 0.0), (60.0, 0.0), (60.0, 40.0), (0.0, 40.0)]
        .map(|(x, y)| Point::new(x, y))
        .to_vec();
    board.footprints = vec![
        part(1, 10.0, 10.0, SHEET_A),
        part(2, 14.0, 12.0, SHEET_A),
        part(3, 5.0, 55.0, SHEET_B),
    ];
    let text = signex_types::format::SnxPcb::new(board)
        .write_string()
        .expect("encode");
    std::fs::write(path, text).expect("write board");
}

fn set_board(project: &Path, filename: &str) {
    let mut data = signex_types::project::parse_project(project).expect("project");
    data.pcb_file = Some(filename.into());
    signex_types::project::write_project(project, &data).expect("project");
}

fn command(id: &str, args: Value) -> Value {
    let mut args = args;
    args["document"] = json!("board.snxpcb");
    json!({"id": id, "args": args})
}

fn room_names(data: &Value) -> Vec<String> {
    data["rooms"]
        .as_array()
        .expect("rooms")
        .iter()
        .map(|r| r["name"].as_str().unwrap_or_default().to_string())
        .collect()
}

#[test]
fn the_rooms_commands_run_headless_and_answer_with_json() {
    let (_guard, root) = temp_root();
    let fx = pins_fixture(&root);
    write_hierarchy(&fx.dir);
    write_board(&fx.dir.join("board.snxpcb"));
    set_board(&fx.project, "board.snxpcb");
    let before = without_journals(snapshot(&root));
    let out = run_cli(
        &["run", fx.project.to_str().unwrap(), "--headless"],
        jsonl(&[
            json!({"id": "open_document", "args": {"path": "board.snxpcb"}}),
            command("create_rooms_from_sheets", json!({"margin": 1.0})),
            command(
                "place_board_room",
                json!({"start": [40, 2], "end": [58, 12], "name": "spare"}),
            ),
            command("arrange_in_room", json!({"room": "mcu"})),
            command(
                "auto_place",
                json!({"weights": "connection:2,edges:1", "dry_run": true}),
            ),
        ]),
    );
    for line in &out.lines[1..] {
        assert_eq!(line["status"], "answered", "{}", out.stdout);
    }
    let created = &out.lines[1]["data"];
    assert_eq!(room_names(created), ["dcdc", "mcu"], "{}", out.stdout);
    assert_eq!(created["rooms"][0]["margin_mm"], 1.0);
    let placed = &out.lines[2]["data"]["room"];
    assert_eq!(placed["name"], "spare");
    assert_eq!(placed["outline"]["shape"], "rectangle");
    assert_eq!(out.lines[3]["data"]["room"], "mcu");
    let dry = &out.lines[4]["data"];
    assert_eq!(dry["dry_run"], true);
    assert_eq!(dry["rooms"], json!(["dcdc", "mcu", "spare"]));
    assert!(dry["score"]["before"]["connection"].is_number());
    assert_eq!(without_journals(snapshot(&root)), before, "nothing written");
}
