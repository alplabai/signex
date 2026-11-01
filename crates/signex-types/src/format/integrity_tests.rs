//! Sheet-open integrity (#1830): a repeated object uuid is repaired and
//! reported, and a coordinate the writer could not save back is refused.

use std::collections::HashSet;

use super::tests::{empty_board, empty_sheet, sample_symbol};
use super::*;
use crate::schematic::{
    Bus, BusEntry, ChildSheet, HAlign, Junction, Label, LabelType, NoConnect, Point,
    SchematicSheet, SheetPin, TextNote, VAlign, Wire,
};

fn load(text: &str) -> SchematicSheet {
    SnxSchematic::parse(text).unwrap().sheet
}

fn save(sheet: &SchematicSheet) -> String {
    SnxSchematic::new(sheet.clone()).write_string().unwrap()
}

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

fn wire(n: u128, end_x: f64) -> Wire {
    Wire {
        uuid: id(n),
        start: Point::new(1.0, 2.0),
        end: Point::new(end_x, 2.0),
        ..Default::default()
    }
}

fn label(n: u128, text: &str) -> Label {
    Label {
        uuid: id(n),
        text: text.into(),
        position: Point::new(1.0, 2.0),
        rotation: 0.0,
        label_type: LabelType::Net,
        shape: String::new(),
        font_size: 1.27,
        font_family: None,
        justify: HAlign::Left,
        justify_v: VAlign::Bottom,
        locked: false,
        unknown_columns: Default::default(),
    }
}

fn symbol(n: u128, reference: &str) -> crate::schematic::Symbol {
    let mut s = sample_symbol();
    s.uuid = id(n);
    s.reference = reference.into();
    s.footprint.clear();
    s
}

/// Every uuid the sheet's objects carry, as one list.
fn all_ids(sheet: &SchematicSheet) -> Vec<Uuid> {
    let mut ids: Vec<Uuid> = sheet.symbols.iter().map(|s| s.uuid).collect();
    ids.extend(
        sheet
            .symbols
            .iter()
            .flat_map(|s| s.pin_uuids.values().copied()),
    );
    ids.extend(sheet.wires.iter().map(|w| w.uuid));
    ids.extend(sheet.junctions.iter().map(|j| j.uuid));
    ids.extend(sheet.labels.iter().map(|l| l.uuid));
    ids.extend(sheet.child_sheets.iter().map(|c| c.uuid));
    ids.extend(
        sheet
            .child_sheets
            .iter()
            .flat_map(|c| c.pins.iter().map(|p| p.uuid)),
    );
    ids.extend(sheet.no_connects.iter().map(|n| n.uuid));
    ids.extend(sheet.text_notes.iter().map(|t| t.uuid));
    ids.extend(sheet.buses.iter().map(|b| b.uuid));
    ids.extend(sheet.bus_entries.iter().map(|b| b.uuid));
    ids.extend(sheet.drawings.iter().map(|d| d.uuid()));
    ids.extend(sheet.no_erc_directives.iter().map(|n| n.uuid));
    ids
}

fn assert_unique(sheet: &SchematicSheet) {
    let ids = all_ids(sheet);
    assert_eq!(
        ids.len(),
        ids.iter().collect::<HashSet<_>>().len(),
        "{ids:?}"
    );
}

#[test]
fn a_clean_sheet_reports_no_repair() {
    let mut sheet = empty_sheet();
    sheet.symbols.push(symbol(1, "U1"));
    sheet.wires.push(wire(2, 5.0));
    sheet.labels.push(label(3, "VCC"));
    let loaded = load(&save(&sheet));
    assert!(loaded.unknown.load_repairs.is_empty());
    assert_eq!(all_ids(&loaded), all_ids(&sheet));
}

#[test]
fn an_identical_duplicate_wire_row_is_dropped_with_a_message() {
    let mut sheet = empty_sheet();
    sheet.wires = vec![wire(7, 5.0), wire(7, 5.0)];
    let loaded = load(&save(&sheet));
    assert_eq!(loaded.wires.len(), 1);
    assert_eq!(loaded.wires[0].uuid, id(7));
    assert_eq!(loaded.unknown.load_repairs.len(), 1);
    let message = &loaded.unknown.load_repairs[0];
    assert!(message.starts_with("wire (row 2)"), "{message}");
    assert!(message.contains(&id(7).to_string()), "{message}");
    assert!(message.contains("dropped"), "{message}");
}

#[test]
fn a_wire_with_a_taken_uuid_and_other_geometry_gets_a_fresh_uuid() {
    let mut sheet = empty_sheet();
    sheet.wires = vec![wire(7, 5.0), wire(7, 9.0)];
    let loaded = load(&save(&sheet));
    assert_eq!(loaded.wires.len(), 2);
    assert_eq!(loaded.wires[0].uuid, id(7), "the first holder keeps its id");
    assert_ne!(loaded.wires[1].uuid, id(7));
    assert_eq!(
        loaded.wires[1].end.x, 9.0,
        "the repaired wire keeps its geometry"
    );
    assert_eq!(loaded.wires[1].uuid.get_version_num(), 7);
    assert_unique(&loaded);
    assert_eq!(loaded.unknown.load_repairs.len(), 1);
    assert!(loaded.unknown.load_repairs[0].contains("new uuid"));
}

#[test]
fn duplicate_component_rows_are_repaired_and_keep_their_extras() {
    let mut sheet = empty_sheet();
    let mut first = symbol(1, "U1");
    first.unit = 2;
    sheet.symbols = vec![first, symbol(1, "U2")];
    let loaded = load(&save(&sheet));
    assert_eq!(loaded.symbols.len(), 2);
    assert_unique(&loaded);
    assert_eq!(loaded.symbols[0].uuid, id(1));
    assert_eq!(loaded.symbols[1].reference, "U2");
    let message = &loaded.unknown.load_repairs[0];
    assert!(message.contains("symbol U2"), "{message}");
}

#[test]
fn an_exact_duplicate_component_row_is_dropped() {
    let mut sheet = empty_sheet();
    sheet.symbols = vec![symbol(1, "U1"), symbol(1, "U1")];
    let loaded = load(&save(&sheet));
    assert_eq!(loaded.symbols.len(), 1);
    assert!(loaded.unknown.load_repairs[0].contains("dropped"));
}

#[test]
fn a_uuid_shared_by_two_kinds_is_repaired_on_the_later_kind() {
    let mut sheet = empty_sheet();
    sheet.wires = vec![wire(5, 5.0)];
    sheet.labels = vec![label(5, "VCC")];
    sheet.junctions = vec![Junction {
        uuid: id(5),
        position: Point::new(1.0, 2.0),
        ..Default::default()
    }];
    let loaded = load(&save(&sheet));
    assert_eq!(loaded.wires[0].uuid, id(5));
    assert_unique(&loaded);
    assert_eq!(loaded.unknown.load_repairs.len(), 2);
    assert!(
        loaded
            .unknown
            .load_repairs
            .iter()
            .any(|m| m.contains("label VCC"))
    );
}

#[test]
fn every_uuid_kind_is_repaired() {
    let mut sheet = empty_sheet();
    let origin = Point::new(0.0, 0.0);
    sheet.symbols = vec![symbol(9, "U1")];
    sheet.wires = vec![wire(9, 5.0)];
    sheet.junctions = vec![Junction {
        uuid: id(9),
        position: origin,
        ..Default::default()
    }];
    sheet.labels = vec![label(9, "A")];
    sheet.child_sheets = vec![ChildSheet {
        uuid: id(9),
        name: "child".into(),
        filename: "child.snxsch".into(),
        position: origin,
        size: (10.0, 10.0),
        stroke_width: 0.0,
        fill: Default::default(),
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins: vec![SheetPin {
            uuid: id(9),
            name: "P".into(),
            direction: String::new(),
            position: origin,
            rotation: 0.0,
            auto_generated: false,
            user_moved: false,
        }],
        instances: vec![],
    }];
    sheet.no_connects = vec![NoConnect {
        uuid: id(9),
        position: origin,
    }];
    sheet.text_notes = vec![TextNote {
        uuid: id(9),
        text: "note".into(),
        position: origin,
        rotation: 0.0,
        ..sample_note()
    }];
    sheet.buses = vec![Bus {
        uuid: id(9),
        start: origin,
        end: Point::new(5.0, 0.0),
    }];
    sheet.bus_entries = vec![BusEntry {
        uuid: id(9),
        position: origin,
        size: (2.0, 2.0),
    }];
    sheet.no_erc_directives = vec![NoConnect {
        uuid: id(9),
        position: origin,
    }];
    let loaded = load(&save(&sheet));
    assert_unique(&loaded);
    // All but the first holder (the symbol) were re-minted: ten more objects.
    assert_eq!(
        loaded.unknown.load_repairs.len(),
        10,
        "{:?}",
        loaded.unknown.load_repairs
    );
    assert_eq!(loaded.symbols[0].uuid, id(9));
}

fn sample_note() -> TextNote {
    serde_json::from_value(serde_json::json!({
        "uuid": id(0),
        "text": "",
        "position": { "x": 0.0, "y": 0.0 },
    }))
    .unwrap()
}

#[test]
fn a_repeated_pin_uuid_is_repaired() {
    let mut sheet = empty_sheet();
    let mut a = symbol(1, "U1");
    a.pin_uuids.insert("1".into(), id(50));
    let mut b = symbol(2, "U2");
    b.pin_uuids.insert("1".into(), id(50));
    sheet.symbols = vec![a, b];
    let loaded = load(&save(&sheet));
    assert_unique(&loaded);
    assert_eq!(loaded.symbols[0].pin_uuids["1"], id(50));
    assert!(loaded.unknown.load_repairs[0].contains("pin 1 of symbol U2"));
}

#[test]
fn a_repaired_sheet_saves_clean_and_reloads_without_repair() {
    let mut sheet = empty_sheet();
    sheet.wires = vec![wire(7, 5.0), wire(7, 9.0), wire(7, 5.0)];
    let loaded = load(&save(&sheet));
    assert_eq!(loaded.wires.len(), 2);
    let again = load(&save(&loaded));
    assert!(again.unknown.load_repairs.is_empty());
    assert_eq!(all_ids(&again), all_ids(&loaded));
}

/// A saved one-wire file whose wire runs from 1 mm to 5 mm, with the end
/// column's `5000000` replaced by `end_nm`.
fn wire_file_with_end(end_nm: &str) -> String {
    let mut sheet = empty_sheet();
    sheet.wires = vec![wire(7, 5.0)];
    save(&sheet).replacen("5000000", end_nm, 1)
}

#[test]
fn a_coordinate_at_the_i64_ends_is_refused_with_its_field() {
    for nm in ["9223372036854775807", "-9223372036854775808"] {
        let err = SnxSchematic::parse(&wire_file_with_end(nm)).unwrap_err();
        assert!(
            matches!(&err, FormatError::CoordinateOutOfRange { field, row: 0, .. } if field == "end_x"),
            "{nm}: {err}"
        );
        let text = err.to_string();
        assert!(
            text.contains("sheets.wires") && text.contains("end_x"),
            "{text}"
        );
    }
}

#[test]
fn a_coordinate_that_round_trips_still_opens() {
    for nm in ["9000000000000000000", "-9000000000000000000", "0"] {
        let loaded = load(&wire_file_with_end(nm));
        let resaved = save(&loaded);
        assert!(resaved.contains(nm), "{nm} did not survive:\n{resaved}");
    }
}

#[test]
fn a_pcb_coordinate_at_the_i64_end_is_refused() {
    let mut board = empty_board();
    board.segments.push(crate::pcb::Segment {
        uuid: id(3),
        start: crate::pcb::Point { x: 1.0, y: 2.0 },
        end: crate::pcb::Point { x: 5.0, y: 2.0 },
        width: 0.25,
        layer: "F.Cu".into(),
        net: 0,
        unknown_columns: Default::default(),
    });
    let text = SnxPcb::new(board).write_string().unwrap();
    let bad = text.replacen("5000000", "9223372036854775807", 1);
    let err = SnxPcb::parse(&bad).unwrap_err();
    assert!(
        matches!(err, FormatError::CoordinateOutOfRange { .. }),
        "{err}"
    );
}
