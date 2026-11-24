//! #1280: placing and editing signal harness objects, each one undo step,
//! and the selection commands on them.

use crate::test_support::test_sheet;
use crate::*;
use signex_types::schematic::{
    ChildSheet, FillType, HarnessConnector, HarnessEntry, HarnessWire, Point, SelectedItem,
    SelectedKind, SheetPin,
};
use uuid::Uuid;

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

fn connector(names: &[&str]) -> HarnessConnector {
    HarnessConnector {
        uuid: id(1),
        harness_type: "SPI_BUS".into(),
        position: Point::new(10.0, 10.0),
        width: 10.16,
        entry_pitch: 2.54,
        rotation: 0.0,
        entries: names
            .iter()
            .enumerate()
            .map(|(i, n)| HarnessEntry {
                uuid: id(100 + i as u128),
                name: (*n).to_string(),
            })
            .collect(),
        locked: false,
    }
}

fn harness_wire() -> HarnessWire {
    HarnessWire {
        uuid: id(2),
        start: Point::new(0.0, 0.0),
        end: Point::new(10.0, 0.0),
        locked: false,
    }
}

fn placed() -> Engine {
    let mut engine = Engine::new(test_sheet()).unwrap();
    engine
        .execute(Command::PlaceHarnessConnector {
            connector: connector(&["SCK", "MOSI"]),
        })
        .unwrap();
    engine
}

#[test]
fn placing_a_connector_is_one_undo_step_with_a_harness_patch() {
    let mut engine = Engine::new(test_sheet()).unwrap();
    let done = engine
        .execute(Command::PlaceHarnessConnector {
            connector: connector(&["SCK"]),
        })
        .unwrap();
    assert_eq!(done.patch_pair.unwrap().document, DocumentPatch::HARNESS);
    assert_eq!(engine.document().harness.connectors.len(), 1);
    engine.undo().unwrap();
    assert!(engine.document().harness.connectors.is_empty());
    assert!(!engine.can_undo());
}

#[test]
fn a_connector_without_type_or_size_is_refused() {
    let mut engine = Engine::new(test_sheet()).unwrap();
    let mut bad = connector(&["SCK"]);
    bad.harness_type = " ".into();
    assert!(matches!(
        engine.execute(Command::PlaceHarnessConnector { connector: bad }),
        Err(EngineError::InvalidHarnessConnector)
    ));
    let mut flat = connector(&["SCK"]);
    flat.width = 0.0;
    assert!(
        engine
            .execute(Command::PlaceHarnessConnector { connector: flat })
            .is_err()
    );
    let mut nameless = connector(&[""]);
    nameless.uuid = id(7);
    assert!(matches!(
        engine.execute(Command::PlaceHarnessConnector {
            connector: nameless
        }),
        Err(EngineError::EmptyHarnessEntryName)
    ));
    assert!(engine.document().harness.connectors.is_empty());
}

#[test]
fn consecutive_updates_of_one_connector_are_one_undo_step() {
    let mut engine = placed();
    for width in [12.0, 13.0, 14.0] {
        let mut next = connector(&["SCK", "MOSI"]);
        next.width = width;
        let done = engine
            .execute(Command::UpdateHarnessConnector {
                uuid: id(1),
                connector: next,
            })
            .unwrap();
        assert!(done.changed);
    }
    assert_eq!(engine.document().harness.connectors[0].width, 14.0);
    engine.undo().unwrap();
    assert_eq!(engine.document().harness.connectors[0].width, 10.16);
}

#[test]
fn an_unchanged_update_records_nothing_and_a_missing_one_is_refused() {
    let mut engine = placed();
    let depth = engine.undo_depth();
    let same = engine
        .execute(Command::UpdateHarnessConnector {
            uuid: id(1),
            connector: connector(&["SCK", "MOSI"]),
        })
        .unwrap();
    assert!(!same.changed);
    assert_eq!(engine.undo_depth(), depth);
    assert!(matches!(
        engine.execute(Command::UpdateHarnessConnector {
            uuid: id(99),
            connector: connector(&["SCK"]),
        }),
        Err(EngineError::HarnessNotFound { .. })
    ));
}

#[test]
fn an_entry_lands_in_its_slot_as_one_step() {
    let mut engine = placed();
    engine
        .execute(Command::PlaceHarnessEntry {
            connector_id: id(1),
            entry: HarnessEntry {
                uuid: id(200),
                name: "CS".into(),
            },
            index: 1,
        })
        .unwrap();
    let names: Vec<&str> = engine.document().harness.connectors[0]
        .entries
        .iter()
        .map(|e| e.name.as_str())
        .collect();
    assert_eq!(names, ["SCK", "CS", "MOSI"]);
    engine.undo().unwrap();
    assert_eq!(engine.document().harness.connectors[0].entries.len(), 2);
}

#[test]
fn a_harness_wire_needs_two_ends() {
    let mut engine = Engine::new(test_sheet()).unwrap();
    let mut dot = harness_wire();
    dot.end = dot.start;
    assert!(matches!(
        engine.execute(Command::PlaceHarnessWire { wire: dot }),
        Err(EngineError::InvalidHarnessWire)
    ));
    engine
        .execute(Command::PlaceHarnessWire {
            wire: harness_wire(),
        })
        .unwrap();
    assert_eq!(engine.document().harness.wires.len(), 1);
}

#[test]
fn a_sheet_entry_takes_and_drops_a_harness_type() {
    let mut sheet = test_sheet();
    sheet.child_sheets.push(ChildSheet {
        uuid: id(50),
        name: "sensor".into(),
        filename: "sensor.snxsch".into(),
        position: Point::new(50.0, 0.0),
        size: (20.0, 20.0),
        stroke_width: 0.0,
        fill: FillType::None,
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins: vec![SheetPin {
            uuid: id(51),
            name: "SPI_BUS".into(),
            direction: String::new(),
            position: Point::new(50.0, 10.0),
            rotation: 0.0,
            auto_generated: false,
            user_moved: false,
        }],
        instances: Vec::new(),
    });
    let mut engine = Engine::new(sheet).unwrap();
    engine
        .execute(Command::SetHarnessEntryType {
            sheet_pin_id: id(51),
            harness_type: Some("SPI_BUS".into()),
        })
        .unwrap();
    assert_eq!(
        engine.document().harness.entry_type(id(51)),
        Some("SPI_BUS")
    );
    let cleared = engine
        .execute(Command::SetHarnessEntryType {
            sheet_pin_id: id(51),
            harness_type: None,
        })
        .unwrap();
    assert!(cleared.changed);
    assert_eq!(engine.document().harness.entry_type(id(51)), None);
    assert!(matches!(
        engine.execute(Command::SetHarnessEntryType {
            sheet_pin_id: id(99),
            harness_type: Some("X".into()),
        }),
        Err(EngineError::HarnessNotFound { .. })
    ));
}

#[test]
fn a_connector_moves_turns_and_deletes_with_its_entries() {
    let mut engine = placed();
    let item = SelectedItem::new(id(1), SelectedKind::HarnessConnector);
    engine
        .execute(Command::MoveSelection {
            items: vec![item],
            dx: 2.54,
            dy: 0.0,
            keep_wires: false,
            wiring: signex_types::corner::DragWiringMode::Auto,
        })
        .unwrap();
    assert_eq!(
        engine.document().harness.connectors[0].position,
        Point::new(12.54, 10.0)
    );
    engine
        .execute(Command::RotateSelection {
            items: vec![item],
            angle_degrees: 90.0,
            keep_wires: false,
        })
        .unwrap();
    assert_ne!(engine.document().harness.connectors[0].rotation, 0.0);
    engine
        .execute(Command::DeleteSelection { items: vec![item] })
        .unwrap();
    assert!(engine.document().harness.connectors.is_empty());
}

#[test]
fn deleting_an_entry_closes_the_gap() {
    let mut engine = placed();
    engine
        .execute(Command::DeleteSelection {
            items: vec![SelectedItem::new(id(100), SelectedKind::HarnessEntry)],
        })
        .unwrap();
    let names: Vec<&str> = engine.document().harness.connectors[0]
        .entries
        .iter()
        .map(|e| e.name.as_str())
        .collect();
    assert_eq!(names, ["MOSI"]);
}

#[test]
fn a_locked_connector_refuses_edits_and_freezes_its_entries() {
    let mut engine = placed();
    let item = SelectedItem::new(id(1), SelectedKind::HarnessConnector);
    engine
        .execute(Command::SetItemLocked { item, locked: true })
        .unwrap();
    assert_eq!(engine.item_locked(&item), Some(true));
    let mut wider = connector(&["SCK", "MOSI"]);
    wider.width = 20.0;
    wider.locked = true;
    assert!(matches!(
        engine.execute(Command::UpdateHarnessConnector {
            uuid: id(1),
            connector: wider,
        }),
        Err(EngineError::ItemLocked { .. })
    ));
    assert!(matches!(
        engine.execute(Command::DeleteSelection {
            items: vec![SelectedItem::new(id(100), SelectedKind::HarnessEntry)],
        }),
        Err(EngineError::ItemLocked { .. })
    ));
}

#[test]
fn a_new_harness_id_already_in_the_sheet_is_refused() {
    let mut engine = placed();
    let mut wire = harness_wire();
    wire.uuid = id(100);
    assert!(matches!(
        engine.execute(Command::PlaceHarnessWire { wire }),
        Err(EngineError::IdInUse(taken)) if taken == id(100)
    ));
}

#[test]
fn properties_rows_name_the_type_and_entries() {
    let engine = placed();
    let details = engine
        .describe_single_selection(&[SelectedItem::new(id(1), SelectedKind::HarnessConnector)])
        .expect("details");
    assert!(
        details
            .info
            .iter()
            .any(|(k, v)| k == "Harness Type" && v == "SPI_BUS")
    );
    assert!(details.info.iter().any(|(k, v)| k == "Entries" && v == "2"));
}
