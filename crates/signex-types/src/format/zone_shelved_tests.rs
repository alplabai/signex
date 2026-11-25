//! #2523 — a pour's `shelved` key: written only when on, read back as
//! saved, and absent from a board with no shelved pour, so older boards
//! re-save byte-identical.

use uuid::Uuid;

use super::tests::empty_board;
use super::*;
use crate::format_version::FormatVersion;
use crate::pcb::{PcbBoard, Point, Zone};
use crate::zone_pour::POUR_FILL;

fn pour(n: u128, shelved: bool) -> Zone {
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
        shelved,
        unfilled: false,
        unknown: Default::default(),
    }
}

fn save(board: &PcbBoard) -> String {
    SnxPcb::new(board.clone()).write_string().unwrap()
}

fn load(text: &str) -> PcbBoard {
    SnxPcb::parse(text).unwrap().board
}

#[test]
fn a_shelved_pour_round_trips_and_keeps_its_settings() {
    let mut board = empty_board();
    board.zones = vec![pour(1, true), pour(2, false)];
    let text = save(&board);
    assert_eq!(text.matches("shelved = true").count(), 1, "{text}");
    let loaded = load(&text);
    assert_eq!(loaded.zones, board.zones);
    assert!(loaded.zones[0].is_shelved_pour());
    assert!(!loaded.zones[1].is_shelved_pour());
    assert_eq!(save(&loaded), text, "a re-save is byte-identical");
}

#[test]
fn a_board_with_no_shelved_pour_writes_no_shelved_key() {
    let mut board = empty_board();
    board.zones = vec![pour(1, false)];
    let text = save(&board);
    assert!(!text.contains("shelved"), "{text}");
    let loaded = load(&text);
    assert!(!loaded.zones[0].shelved);
    assert_eq!(save(&loaded), text);
}

fn token_of(text: &str) -> &str {
    let first = text.lines().next().unwrap_or_default();
    first
        .strip_prefix("format = \"")
        .and_then(|rest| rest.strip_suffix('"'))
        .unwrap_or_default()
}

/// #2738: one shelved pour and nothing else raises the token past what the
/// previous build reads, so that build opens the board read-only and refuses
/// its saves instead of flooding the pour.
#[test]
fn a_shelved_pour_opens_read_only_in_a_build_that_knows_only_1_8() {
    let one_eight = FormatVersion {
        current: SNXPCB_FORMAT_V1_8,
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
        ],
        ..SNXPCB_VERSION
    };
    let mut board = empty_board();
    board.zones = vec![pour(1, true)];
    let text = save(&board);
    let token = token_of(&text);
    assert_eq!(token, SNXPCB_FORMAT_V1_9);
    assert!(one_eight.is_newer_minor(token), "{token}");
    assert!(one_eight.refuse_save(token).is_err(), "{token}");
    assert!(!SNXPCB_VERSION.is_newer_minor(token), "this build reads it");
    assert!(!SnxPcb::parse(&text).unwrap().is_from_newer_minor());
}

/// #2738: a board without a shelved pour keeps its token, and unshelving the
/// last one writes the lower token again.
#[test]
fn a_board_without_a_shelved_pour_keeps_its_token() {
    let mut board = empty_board();
    board.zones = vec![pour(1, false)];
    assert_eq!(token_of(&save(&board)), SNXPCB_FORMAT_V1);
    board.zones[0].shelved = true;
    assert_eq!(token_of(&save(&board)), SNXPCB_FORMAT_V1_9);
    board.zones[0].shelved = false;
    assert_eq!(token_of(&save(&board)), SNXPCB_FORMAT_V1);
}

/// Unshelving drops the key again, so the file is what it was before the
/// pour was shelved.
#[test]
fn unshelving_restores_the_file_bytes() {
    let mut board = empty_board();
    board.zones = vec![pour(1, false)];
    let before = save(&board);
    board.zones[0].shelved = true;
    assert_ne!(save(&board), before);
    board.zones[0].shelved = false;
    assert_eq!(save(&board), before);
}
