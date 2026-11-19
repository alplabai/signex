//! #2457: the optional `[[rooms]]` array of a `.snxpcb`.

use uuid::Uuid;

use super::tests::empty_board;
use super::*;
use crate::pcb::{Point, Room, RoomLayer, RoomOutline, RoomSource};
use crate::schematic::InstancePath;

const OLD_BOARD: &str = include_str!("../../tests/corpus/pcb/board-outline-only.snxpcb");

fn rooms() -> Vec<Room> {
    let mut dcdc = Room::drawn(
        Uuid::from_u128(1),
        "dcdc".into(),
        "#A058C8",
        RoomOutline::Rect {
            x_mm: 20.0,
            y_mm: 2.0,
            w_mm: 24.0,
            h_mm: 33.0,
        },
    );
    dcdc.source = Some(RoomSource {
        sheet: InstancePath::from_chain(vec![Uuid::from_u128(10)]),
        channel: None,
    });
    dcdc.keep_parts_inside = true;
    dcdc.margin_mm = 0.5;
    let mut channel = Room::drawn(
        Uuid::from_u128(2),
        "amp_CH2".into(),
        "#409C58",
        RoomOutline::Polygon(vec![
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
            Point::new(10.0, 6.0),
            Point::new(3.0, 10.0),
        ]),
    );
    channel.source = Some(RoomSource {
        sheet: InstancePath::from_chain(vec![Uuid::from_u128(11), Uuid::from_u128(12)]),
        channel: Some(2),
    });
    channel.layer = RoomLayer::Both;
    channel.locked = true;
    channel.margin_mm = 0.0;
    vec![dcdc, channel]
}

fn with_rooms() -> String {
    let mut board = empty_board();
    board.rooms = rooms();
    SnxPcb::new(board).write_string().expect("writes")
}

#[test]
fn rooms_round_trip_and_a_second_save_is_byte_identical() {
    let text = with_rooms();
    assert!(text.contains("[[rooms]]"), "{text}");
    assert!(text.contains("shape = \"polygon\""), "{text}");
    assert!(text.contains("margin = 500000"), "{text}");
    assert!(
        text.contains("format = \"snxpcb/1.4\""),
        "no version bump: {text}"
    );
    let back = SnxPcb::parse(&text).expect("reads back").board;
    assert_eq!(back.rooms, rooms());
    assert!(
        back.unknown.manifest.is_empty(),
        "rooms are not foreign keys"
    );
    let again = SnxPcb::new(back).write_string().expect("writes again");
    assert_eq!(again, text);
}

#[test]
fn a_board_without_rooms_saves_byte_identical() {
    let board = SnxPcb::parse(OLD_BOARD).expect("parses").board;
    assert!(board.rooms.is_empty());
    let text = SnxPcb::new(board).write_string().expect("writes");
    assert_eq!(text, OLD_BOARD, "no-edit save must keep every byte");
    assert!(!text.contains("rooms"), "{text}");
}

#[test]
fn a_malformed_room_is_refused_naming_its_row() {
    let text = with_rooms();
    let broken = text.replacen("layer = \"both\"", "layer = \"sideways\"", 1);
    match SnxPcb::parse(&broken) {
        Err(FormatError::TsvFieldParse {
            block, row, field, ..
        }) => {
            assert_eq!((block.as_str(), row, field.as_str()), ("rooms", 1, "layer"));
        }
        other => panic!("expected a refusal naming the room row, got {other:?}"),
    }
    let colour = text.replacen("\"#A058C8\"", "\"purple\"", 1);
    assert!(matches!(
        SnxPcb::parse(&colour),
        Err(FormatError::TsvFieldParse { row: 0, .. })
    ));
}

#[test]
fn a_non_finite_room_is_refused_on_write() {
    let mut board = empty_board();
    let mut list = rooms();
    list[0].margin_mm = f64::NAN;
    board.rooms = list;
    assert!(matches!(
        SnxPcb::new(board).write_string(),
        Err(FormatError::NonFiniteField { row: 0, .. })
    ));
}

#[test]
fn a_foreign_key_of_a_room_survives_a_save() {
    let text = with_rooms().replacen("name = \"dcdc\"", "name = \"dcdc\"\nfuture_room_key = 7", 1);
    let board = SnxPcb::parse(&text).expect("parses").board;
    assert!(board.rooms[0].unknown.contains_key("future_room_key"));
    let out = SnxPcb::new(board).write_string().expect("writes");
    assert!(out.contains("future_room_key = 7"), "{out}");
}

/// What a `snxpcb/1.4` build without rooms does with the array: it is a
/// top-level key it does not declare, so the manifest catch-all keeps it and
/// the save writes it back. Shown here with a key this build does not
/// declare either.
#[test]
fn an_older_build_keeps_the_rooms_as_a_foreign_key() {
    let text = with_rooms().replace("[[rooms]]", "[[rooms_unknown_here]]");
    let board = SnxPcb::parse(&text).expect("parses").board;
    assert!(board.rooms.is_empty());
    assert!(board.unknown.manifest.contains_key("rooms_unknown_here"));
    let out = SnxPcb::new(board).write_string().expect("writes");
    let back = SnxPcb::parse(&out.replace("[[rooms_unknown_here]]", "[[rooms]]"))
        .expect("reads back")
        .board;
    assert_eq!(back.rooms, rooms(), "{out}");
}
