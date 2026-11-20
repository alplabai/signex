//! #1280: copy and paste carry harness connectors (with their entries) and
//! harness wires, under fresh ids.

use crate::test_support::test_sheet;
use crate::{Command, Engine};
use signex_types::object_id::SequentialIds;
use signex_types::schematic::{
    HarnessConnector, HarnessEntry, HarnessWire, Point, SelectedItem, SelectedKind,
};
use uuid::Uuid;

fn id(n: u128) -> Uuid {
    Uuid::from_u128(0xC1_0000 + n)
}

fn sheet_with_harness() -> Engine {
    let mut sheet = test_sheet();
    sheet.harness.connectors.push(HarnessConnector {
        uuid: id(1),
        harness_type: "SPI_BUS".into(),
        position: Point::new(25.4, 25.4),
        width: 10.16,
        entry_pitch: 2.54,
        rotation: 0.0,
        entries: vec![
            HarnessEntry {
                uuid: id(2),
                name: "SCK".into(),
            },
            HarnessEntry {
                uuid: id(3),
                name: "MOSI".into(),
            },
        ],
        locked: true,
    });
    sheet.harness.wires.push(HarnessWire {
        uuid: id(4),
        start: Point::new(50.8, 25.4),
        end: Point::new(76.2, 25.4),
        locked: false,
    });
    Engine::new(sheet).unwrap()
}

fn items() -> Vec<SelectedItem> {
    vec![
        SelectedItem::new(id(1), SelectedKind::HarnessConnector),
        SelectedItem::new(id(4), SelectedKind::HarnessWire),
    ]
}

#[test]
fn a_copy_pastes_the_connector_its_entries_and_the_wire_under_new_ids() {
    let engine = sheet_with_harness();
    let copied = engine.copy_selection(&items(), 1.27).unwrap().unwrap();
    assert_eq!(copied.payload.item_count(), 2);
    let mut target = Engine::new(test_sheet()).unwrap();
    target
        .execute_batch(
            copied
                .payload
                .place_commands(copied.origin, &mut SequentialIds::default())
                .unwrap(),
        )
        .unwrap();
    let harness = &target.document().harness;
    assert_eq!(harness.connectors.len(), 1);
    assert_eq!(harness.wires.len(), 1);
    let c = &harness.connectors[0];
    assert_eq!(c.harness_type, "SPI_BUS");
    assert_eq!(c.entries.len(), 2);
    assert!(c.locked, "the copy keeps its lock");
    let original = [id(1), id(2), id(3), id(4)];
    assert!(!original.contains(&c.uuid));
    assert!(c.entries.iter().all(|e| !original.contains(&e.uuid)));
    assert!(!original.contains(&harness.wires[0].uuid));
    // Placed at the origin, the copy lands over the original.
    assert!((c.position.x - 25.4).abs() < 1e-9 && (c.position.y - 25.4).abs() < 1e-9);
}

#[test]
fn a_lone_entry_is_not_carried_so_cut_leaves_it() {
    let entry = SelectedItem::new(id(2), SelectedKind::HarnessEntry);
    let (cuttable, kept) = crate::partition_cuttable(&[entry]);
    assert!(cuttable.is_empty());
    assert_eq!(kept, [entry]);
}

#[test]
fn a_copied_group_turns_with_its_harness() {
    let engine = sheet_with_harness();
    let copied = engine.copy_selection(&items(), 1.27).unwrap().unwrap();
    let turned = copied.payload.rotate(false).unwrap();
    let c = &turned.fragment().harness.connectors[0];
    assert_ne!(c.rotation, 0.0);
    assert!(c.locked);
    // The wire turned a quarter: it now runs vertically.
    let w = &turned.fragment().harness.wires[0];
    assert!((w.start.x - w.end.x).abs() < 1e-9);
}

#[test]
fn the_clipboard_text_round_trips_the_harness() {
    let engine = sheet_with_harness();
    let copied = engine.copy_selection(&items(), 1.27).unwrap().unwrap();
    let text = copied.to_clipboard_text().unwrap();
    let back = crate::CopiedSelection::from_clipboard_text(&text, 1.27)
        .unwrap()
        .unwrap();
    let (was, now) = (
        &copied.payload.fragment().harness,
        &back.payload.fragment().harness,
    );
    assert_eq!(now.connectors.len(), 1);
    assert_eq!(now.wires.len(), 1);
    assert_eq!(
        now.connectors[0].harness_type,
        was.connectors[0].harness_type
    );
    assert_eq!(now.connectors[0].entries, was.connectors[0].entries);
    let near = |a: Point, b: Point| (a.x - b.x).abs() < 1e-6 && (a.y - b.y).abs() < 1e-6;
    assert!(near(now.connectors[0].position, was.connectors[0].position));
    assert!(near(now.wires[0].end, was.wires[0].end));
}

#[test]
fn place_commands_place_the_connector_then_the_wire() {
    let engine = sheet_with_harness();
    let copied = engine.copy_selection(&items(), 1.27).unwrap().unwrap();
    let commands = copied
        .payload
        .place_commands(Point::new(0.0, 0.0), &mut SequentialIds::default())
        .unwrap();
    assert!(matches!(
        commands.as_slice(),
        [
            Command::PlaceHarnessConnector { .. },
            Command::PlaceHarnessWire { .. }
        ]
    ));
}
