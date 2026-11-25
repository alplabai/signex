//! DECISIONS 2026-11-25: a board drawing or string on a copper layer is
//! drawn copper (#2883) and writes `snxpcb/1.11`; the same item on a
//! non-copper layer keeps the board's lower token, byte for byte.

use uuid::Uuid;

use super::tests::empty_board;
use super::*;
use crate::format_version::FormatVersion;
use crate::pcb::{BoardGraphic, BoardText, PcbBoard, Point};

fn line(layer: &str) -> BoardGraphic {
    BoardGraphic {
        uuid: Uuid::from_u128(7),
        graphic_type: "line".into(),
        layer: layer.into(),
        width: 0.2,
        start: Some(Point::new(0.0, 0.0)),
        end: Some(Point::new(5.0, 0.0)),
        center: None,
        radius: 0.0,
        points: Vec::new(),
        text: String::new(),
        font_size: 0.0,
    }
}

fn string(layer: &str) -> BoardText {
    BoardText {
        uuid: Uuid::from_u128(8),
        text: "GND".into(),
        position: Point::new(2.0, 3.0),
        layer: layer.into(),
        font_size: 1.0,
        rotation: 0.0,
    }
}

fn save(board: &PcbBoard) -> String {
    SnxPcb::new(board.clone()).write_string().unwrap()
}

fn token_of(text: &str) -> &str {
    let first = text.lines().next().unwrap_or_default();
    first
        .strip_prefix("format = \"")
        .and_then(|rest| rest.strip_suffix('"'))
        .unwrap_or_default()
}

/// The policy of a build that stops at `snxpcb/1.10`.
fn one_ten() -> FormatVersion {
    FormatVersion {
        current: SNXPCB_FORMAT_V1_10,
        readable: &[
            "snxpcb/1",
            "snxpcb/1.1",
            "snxpcb/1.2",
            "snxpcb/1.3",
            SNXPCB_FORMAT_V1,
            SNXPCB_FORMAT_LAYER_COLORS,
            SNXPCB_FORMAT_ROOMS,
            SNXPCB_FORMAT_BOARD_SKETCH_TARGETS,
            SNXPCB_FORMAT_V1_8,
            SNXPCB_FORMAT_V1_9,
            SNXPCB_FORMAT_V1_10,
        ],
        ..SNXPCB_VERSION
    }
}

#[test]
fn a_copper_drawing_writes_snxpcb_1_11_and_round_trips() {
    for layer in ["TopCopper", "Inner2Copper", "BottomCopper"] {
        let mut board = empty_board();
        board.graphics.push(line(layer));
        assert!(board_has_drawn_copper(&board), "{layer}");
        let text = save(&board);
        assert_eq!(token_of(&text), SNXPCB_FORMAT_V1_11, "{text}");
        let file = SnxPcb::parse(&text).unwrap();
        assert!(!file.is_from_newer_minor(), "this build reads 1.11");
        assert_eq!(file.board.graphics, board.graphics);
        assert_eq!(file.write_string().unwrap(), text, "re-save is identical");
    }
}

#[test]
fn copper_text_writes_snxpcb_1_11() {
    let mut board = empty_board();
    board.texts.push(string("BottomCopper"));
    let text = save(&board);
    assert_eq!(token_of(&text), SNXPCB_FORMAT_V1_11, "{text}");
    let file = SnxPcb::parse(&text).unwrap();
    assert_eq!(file.board.texts, board.texts);
    assert_eq!(file.write_string().unwrap(), text);
}

/// The same drawing and string on silk (or a mechanical layer, or a layer
/// name this build does not know) are no copper: the token stays, and the
/// file is what it was before drawn copper.
#[test]
fn the_same_items_off_copper_keep_the_lower_token() {
    for layer in ["TopSilkscreen", "Mechanical3", "NotALayer"] {
        let mut board = empty_board();
        board.graphics.push(line(layer));
        board.texts.push(string(layer));
        assert!(!board_has_drawn_copper(&board), "{layer}");
        assert_eq!(token_of(&save(&board)), SNXPCB_FORMAT_V1, "{layer}");
    }
}

/// Moving the last copper drawing off copper writes the lower token again.
#[test]
fn moving_the_drawing_off_copper_goes_back_to_the_lower_token() {
    let mut board = empty_board();
    board.graphics.push(line("TopSilkscreen"));
    let before = save(&board);
    board.graphics[0].layer = "TopCopper".into();
    assert_eq!(token_of(&save(&board)), SNXPCB_FORMAT_V1_11);
    board.graphics[0].layer = "TopSilkscreen".into();
    assert_eq!(save(&board), before, "the file is what it was");
}

/// A build whose policy stops at `snxpcb/1.10` opens a board with drawn
/// copper read-only and refuses its saves.
#[test]
fn drawn_copper_opens_read_only_in_a_build_that_knows_only_1_10() {
    let mut board = empty_board();
    board.texts.push(string("TopCopper"));
    let text = save(&board);
    let token = token_of(&text);
    assert_eq!(token, SNXPCB_FORMAT_V1_11);
    let older = one_ten();
    assert!(older.is_newer_minor(token), "{token}");
    assert!(older.refuse_save(token).is_err(), "{token}");
    assert!(!SNXPCB_VERSION.is_newer_minor(token), "this build reads it");
    assert!(SNXPCB_VERSION.refuse_save(token).is_ok());
}
