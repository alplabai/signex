//! #2871 — pour cutouts in `[[pour_cutouts]]`: they round-trip apart from
//! `[[zones]]`, and a board without one writes no such array, so older
//! boards read and re-save unchanged.

use uuid::Uuid;

use super::tests::empty_board;
use super::*;
use crate::format_version::FormatVersion;
use crate::pcb::{PcbBoard, Point, Zone};
use crate::zone_cutout::POUR_CUTOUT_FILL;
use crate::zone_pour::POUR_FILL;

fn area(n: u128, fill: &str) -> Zone {
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
        priority: 0,
        fill_type: fill.into(),
        thermal_relief: false,
        thermal_gap: 0.0,
        thermal_width: 0.0,
        clearance: 0.0,
        min_thickness: 0.0,
        min_island_area: 0.0,
        remove_dead_islands: false,
        shelved: false,
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
fn a_cutout_is_written_in_its_own_array_and_round_trips() {
    let mut board = empty_board();
    board.zones = vec![area(1, POUR_FILL), area(2, POUR_CUTOUT_FILL)];
    let text = save(&board);
    assert_eq!(text.matches("[[zones]]").count(), 1, "{text}");
    assert_eq!(text.matches("[[pour_cutouts]]").count(), 1, "{text}");
    let loaded = load(&text);
    assert_eq!(loaded.zones, board.zones);
    assert!(loaded.zones[1].is_pour_cutout());
    assert_eq!(save(&loaded), text, "a re-save is byte-identical");
}

/// An entry of the array is a cutout whatever fill type it names, and the
/// cutouts follow the zones.
#[test]
fn an_entry_of_the_array_is_always_a_cutout() {
    let mut board = empty_board();
    board.zones = vec![area(2, POUR_CUTOUT_FILL), area(1, POUR_FILL)];
    let text = save(&board).replace("fill_type = \"pour_cutout\"", "fill_type = \"solid\"");
    let loaded = load(&text);
    assert_eq!(loaded.zones[0].uuid, Uuid::from_u128(1));
    assert!(loaded.zones[1].is_pour_cutout(), "{text}");
}

/// A board without a cutout writes no `[[pour_cutouts]]`, and a file from
/// before cutouts loads with none and re-saves as it was.
#[test]
fn a_board_without_a_cutout_reads_and_saves_as_before() {
    let mut board = empty_board();
    board.zones = vec![area(1, POUR_FILL)];
    let text = save(&board);
    assert!(!text.contains("pour_cutouts"), "{text}");
    let loaded = load(&text);
    assert_eq!(loaded.zones, board.zones);
    assert!(loaded.zones.iter().all(|z| !z.is_pour_cutout()));
    assert_eq!(save(&loaded), text);
}

fn token_of(text: &str) -> &str {
    let first = text.lines().next().unwrap_or_default();
    first
        .strip_prefix("format = \"")
        .and_then(|rest| rest.strip_suffix('"'))
        .unwrap_or_default()
}

/// The policy of a build that stops at `snxpcb/1.10`, before cutouts.
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

/// DECISIONS 2026-11-24: a pour cutout writes `snxpcb/1.11`, which this
/// build opens for editing and re-saves byte-identical.
#[test]
fn a_pour_cutout_writes_snxpcb_1_11_and_round_trips() {
    let mut board = empty_board();
    board.zones = vec![area(1, POUR_FILL), area(2, POUR_CUTOUT_FILL)];
    let text = save(&board);
    assert_eq!(token_of(&text), SNXPCB_FORMAT_V1_11, "{text}");
    let file = SnxPcb::parse(&text).unwrap();
    assert!(!file.is_from_newer_minor(), "this build reads 1.11");
    assert_eq!(file.board.zones, board.zones);
    assert_eq!(
        file.write_string().unwrap(),
        text,
        "a re-save is byte-identical"
    );
}

/// A board with no cutout keeps the token it had before cutouts, and
/// deleting the last cutout writes that token again.
#[test]
fn a_board_without_a_cutout_keeps_its_token() {
    assert_eq!(token_of(&save(&empty_board())), SNXPCB_FORMAT_V1);
    let mut board = empty_board();
    board.zones = vec![area(1, POUR_FILL)];
    let before = save(&board);
    assert_eq!(token_of(&before), SNXPCB_FORMAT_V1);
    board.zones.push(area(2, POUR_CUTOUT_FILL));
    assert_eq!(token_of(&save(&board)), SNXPCB_FORMAT_V1_11);
    board.zones.pop();
    assert_eq!(save(&board), before, "the file is what it was");
}

/// A build whose policy stops at `snxpcb/1.10` sees a board with a cutout as
/// newer: it opens it read-only and refuses its saves.
#[test]
fn a_pour_cutout_opens_read_only_in_a_build_that_knows_only_1_10() {
    let mut board = empty_board();
    board.zones = vec![area(2, POUR_CUTOUT_FILL)];
    let text = save(&board);
    let token = token_of(&text);
    assert_eq!(token, SNXPCB_FORMAT_V1_11);
    let older = one_ten();
    assert!(older.is_newer_minor(token), "{token}");
    assert!(older.refuse_save(token).is_err(), "{token}");
    assert!(!SNXPCB_VERSION.is_newer_minor(token), "this build reads it");
    assert!(SNXPCB_VERSION.refuse_save(token).is_ok());
}
