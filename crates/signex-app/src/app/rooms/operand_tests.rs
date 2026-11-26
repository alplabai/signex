//! #2457, #2941, #2935 — the rooms commands from their operands, as the CLI
//! runs them headless, and from the GUI, which runs the same handlers
//! (`translator::ran`). Each edit is one undo step and answers as
//! `docs/CLI.md`, "Rooms and placement", says.

use std::path::{Path, PathBuf};

use pcb_geom::ItemRef;
use serde_json::{Value, json};
use signex_types::pcb::{PcbBoard, Point};
use signex_types::schematic::{ChildSheet, SchematicSheet};
use uuid::Uuid;

use super::{CreateRoomsDialog, CreateRoomsMsg};
use crate::app::command::{CommandArgs, CommandOutcome, translator};
use crate::app::handlers::menu::export::settle;
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;
use crate::menu_bar::MenuMessage;

const SHEET_A: u128 = 0xA;
const SHEET_B: u128 = 0xB;

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

/// A part with one 2 x 2 mm pad on `net`, on the sheet placed as `sheet`.
fn part(n: u128, x: f64, y: f64, sheet: u128, net: u32) -> Value {
    json!({
        "uuid": id(n), "reference": format!("R{n}"),
        "position": {"x": x, "y": y}, "layer": "TopCopper",
        "pads": [{"uuid": id(n + 500), "number": "1", "pad_type": "smd", "shape": "circle",
                  "position": {"x": 0.0, "y": 0.0}, "size": {"x": 2.0, "y": 2.0},
                  "drill": null, "layers": ["TopCopper"],
                  "net": {"number": net, "name": format!("N{net}")}}],
        "schematic_link": {"sheet": [id(sheet)], "units": [id(100 + n)]}
    })
}

/// A 60 x 40 mm board: R1 and R2 on sheet A, R3 on sheet B below the board
/// where Update PCB stages new parts. R1 and R3 share a net.
fn board() -> PcbBoard {
    serde_json::from_value(json!({
        "uuid": id(1),
        "setup": {"grid_size": 0.5},
        "outline": [
            {"x": 0.0, "y": 0.0}, {"x": 60.0, "y": 0.0},
            {"x": 60.0, "y": 40.0}, {"x": 0.0, "y": 40.0}
        ],
        "layers": [{"id": 0, "name": "TopCopper", "layer_type": "signal"}],
        "footprints": [
            part(1, 10.0, 10.0, SHEET_A, 1),
            part(2, 14.0, 12.0, SHEET_A, 2),
            part(3, 5.0, 55.0, SHEET_B, 1),
        ]
    }))
    .expect("the fixture board")
}

fn save(path: PathBuf, sheet: SchematicSheet) {
    signex_engine::Engine::new_with_path(sheet, Some(path))
        .expect("engine")
        .save()
        .expect("sheet saved");
}

/// `main.snxsch` placing `dcdc.snxsch` as sheet A and `mcu.snxsch` as
/// sheet B, written to `dir`.
fn write_hierarchy(dir: &Path) {
    let child = |n: u128, file: &str| -> ChildSheet {
        serde_json::from_value(json!({
            "uuid": id(n), "name": file.trim_end_matches(".snxsch"), "filename": file,
            "position": {"x": 0.0, "y": 0.0}, "size": [20.0, 10.0]
        }))
        .expect("a child sheet")
    };
    save(
        dir.join("main.snxsch"),
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
        save(dir.join(file), sheet);
    }
}

fn app_with(dir: &Path) -> (Signex, PathBuf) {
    let mut app = app_workspace(&dir.to_string_lossy(), &["main.snxsch"]);
    app.ui_state.first_run_tour_open = false;
    app.document_state.projects[0].data.pcb_file = Some("board.snxpcb".into());
    let path = dir.join("board.snxpcb");
    app.open_pcb_tab(path.clone(), "board".into(), board());
    (app, path)
}

/// The command from `operands`, with `document` naming the board, as a
/// headless run of `signex-cli` runs it: its reads driven to the end.
fn headless(app: &mut Signex, path: &Path, command: &str, operands: &[&str]) -> CommandOutcome {
    let id = AppCommandId::new(command).expect("a catalog id");
    let document = format!("document={}", path.display());
    let mut texts = vec![document.as_str()];
    texts.extend_from_slice(operands);
    let args = CommandArgs::parse_operands(&id, &texts).expect("operands parse");
    app.run_headless_command_in_test(&id, args).0
}

fn answered(outcome: CommandOutcome) -> Value {
    match outcome {
        CommandOutcome::Answered(answer) => answer,
        other => panic!("not answered: {other:?}"),
    }
}

fn run_menu(app: &mut Signex, command: &str) {
    let task = app.update(Message::Menu(MenuMessage::command(command)));
    settle(app, task);
}

fn live(app: &Signex, path: &Path) -> PcbBoard {
    app.document_state.pcb_board(path).expect("board").clone()
}

fn undo_len(app: &Signex, path: &Path) -> usize {
    app.document_state
        .board_engine(path)
        .expect("engine")
        .undo_len()
}

fn names(list: &Value) -> Vec<String> {
    list.as_array()
        .expect("a list")
        .iter()
        .map(|v| v["name"].as_str().unwrap_or_default().to_string())
        .collect()
}

#[test]
fn place_board_room_from_operands_places_the_room_described_as_one_step() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, path) = app_with(dir.path());
    let steps = undo_len(&app, &path);
    let sheet = format!("sheet=/{}", id(SHEET_A));
    let answer = answered(headless(
        &mut app,
        &path,
        "place_board_room",
        &["start=5,5", "end=25,20", "name=dcdc", &sheet, "side=both"],
    ));
    let room = &answer["room"];
    assert_eq!(room["name"], "dcdc");
    assert_eq!(room["side"], "both");
    assert_eq!(room["sheet"], format!("/{}", id(SHEET_A)));
    assert_eq!(room["outline"]["shape"], "rectangle");
    assert_eq!(room["area_mm2"], 300.0);
    let parts: Vec<&str> = room["parts"]
        .as_array()
        .expect("parts")
        .iter()
        .map(|p| p["designator"].as_str().unwrap_or_default())
        .collect();
    assert_eq!(parts, ["R1", "R2"]);
    assert_eq!(undo_len(&app, &path), steps + 1, "one undo step");
    assert_eq!(live(&app, &path).rooms.len(), 1);

    let polygon = answered(headless(
        &mut app,
        &path,
        "place_board_room",
        &["corners=30,0;40,0;35,10"],
    ));
    assert_eq!(polygon["room"]["name"], "Room 1");
    assert_eq!(polygon["room"]["outline"]["shape"], "polygon");
    assert_eq!(polygon["room"]["area_mm2"], 50.0);
}

#[test]
fn place_board_room_refuses_an_outline_side_sheet_or_name_it_cannot_take() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, path) = app_with(dir.path());
    for operands in [
        vec!["start=5,5"],
        vec!["start=5,5", "end=5,9"],
        vec!["corners=0,0;1,1"],
        vec!["start=5,5", "end=9,9", "side=left"],
        vec!["start=5,5", "end=9,9", "sheet=dcdc"],
    ] {
        let outcome = headless(&mut app, &path, "place_board_room", &operands);
        assert!(
            matches!(outcome, CommandOutcome::Disabled { .. }),
            "{operands:?}: {outcome:?}"
        );
    }
    let named = ["start=0,0", "end=9,9", "name=a"];
    headless(&mut app, &path, "place_board_room", &named);
    let twice = headless(&mut app, &path, "place_board_room", &named);
    assert!(
        matches!(twice, CommandOutcome::Disabled { .. }),
        "{twice:?}"
    );
    let rooms = live(&app, &path).rooms.len();
    assert_eq!(rooms, 1, "nothing refused was placed");
}

/// Place ▸ Room's second click runs the operand handler.
#[test]
fn the_place_room_tool_runs_the_operand_handler() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, path) = app_with(dir.path());
    run_menu(&mut app, "place_board_room");
    assert!(app.board_tool_clicked(Point::new(5.0, 5.0)));
    assert!(app.board_tool_clicked(Point::new(25.0, 20.0)));
    assert!(translator::ran("place_board_room"));
    assert_eq!(live(&app, &path).rooms[0].name, "Room 1");
}

#[test]
fn create_rooms_from_sheets_runs_headless_on_the_sheets_it_reads() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_hierarchy(dir.path());
    let (mut app, path) = app_with(dir.path());
    let steps = undo_len(&app, &path);
    let answer = answered(headless(
        &mut app,
        &path,
        "create_rooms_from_sheets",
        &["margin=1", "side=top"],
    ));
    assert_eq!(names(&answer["rooms"]), ["dcdc", "mcu"]);
    assert_eq!(answer["rooms"][0]["margin_mm"], 1.0);
    assert_eq!(answer["rooms"][0]["keep_parts_inside"], true);
    let moved: Vec<&str> = answer["moved"]
        .as_array()
        .expect("moved")
        .iter()
        .map(|m| m["designator"].as_str().unwrap_or_default())
        .collect();
    assert!(moved.contains(&"R3"), "the staged part moves: {moved:?}");
    assert_eq!(undo_len(&app, &path), steps + 1, "one undo step");
    run_menu(&mut app, "undo");
    assert!(live(&app, &path).rooms.is_empty());

    let only = answered(headless(
        &mut app,
        &path,
        "create_rooms_from_sheets",
        &["rooms=mcu"],
    ));
    assert_eq!(names(&only["rooms"]), ["mcu"]);
}

/// The dialog's Create runs the operand handler on the sheets it read.
#[test]
fn the_dialogs_create_runs_the_operand_handler() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, path) = app_with(dir.path());
    let at = |n: u128| signex_types::schematic::InstancePath::from_chain(vec![id(n)]);
    let sheets = vec![signex_engine::pcb::rooms::SheetMembers {
        name: "dcdc".into(),
        file: "dcdc.snxsch".into(),
        placements: vec![at(SHEET_A)],
    }];
    let dialog = CreateRoomsDialog::new(path.clone(), "ProjectA.snxprj".into(), sheets);
    app.ui_state.rooms.create = Some(Box::new(dialog));
    let task = app.update(Message::from(CreateRoomsMsg::Confirmed));
    settle(&mut app, task);
    assert!(translator::ran("create_rooms_from_sheets"));
    assert_eq!(live(&app, &path).rooms[0].name, "dcdc");
    assert!(app.ui_state.rooms.create.is_none());
}

#[test]
fn arrange_in_room_packs_the_rooms_parts_and_answers_each_move() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, path) = app_with(dir.path());
    let sheet = format!("sheet=/{}", id(SHEET_A));
    headless(
        &mut app,
        &path,
        "place_board_room",
        &["start=30,5", "end=55,30", "name=dcdc", &sheet],
    );
    let steps = undo_len(&app, &path);
    let answer = answered(headless(&mut app, &path, "arrange_in_room", &["room=dcdc"]));
    assert_eq!(answer["room"], "dcdc");
    let moved = answer["moved"].as_array().expect("moved");
    assert_eq!(moved.len(), 2, "{answer}");
    let inside = |m: &Value| m["to"][0].as_f64().unwrap_or(0.0) > 30.0;
    assert!(moved.iter().all(inside));
    assert_eq!(answer["outside"], json!([]));
    assert_eq!(undo_len(&app, &path), steps + 1, "one undo step");
    let unknown = headless(&mut app, &path, "arrange_in_room", &["room=nowhere"]);
    assert!(matches!(unknown, CommandOutcome::Disabled { .. }));
}

/// Arrange in Room from the palette acts on the room Properties shows,
/// through the operand handler.
#[test]
fn arrange_in_room_from_the_gui_acts_on_the_selected_room() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, path) = app_with(dir.path());
    let sheet = format!("sheet=/{}", id(SHEET_A));
    let placed = answered(headless(
        &mut app,
        &path,
        "place_board_room",
        &["start=30,5", "end=55,30", &sheet],
    ));
    let room = Uuid::parse_str(placed["room"]["id"].as_str().expect("id")).expect("uuid");
    app.select_pcb_items(&[ItemRef::Room(room)]);
    let steps = undo_len(&app, &path);
    run_menu(&mut app, "arrange_in_room");
    assert!(translator::ran("arrange_in_room"));
    assert_eq!(undo_len(&app, &path), steps + 1);
    let r1 = live(&app, &path).footprints[0].position;
    assert!(r1.x > 30.0, "R1 moved into the room: {r1:?}");
}

#[test]
fn auto_place_answers_each_move_and_the_scores_and_a_dry_run_changes_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, path) = app_with(dir.path());
    let sheet = format!("sheet=/{}", id(SHEET_A));
    headless(
        &mut app,
        &path,
        "place_board_room",
        &["start=30,5", "end=55,30", "name=dcdc", &sheet],
    );
    let before = serde_json::to_value(live(&app, &path)).expect("json");
    let steps = undo_len(&app, &path);
    let dry = answered(headless(
        &mut app,
        &path,
        "auto_place",
        &["weights=connection:3,edges:0", "dry_run=true"],
    ));
    assert_eq!(dry["dry_run"], true);
    assert_eq!(dry["weights"]["connection"], 3.0);
    assert_eq!(dry["weights"]["edges"], 0.0);
    assert_eq!(dry["rooms"], json!(["dcdc"]));
    let after = serde_json::to_value(live(&app, &path)).expect("json");
    assert_eq!(after, before, "a dry run changes nothing");
    assert_eq!(undo_len(&app, &path), steps);

    let run = answered(headless(
        &mut app,
        &path,
        "auto_place",
        &["weights=connection:3,edges:0"],
    ));
    assert_eq!(run["moved"], dry["moved"], "placed as the dry run said");
    assert_eq!(run["moved"].as_array().map(Vec::len), Some(2));
    assert!(run["score"]["before"]["connection"].is_number());
    assert!(run["score"]["after"]["edges"].is_number());
    assert_eq!(undo_len(&app, &path), steps + 1, "one undo step");
    for bad in ["weights=speed:1", "rooms=nowhere"] {
        let refused = headless(&mut app, &path, "auto_place", &[bad]);
        assert!(matches!(refused, CommandOutcome::Disabled { .. }), "{bad}");
    }
}

#[test]
fn auto_place_from_the_gui_runs_the_operand_handler() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, path) = app_with(dir.path());
    let sheet = format!("sheet=/{}", id(SHEET_A));
    headless(
        &mut app,
        &path,
        "place_board_room",
        &["start=30,5", "end=55,30", &sheet],
    );
    let steps = undo_len(&app, &path);
    run_menu(&mut app, "auto_place");
    assert!(translator::ran("auto_place"));
    assert_eq!(undo_len(&app, &path), steps + 1);
    run_menu(&mut app, "undo");
    assert_eq!(undo_len(&app, &path), steps, "one undo takes it back");
}
