//! #2871 — pour cutouts in `[[pour_cutouts]]`: they round-trip apart from
//! `[[zones]]`, and a board without one writes no such array, so older
//! boards read and re-save unchanged.

use uuid::Uuid;

use super::tests::empty_board;
use super::*;
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
