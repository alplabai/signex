//! Row m163: rooms on a board. Place ▸ Room, what Room Properties shows and
//! the DRC rule are reachable; the Room Properties edits and the Create Rooms
//! dialog speak a message type the crate does not export.

use pcb_geom::ItemRef;
use signex_app::app::{InputMsg, Message, PcbMsg};
use signex_drc::ViolationKind;
use signex_types::pcb::{PcbBoard, Room, RoomOutline, RoomSource};
use signex_types::schematic::InstancePath;
use uuid::Uuid;

use super::support::{BoardBench, command, open_board, redo, undo};
use crate::settle_support::settle;

const SHEET_A: u128 = 0xCA;
const ROOM: Uuid = Uuid::from_u128(0xC163);

fn part(n: u128, x: f64, y: f64) -> serde_json::Value {
    serde_json::json!({
        "uuid": Uuid::from_u128(0xC000 + n), "reference": format!("R{n}"),
        "position": {"x": x, "y": y}, "layer": "TopCopper",
        "schematic_link": {
            "sheet": [Uuid::from_u128(SHEET_A)],
            "units": [Uuid::from_u128(0xD000 + n)]
        }
    })
}

/// A 60 x 40 mm board on a 0.5 mm grid: R1 at (10, 10) and R2 at (14, 12),
/// both of sheet A.
fn board() -> PcbBoard {
    serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(0xC0),
        "setup": {"grid_size": 0.5},
        "outline": [
            {"x": 0.0, "y": 0.0}, {"x": 60.0, "y": 0.0},
            {"x": 60.0, "y": 40.0}, {"x": 0.0, "y": 40.0}
        ],
        "layers": [{"id": 0, "name": "TopCopper", "layer_type": "signal"}],
        "footprints": [part(1, 10.0, 10.0), part(2, 14.0, 12.0)]
    }))
    .expect("the audit board")
}

/// [`board`] with a room of sheet A round R1 that leaves R2 outside.
fn board_with_room(keep_inside: bool) -> PcbBoard {
    let mut board = board();
    let mut room = Room::drawn(
        ROOM,
        "dcdc".to_string(),
        "#A058C8",
        RoomOutline::Rect {
            x_mm: 8.0,
            y_mm: 8.0,
            w_mm: 4.0,
            h_mm: 8.0,
        },
    );
    room.source = Some(RoomSource {
        sheet: InstancePath::from_chain(vec![Uuid::from_u128(SHEET_A)]),
        channel: None,
    });
    room.keep_parts_inside = keep_inside;
    room.margin_mm = 0.0;
    board.rooms = vec![room];
    board
}

fn draw_room(b: &mut BoardBench) {
    command(&mut b.app, "place_board_room");
    b.click(5.0, 5.0);
    b.click(25.0, 20.0);
}

fn run_drc(b: &mut BoardBench) -> Vec<ViolationKind> {
    settle(&mut b.app, Message::Pcb(PcbMsg::DrcRequested));
    b.app
        .interaction_state
        .pcb_drc
        .done_for(&b.path)
        .expect("the run finished")
        .report
        .violations
        .iter()
        .map(|violation| violation.kind)
        .collect()
}

#[test]
fn place_room_draws_a_named_room_from_two_corners_and_selects_it() {
    let mut b = open_board(board());
    draw_room(&mut b);
    let board = b.board();
    assert_eq!(board.rooms.len(), 1);
    let room = &board.rooms[0];
    assert_eq!(room.name, "Room 1");
    let bounds = room.outline.bounds().expect("an outline");
    assert_eq!(
        (bounds.x_mm, bounds.y_mm, bounds.w_mm, bounds.h_mm),
        (5.0, 5.0, 20.0, 15.0)
    );
    assert!(
        b.app
            .interaction_state
            .pcb_selection
            .selection()
            .items()
            .any(|item| item == ItemRef::Room(room.uuid)),
        "the new room is selected"
    );
    let shown = b
        .app
        .document_state
        .panel_ctx
        .pcb_board
        .as_ref()
        .and_then(|summary| summary.room.as_ref())
        .map(|facts| facts.room.uuid);
    assert_eq!(shown, Some(room.uuid), "Room Properties shows it");
}

#[test]
fn a_drawn_room_is_one_undo_step_and_marks_the_board_dirty() {
    let mut b = open_board(board());
    let before = b.json();
    draw_room(&mut b);
    let after = b.json();
    assert_ne!(before, after);
    assert!(b.dirty());
    undo(&mut b.app);
    assert_eq!(b.json(), before);
    assert!(!b.dirty(), "undo is back at the save point");
    redo(&mut b.app);
    assert_eq!(b.json(), after);
    assert!(b.dirty());
}

#[test]
fn one_corner_places_no_room() {
    let mut b = open_board(board());
    command(&mut b.app, "place_board_room");
    b.click(5.0, 5.0);
    assert!(b.board().rooms.is_empty());
    assert!(!b.dirty());
}

#[test]
fn two_corners_on_one_line_place_no_room() {
    let mut b = open_board(board());
    let before = b.json();
    command(&mut b.app, "place_board_room");
    b.click(5.0, 5.0);
    b.click(25.0, 5.0);
    assert_eq!(b.json(), before, "a room needs an area");
    assert!(!b.dirty());
}

#[test]
fn esc_after_the_first_corner_places_no_room() {
    let mut b = open_board(board());
    let before = b.json();
    command(&mut b.app, "place_board_room");
    b.click(5.0, 5.0);
    settle(
        &mut b.app,
        Message::Input(InputMsg::EscapePressed { window: None }),
    );
    assert_eq!(b.json(), before);
    assert!(!b.dirty());
}

#[test]
fn room_properties_lists_the_sheets_parts_and_which_are_outside() {
    let mut b = open_board(board_with_room(false));
    b.click(9.0, 15.0);
    let facts = b
        .app
        .document_state
        .panel_ctx
        .pcb_board
        .as_ref()
        .and_then(|summary| summary.room.clone())
        .expect("Room Properties shows the room");
    assert_eq!(facts.room.uuid, ROOM);
    let names: Vec<&str> = facts
        .parts
        .iter()
        .map(|part| part.designator.as_str())
        .collect();
    assert_eq!(names, ["R1", "R2"], "the parts of the room's sheet");
    assert_eq!(facts.parts[0].outside_nm, 0, "R1 is inside");
    assert!(facts.parts[1].outside_nm > 0, "R2 is outside");
    assert!(!b.dirty(), "selecting a room edits nothing");
}

#[test]
fn a_part_outside_a_keep_inside_room_is_a_drc_violation() {
    let mut b = open_board(board_with_room(true));
    let kinds = run_drc(&mut b);
    assert!(
        kinds.contains(&ViolationKind::PartOutsideRoom),
        "R2 is outside: {kinds:?}"
    );
}

#[test]
fn a_room_without_keep_parts_inside_is_not_checked() {
    let mut b = open_board(board_with_room(false));
    let kinds = run_drc(&mut b);
    assert!(
        !kinds.contains(&ViolationKind::PartOutsideRoom),
        "{kinds:?}"
    );
}

#[test]
fn create_rooms_on_a_board_in_no_project_changes_nothing() {
    let mut b = open_board(board());
    let before = b.json();
    command(&mut b.app, "create_rooms_from_sheets");
    assert_eq!(b.json(), before);
    assert!(!b.dirty());
}
