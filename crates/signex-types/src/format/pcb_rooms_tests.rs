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

/// 2026-11-20: a board with rooms writes `snxpcb/1.6` and reads back as
/// itself, not read-only.
#[test]
fn rooms_round_trip_and_a_second_save_is_byte_identical() {
    let text = with_rooms();
    assert!(text.contains("[[rooms]]"), "{text}");
    assert!(text.contains("shape = \"polygon\""), "{text}");
    assert!(text.contains("margin = 500000"), "{text}");
    assert_eq!(SNXPCB_FORMAT_ROOMS, "snxpcb/1.6");
    assert!(
        text.starts_with(&format!("format = \"{SNXPCB_FORMAT_ROOMS}\"")),
        "a board with rooms writes 1.6: {text}"
    );
    let file = SnxPcb::parse(&text).expect("reads back");
    assert!(!file.is_from_newer_minor(), "this build reads 1.6");
    assert!(file.board.unknown.newer_minor_source.is_none());
    let back = file.board;
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

/// Caner (2026-11-20): a room keeps no key this build does not know.
#[test]
fn a_key_a_room_does_not_know_is_not_kept() {
    let text = with_rooms().replacen("name = \"dcdc\"", "name = \"dcdc\"\nfuture_room_key = 7", 1);
    let board = SnxPcb::parse(&text).expect("parses").board;
    assert_eq!(board.rooms, rooms());
    let out = SnxPcb::new(board).write_string().expect("writes");
    assert!(!out.contains("future_room_key"), "{out}");
}

/// 2026-11-20: rooms raise the token over layer colours and only rooms do.
/// A board without rooms keeps 1.4, or 1.5 with layer colours, byte for
/// byte; removing its last room writes the lower token again.
#[test]
fn a_board_without_rooms_keeps_its_token_byte_for_byte() {
    use crate::pcb_layer::{PcbLayer, TechnicalLayer};
    use crate::pcb_options::MARK_COLORS;
    let plain = SnxPcb::new(empty_board()).write_string().expect("writes");
    assert!(
        plain.starts_with(&format!("format = \"{SNXPCB_FORMAT_V1}\"")),
        "{plain}"
    );

    let mut coloured = empty_board();
    let silk = PcbLayer::Technical(TechnicalLayer::SilkscreenTop);
    assert!(coloured.options.set_layer_color(silk, Some(MARK_COLORS[1])));
    let colours_only = SnxPcb::new(coloured.clone())
        .write_string()
        .expect("writes");
    assert!(
        colours_only.starts_with(&format!("format = \"{SNXPCB_FORMAT_LAYER_COLORS}\"")),
        "{colours_only}"
    );

    let mut both = coloured.clone();
    both.rooms = rooms();
    let text = SnxPcb::new(both).write_string().expect("writes");
    assert!(
        text.starts_with(&format!("format = \"{SNXPCB_FORMAT_ROOMS}\"")),
        "rooms win over layer colours: {text}"
    );
    let back = SnxPcb::parse(&text).expect("reads back").board;
    assert_eq!(back.rooms, rooms());
    assert_eq!(back.options.layer_colors, coloured.options.layer_colors);

    let mut emptied = back;
    emptied.rooms.clear();
    let again = SnxPcb::new(emptied).write_string().expect("writes");
    assert_eq!(again, colours_only, "no rooms: byte-identical 1.5");
    let mut emptied = SnxPcb::parse(&with_rooms()).expect("parses").board;
    emptied.rooms.clear();
    let again = SnxPcb::new(emptied).write_string().expect("writes");
    assert_eq!(again, plain, "no rooms, no colours: byte-identical 1.4");
}

/// 2026-11-20: a build whose policy stops at `snxpcb/1.5` sees a board with
/// rooms as a newer minor, so it opens it read-only and refuses every save,
/// instead of loading it and dropping the rooms on the next save.
#[test]
fn a_build_that_knows_only_1_5_opens_a_board_with_rooms_read_only() {
    let one_five = FormatVersion {
        current: SNXPCB_FORMAT_LAYER_COLORS,
        readable: &[
            "snxpcb/1",
            "snxpcb/1.1",
            "snxpcb/1.2",
            "snxpcb/1.3",
            SNXPCB_FORMAT_V1,
            SNXPCB_FORMAT_LAYER_COLORS,
        ],
        ..SNXPCB_VERSION
    };
    let text = with_rooms();
    let token = SNXPCB_FORMAT_ROOMS;
    assert_eq!(
        text.lines().next(),
        Some(format!("format = \"{token}\"").as_str())
    );
    assert_eq!(one_five.read(Some(token)), Ok(token.to_string()));
    assert!(one_five.is_newer_minor(token));
    let err = one_five.refuse_save(token).unwrap_err();
    assert!(err.to_string().contains(SNXPCB_FORMAT_ROOMS), "{err}");

    for older in [SNXPCB_FORMAT_V1, SNXPCB_FORMAT_LAYER_COLORS] {
        assert!(!one_five.is_newer_minor(older), "{older}");
        assert!(one_five.refuse_save(older).is_ok(), "{older}");
    }
    assert!(
        !SNXPCB_VERSION.is_newer_minor(token),
        "this build reads 1.6"
    );
}
