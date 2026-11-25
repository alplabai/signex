//! #2506 — a pour's `unfilled` key: written only when on, read back as
//! saved, and a board with an unfilled pour writes `snxpcb/1.11`, which a
//! build that knows only 1.10 opens read-only. A board with no unfilled pour
//! keeps its token, byte for byte.

use uuid::Uuid;

use super::tests::empty_board;
use super::*;
use crate::format_version::FormatVersion;
use crate::pcb::{PcbBoard, Point, Zone};
use crate::zone_pour::POUR_FILL;

fn pour(n: u128, unfilled: bool) -> Zone {
    Zone {
        uuid: Uuid::from_u128(n),
        net: 0,
        net_name: String::new(),
        layer: "TopCopper".into(),
        outline: vec![
            Point::new(0.0, 0.0),
            Point::new(4.0, 0.0),
            Point::new(4.0, 3.0),
        ],
        holes: Vec::new(),
        priority: 1,
        fill_type: POUR_FILL.into(),
        thermal_relief: false,
        thermal_gap: 0.0,
        thermal_width: 0.0,
        clearance: 0.3,
        min_thickness: 0.0,
        min_island_area: 0.0,
        remove_dead_islands: false,
        shelved: false,
        unfilled,
        unknown: Default::default(),
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

/// The policy of a build that stops at `snxpcb/1.10`, before Unfill.
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
fn an_unfilled_pour_writes_snxpcb_1_11_and_round_trips() {
    let mut board = empty_board();
    board.zones = vec![pour(1, true), pour(2, false)];
    let text = save(&board);
    assert_eq!(text.matches("unfilled = true").count(), 1, "{text}");
    assert_eq!(token_of(&text), SNXPCB_FORMAT_V1_11, "{text}");
    let file = SnxPcb::parse(&text).unwrap();
    assert!(!file.is_from_newer_minor(), "this build reads 1.11");
    assert_eq!(file.board.zones, board.zones);
    assert!(file.board.zones[0].is_unfilled_pour());
    assert!(!file.board.zones[1].is_unfilled_pour());
    assert_eq!(
        file.write_string().unwrap(),
        text,
        "a re-save is byte-identical"
    );
}

/// A board with no unfilled pour writes no key and keeps its token; filling
/// the last unfilled pour again writes the bytes it had before.
#[test]
fn a_board_without_an_unfilled_pour_keeps_its_token() {
    let mut board = empty_board();
    board.zones = vec![pour(1, false)];
    let before = save(&board);
    assert!(!before.contains("unfilled"), "{before}");
    assert_eq!(token_of(&before), SNXPCB_FORMAT_V1);
    board.zones[0].unfilled = true;
    assert_eq!(token_of(&save(&board)), SNXPCB_FORMAT_V1_11);
    board.zones[0].unfilled = false;
    assert_eq!(save(&board), before, "the file is what it was");
}

/// A build whose policy stops at `snxpcb/1.10` sees a board with an unfilled
/// pour as newer: it opens it read-only and refuses its saves instead of
/// flooding the pour.
#[test]
fn an_unfilled_pour_opens_read_only_in_a_build_that_knows_only_1_10() {
    let mut board = empty_board();
    board.zones = vec![pour(1, true)];
    let text = save(&board);
    let token = token_of(&text);
    assert_eq!(token, SNXPCB_FORMAT_V1_11);
    let older = one_ten();
    assert!(older.is_newer_minor(token), "{token}");
    assert!(older.refuse_save(token).is_err(), "{token}");
    assert!(!SNXPCB_VERSION.is_newer_minor(token), "this build reads it");
    assert!(SNXPCB_VERSION.refuse_save(token).is_ok());
}
