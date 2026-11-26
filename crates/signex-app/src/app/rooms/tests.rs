//! #2457 — rooms through `Signex::update`: Place ▸ Room, Room Properties
//! edits and their undo, and Create Rooms from Sheets as one undo step.

use std::path::{Path, PathBuf};

use pcb_geom::ItemRef;
use signex_engine::pcb::rooms::SheetMembers;
use signex_types::pcb::{PcbBoard, Point};
use signex_types::schematic::InstancePath;
use uuid::Uuid;

use super::{CreateRoomsDialog, CreateRoomsMsg, RoomField, RoomMsg};
use crate::app::handlers::menu::export::settle;
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::pcb_tools::BoardPlaceTool;
use crate::app::{Message, Signex};
use crate::dock::DockMessage;
use crate::menu_bar::MenuMessage;
use crate::panels::PanelMsg;

const SHEET_A: u128 = 0xA;
const SHEET_B: u128 = 0xB;

fn part(n: u128, x: f64, y: f64, sheet: u128) -> serde_json::Value {
    serde_json::json!({
        "uuid": Uuid::from_u128(n), "reference": format!("R{n}"),
        "position": {"x": x, "y": y}, "layer": "TopCopper",
        "schematic_link": {"sheet": [Uuid::from_u128(sheet)], "units": [Uuid::from_u128(100 + n)]}
    })
}

/// A 60 x 40 mm board: R1 and R2 on sheet A, R3 on sheet B below the board
/// where Update PCB stages new parts.
fn board() -> PcbBoard {
    serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(1),
        "setup": {"grid_size": 0.5},
        "outline": [
            {"x": 0.0, "y": 0.0}, {"x": 60.0, "y": 0.0},
            {"x": 60.0, "y": 40.0}, {"x": 0.0, "y": 40.0}
        ],
        "layers": [{"id": 0, "name": "TopCopper", "layer_type": "signal"}],
        "footprints": [
            part(1, 10.0, 10.0, SHEET_A),
            part(2, 14.0, 12.0, SHEET_A),
            part(3, 5.0, 55.0, SHEET_B),
        ]
    }))
    .expect("the fixture board")
}

fn app_with(dir: &Path) -> (Signex, PathBuf) {
    let mut app = app_workspace(&dir.to_string_lossy(), &[]);
    app.ui_state.first_run_tour_open = false;
    app.ui_state.unit = signex_types::coord::Unit::Mm;
    app.document_state.projects[0].data.pcb_file = Some("board.snxpcb".into());
    let path = dir.join("board.snxpcb");
    app.open_pcb_tab(path.clone(), "board".into(), board());
    (app, path)
}

fn run(app: &mut Signex, id: &str) {
    let task = app.update(Message::Menu(MenuMessage::command(id)));
    settle(app, task);
}

fn room_msg(app: &mut Signex, msg: RoomMsg) {
    let task = app.update(Message::Dock(DockMessage::Panel(PanelMsg::Room(msg))));
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

/// Place ▸ Room: two corners draw `Room 1` as one step, the tool ends and
/// the room is selected, so Properties shows it.
#[test]
fn place_room_draws_and_selects_the_room() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, path) = app_with(dir.path());
    run(&mut app, "place_board_room");
    assert_eq!(
        app.own_board_tool().map(|s| s.tool),
        Some(BoardPlaceTool::Room)
    );
    let before = undo_len(&app, &path);
    assert!(app.board_tool_clicked(Point::new(5.0, 5.0)));
    assert!(
        live(&app, &path).rooms.is_empty(),
        "one corner places nothing"
    );
    assert!(app.board_tool_clicked(Point::new(25.0, 20.0)));
    let board = live(&app, &path);
    assert_eq!(board.rooms.len(), 1);
    assert_eq!(board.rooms[0].name, "Room 1");
    assert_eq!(board.rooms[0].colour, "#2CA8B8");
    assert_eq!(undo_len(&app, &path), before + 1);
    assert!(app.own_board_tool().is_none(), "the tool ends");
    let id = board.rooms[0].uuid;
    assert!(
        app.interaction_state
            .pcb_selection
            .selection()
            .items()
            .any(|i| i == ItemRef::Room(id))
    );
    let summary = app
        .document_view
        .panel_ctx
        .pcb_board
        .as_ref()
        .expect("summary");
    assert_eq!(summary.room.as_ref().map(|r| r.room.uuid), Some(id));
}

/// Each Room Properties edit is one undo step; X moves the room's parts.
#[test]
fn room_properties_edits_are_undone_one_by_one() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, path) = app_with(dir.path());
    run(&mut app, "place_board_room");
    app.board_tool_clicked(Point::new(5.0, 5.0));
    app.board_tool_clicked(Point::new(25.0, 20.0));
    let room = live(&app, &path).rooms[0].uuid;
    let steps = undo_len(&app, &path);
    room_msg(&mut app, RoomMsg::KeepToggled { room, keep: true });
    assert!(live(&app, &path).rooms[0].keep_parts_inside);
    room_msg(
        &mut app,
        RoomMsg::FieldEdited {
            room,
            field: RoomField::Name,
            text: "dcdc".into(),
        },
    );
    room_msg(
        &mut app,
        RoomMsg::FieldSubmitted {
            room,
            field: RoomField::Name,
        },
    );
    assert_eq!(live(&app, &path).rooms[0].name, "dcdc");
    assert_eq!(undo_len(&app, &path), steps + 2);
    run(&mut app, "undo");
    assert_eq!(live(&app, &path).rooms[0].name, "Room 1");
    assert!(live(&app, &path).rooms[0].keep_parts_inside);
    run(&mut app, "undo");
    assert!(!live(&app, &path).rooms[0].keep_parts_inside);
}

fn sheets() -> Vec<SheetMembers> {
    let at = |n: u128| InstancePath::from_chain(vec![Uuid::from_u128(n)]);
    vec![
        SheetMembers {
            name: "dcdc".into(),
            file: "dcdc.snxsch".into(),
            placements: vec![at(SHEET_A)],
        },
        SheetMembers {
            name: "mcu".into(),
            file: "mcu.snxsch".into(),
            placements: vec![at(SHEET_B)],
        },
    ]
}

/// Create makes every ticked room and moves the staged part into its room,
/// all as one undo step; an unticked room is left out.
#[test]
fn create_rooms_makes_the_ticked_rooms_in_one_undo_step() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, path) = app_with(dir.path());
    app.ui_state.rooms.create = Some(Box::new(CreateRoomsDialog::new(
        path.clone(),
        "project.snxprj".into(),
        sheets(),
    )));
    let steps = undo_len(&app, &path);
    let task = app.update(Message::from(CreateRoomsMsg::Confirmed));
    settle(&mut app, task);
    let board = live(&app, &path);
    let names: Vec<&str> = board.rooms.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, ["dcdc", "mcu"]);
    assert!(app.ui_state.rooms.create.is_none(), "the dialog closes");
    assert_eq!(undo_len(&app, &path), steps + 1, "one undo step");
    let r3 = board
        .footprints
        .iter()
        .find(|f| f.reference == "R3")
        .expect("R3");
    assert!(r3.position.y < 40.0, "the staged part moved onto the board");
    run(&mut app, "undo");
    assert!(live(&app, &path).rooms.is_empty());

    app.ui_state.rooms.create = Some(Box::new(CreateRoomsDialog::new(
        path.clone(),
        "project.snxprj".into(),
        sheets(),
    )));
    let task = app.update(Message::from(CreateRoomsMsg::RowToggled("mcu".into())));
    settle(&mut app, task);
    let task = app.update(Message::from(CreateRoomsMsg::Confirmed));
    settle(&mut app, task);
    let names: Vec<String> = live(&app, &path)
        .rooms
        .iter()
        .map(|r| r.name.clone())
        .collect();
    assert_eq!(names, ["dcdc"]);
}
