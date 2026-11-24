//! A moved harness entry slides along its connector's flat side, its wire
//! kept on it, as one undo step (docs/DECISIONS.md, 2026-11-24).

use crate::test_support::test_sheet;
use crate::*;
use signex_types::corner::DragWiringMode;
use signex_types::schematic::{
    HarnessConnector, HarnessEntry, Point, SelectedItem, SelectedKind, Wire,
};
use uuid::Uuid;

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

/// Entries SCK, MOSI, MISO (ids 100..=102), a signal wire on each ending on
/// its entry point and running left (ids 200..=202).
fn engine() -> Engine {
    let mut sheet = test_sheet();
    let connector = HarnessConnector {
        uuid: id(1),
        harness_type: "SPI_BUS".into(),
        position: Point::new(25.4, 25.4),
        width: 10.16,
        entry_pitch: 2.54,
        rotation: 0.0,
        entries: ["SCK", "MOSI", "MISO"]
            .iter()
            .enumerate()
            .map(|(i, n)| HarnessEntry {
                uuid: id(100 + i as u128),
                name: (*n).to_string(),
            })
            .collect(),
        locked: false,
    };
    for i in 0..3 {
        let end = connector.entry_point(i);
        sheet.wires.push(Wire {
            uuid: id(200 + i as u128),
            start: Point::new(end.x - 10.16, end.y),
            end,
            ..Default::default()
        });
    }
    sheet.harness.connectors.push(connector);
    Engine::new(sheet).unwrap()
}

fn names(engine: &Engine) -> Vec<String> {
    let c = &engine.document().harness.connectors[0];
    c.entries.iter().map(|e| e.name.clone()).collect()
}

/// The entry point each signal wire `200 + i` ends on, as the entry's name.
fn wired_names(engine: &Engine) -> Vec<String> {
    let doc = engine.document();
    let c = &doc.harness.connectors[0];
    (0..3)
        .map(|i| {
            let wire = doc.wires.iter().find(|w| w.uuid == id(200 + i)).unwrap();
            let slot = (0..c.entries.len())
                .find(|&s| {
                    let p = c.entry_point(s);
                    (p.x - wire.end.x).abs() < 1e-9 && (p.y - wire.end.y).abs() < 1e-9
                })
                .expect("the wire still ends on an entry");
            c.entries[slot].name.clone()
        })
        .collect()
}

fn slide(engine: &mut Engine, entry: u128, dy: f64, keep_wires: bool) -> CommandResult {
    engine
        .execute(Command::MoveSelection {
            items: vec![SelectedItem::new(id(entry), SelectedKind::HarnessEntry)],
            dx: 1.27,
            dy,
            keep_wires,
            wiring: DragWiringMode::Auto,
        })
        .unwrap()
}

#[test]
fn a_moved_entry_slides_down_the_side_and_keeps_its_wire() {
    let mut engine = engine();
    let depth = engine.undo_depth();
    let done = slide(&mut engine, 100, 5.08, true);
    assert!(done.changed);
    assert_eq!(names(&engine), ["MOSI", "MISO", "SCK"]);
    // Each signal wire still ends on the entry it was wired to.
    assert_eq!(wired_names(&engine), ["SCK", "MOSI", "MISO"]);
    assert_eq!(engine.undo_depth(), depth + 1, "one undo step");

    engine.undo().unwrap();
    assert_eq!(names(&engine), ["SCK", "MOSI", "MISO"]);
    assert_eq!(wired_names(&engine), ["SCK", "MOSI", "MISO"]);
}

#[test]
fn a_slide_past_the_top_stops_at_the_first_slot() {
    let mut engine = engine();
    slide(&mut engine, 101, -50.8, true);
    assert_eq!(names(&engine), ["MOSI", "SCK", "MISO"]);
}

#[test]
fn a_small_move_leaves_the_entry_in_its_slot() {
    let mut engine = engine();
    let depth = engine.undo_depth();
    let done = slide(&mut engine, 101, 0.5, true);
    assert!(!done.changed, "nothing moved, nothing recorded");
    assert_eq!(engine.undo_depth(), depth);
}

#[test]
fn without_keep_wires_the_wires_stay_where_they_are() {
    let mut engine = engine();
    let before = engine.document().wires.clone();
    slide(&mut engine, 100, 5.08, false);
    assert_eq!(names(&engine), ["MOSI", "MISO", "SCK"]);
    assert_eq!(engine.document().wires, before);
}

#[test]
fn an_entry_moved_with_its_connector_rides_along_in_its_slot() {
    let mut engine = engine();
    engine
        .execute(Command::MoveSelection {
            items: vec![
                SelectedItem::new(id(1), SelectedKind::HarnessConnector),
                SelectedItem::new(id(100), SelectedKind::HarnessEntry),
            ],
            dx: 0.0,
            dy: 5.08,
            keep_wires: false,
            wiring: DragWiringMode::Auto,
        })
        .unwrap();
    assert_eq!(names(&engine), ["SCK", "MOSI", "MISO"]);
    let at = engine.document().harness.connectors[0].position;
    assert!((at.y - (25.4 + 5.08)).abs() < 1e-9, "{at:?}");
}
