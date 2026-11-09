//! The optional `color` column of `[sheets.wires]`: absent while no wire is
//! coloured, `#rrggbbaa` on a coloured row, empty on the others, and an
//! unreadable value is kept as a foreign cell and reported.

use super::tests::empty_sheet;
use super::*;
use crate::schematic::{SchematicSheet, Wire};
use crate::theme::Color;

fn load(text: &str) -> Result<SchematicSheet, FormatError> {
    SnxSchematic::parse(text).map(|file| file.sheet)
}

fn save(sheet: &SchematicSheet) -> String {
    SnxSchematic::new(sheet.clone()).write_string().unwrap()
}

fn wire(n: u128, color: Option<Color>) -> Wire {
    Wire {
        uuid: Uuid::from_u128(n),
        color,
        ..Wire::default()
    }
}

fn sheet_of(wires: Vec<Wire>) -> SchematicSheet {
    let mut sheet = empty_sheet();
    sheet.wires = wires;
    sheet
}

#[test]
fn no_column_while_every_wire_is_uncoloured() {
    let text = save(&sheet_of(vec![wire(1, None), wire(2, None)]));
    assert!(!text.contains("color"), "{text}");
    let back = load(&text).unwrap();
    assert!(back.wires.iter().all(|w| w.color.is_none()));
    assert_eq!(save(&back), text);
}

#[test]
fn a_coloured_wire_round_trips_and_the_others_stay_uncoloured() {
    let red = Color::new(255, 0, 0, 255);
    let tint = Color::new(0x12, 0x34, 0x56, 0x78);
    let sheet = sheet_of(vec![wire(1, Some(red)), wire(2, None), wire(3, Some(tint))]);
    let text = save(&sheet);
    assert!(text.contains("#ff0000ff"), "{text}");
    assert!(text.contains("#12345678"), "{text}");

    let back = load(&text).unwrap();
    let colours: Vec<_> = back.wires.iter().map(|w| w.color).collect();
    assert_eq!(colours, [Some(red), None, Some(tint)]);
    assert!(
        back.wires.iter().all(|w| w.unknown_columns.is_empty()),
        "the colour cell is read, not kept as a foreign cell"
    );
    assert_eq!(save(&back), text, "a second save is byte-identical");
}

#[test]
fn an_unreadable_colour_loads_reports_and_round_trips() {
    let good = save(&sheet_of(vec![
        wire(1, None),
        wire(2, Some(Color::new(255, 0, 0, 255))),
    ]));
    let bad = good.replace("#ff0000ff", "#nothex");
    assert_ne!(bad, good);
    let mut sheet = load(&bad).expect("a cosmetic cell never fails the load");
    assert!(sheet.wires.iter().all(|w| w.color.is_none()));

    let repairs: Vec<_> = sheet
        .take_load_repairs()
        .iter()
        .map(SheetLoadRepair::describe)
        .collect();
    assert_eq!(repairs.len(), 1, "{repairs:?}");
    assert!(repairs[0].contains("wire #2"), "{repairs:?}");
    assert!(repairs[0].contains("#nothex"), "{repairs:?}");

    assert_eq!(save(&sheet), bad, "the original cell is written back");
}
