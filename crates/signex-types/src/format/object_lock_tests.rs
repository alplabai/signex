//! The `locked` flag of wires, labels, text notes and drawings (#1573):
//! written only when set, so a file that never locked anything re-saves
//! byte-identical, and a lock survives save and load.

use super::tests::empty_sheet;
use super::*;
use crate::schematic::{
    FillType, HAlign, Label, LabelType, Point, SchDrawing, SchematicSheet, TextNote, VAlign, Wire,
};

fn load(text: &str) -> SchematicSheet {
    SnxSchematic::parse(text).unwrap().sheet
}

fn save(sheet: &SchematicSheet) -> String {
    SnxSchematic::new(sheet.clone()).write_string().unwrap()
}

fn wire(n: u128, locked: bool) -> Wire {
    Wire {
        uuid: Uuid::from_u128(n),
        start: Point::new(1.0, 2.0),
        end: Point::new(5.0, 2.0),
        locked,
        ..Default::default()
    }
}

fn label(n: u128, locked: bool) -> Label {
    Label {
        uuid: Uuid::from_u128(n),
        text: format!("NET{n}"),
        position: Point::new(1.0, 2.0),
        rotation: 0.0,
        label_type: LabelType::Net,
        shape: String::new(),
        font_size: 1.27,
        font_family: None,
        justify: HAlign::Left,
        justify_v: VAlign::Bottom,
        locked,
        unknown_columns: Default::default(),
    }
}

fn note(n: u128, locked: bool) -> TextNote {
    TextNote {
        uuid: Uuid::from_u128(n),
        text: "note".into(),
        position: Point::new(3.0, 4.0),
        rotation: 0.0,
        font_size: 1.27,
        font_family: None,
        justify_h: HAlign::Left,
        justify_v: VAlign::Bottom,
        locked,
    }
}

fn rect(n: u128, locked: bool) -> SchDrawing {
    SchDrawing::Rect {
        uuid: Uuid::from_u128(n),
        start: Point::new(1.0, 1.0),
        end: Point::new(4.0, 3.0),
        width: 0.25,
        fill: FillType::None,
        stroke_color: None,
        locked,
    }
}

fn sheet(locked: bool) -> SchematicSheet {
    let mut sheet = empty_sheet();
    sheet.wires = vec![wire(1, false), wire(2, locked)];
    sheet.labels = vec![label(3, false), label(4, locked)];
    sheet.text_notes = vec![note(5, false), note(6, locked)];
    sheet.drawings = vec![rect(7, false), rect(8, locked)];
    sheet
}

#[test]
fn a_sheet_that_locks_nothing_writes_no_lock_and_resaves_byte_identical() {
    let text = save(&sheet(false));
    assert!(!text.contains("locked"), "{text}");

    let reloaded = load(&text);
    assert!(reloaded.wires.iter().all(|w| !w.locked));
    assert!(reloaded.labels.iter().all(|l| !l.locked));
    assert!(reloaded.text_notes.iter().all(|n| !n.locked));
    assert!(reloaded.drawings.iter().all(|d| !d.is_locked()));
    assert_eq!(save(&reloaded), text);
}

#[test]
fn a_lock_survives_save_and_load_on_exactly_the_locked_rows() {
    let text = save(&sheet(true));
    let back = load(&text);

    assert_eq!(
        back.wires.iter().map(|w| w.locked).collect::<Vec<_>>(),
        [false, true]
    );
    assert_eq!(
        back.labels.iter().map(|l| l.locked).collect::<Vec<_>>(),
        [false, true]
    );
    assert_eq!(
        back.text_notes.iter().map(|n| n.locked).collect::<Vec<_>>(),
        [false, true]
    );
    assert_eq!(
        back.drawings
            .iter()
            .map(SchDrawing::is_locked)
            .collect::<Vec<_>>(),
        [false, true]
    );
    assert_eq!(save(&back), text, "a second save is identical");
}

/// The lock of a note or drawing lives beside the objects, so the typed
/// arrays carry no `locked` key an older build would drop.
#[test]
fn a_note_and_drawing_lock_is_written_beside_the_objects() {
    let text = save(&sheet(true));
    assert!(text.contains("locked_objects"), "{text}");
    let nested = text
        .lines()
        .filter(|line| line.trim_start().starts_with("locked = "))
        .count();
    assert_eq!(nested, 0, "{text}");
}

/// A lock entry whose object is gone is not carried into the next save.
#[test]
fn a_lock_entry_for_a_deleted_object_is_not_written_back() {
    let mut back = load(&save(&sheet(true)));
    let gone = back.drawings[1].uuid();
    back.drawings.remove(1);

    assert!(!save(&back).contains(&gone.to_string()));
}

/// A value other than `true` is not a lock this build wrote: it stays a
/// foreign cell and is written back as it was.
#[test]
fn a_lock_cell_that_is_not_true_is_kept_as_a_foreign_column() {
    let text = save(&sheet(true));
    let odd = text.replace(
        "true
", "TRUE
",
    );
    assert_ne!(odd, text, "the fixture has a lock cell to change");

    let back = load(&odd);
    assert!(back.wires.iter().all(|w| !w.locked));
    assert!(back.labels.iter().all(|l| !l.locked));
    let wire = &back.wires[1];
    assert_eq!(wire.unknown_columns.get("locked"), Some("TRUE"));
    assert_eq!(save(&back), odd, "the cell comes back as it was read");
}
