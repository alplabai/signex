//! A drawing's placement persists in `[extras.sheet.drawing_placements]`, and
//! only when it is `Above`: no entry means behind, so a file that never chose
//! a placement is byte-identical to what a build without the field wrote
//! (#1583).

use super::tests::empty_sheet;
use super::*;
use crate::schematic::{DrawingPlacement, FillType, Point, SchDrawing, SchematicSheet};

const DRAWING: &str = "0192a8c0-0003-7000-8000-000000000001";
const OTHER: &str = "0192a8c0-0003-7000-8000-000000000002";

fn load(text: &str) -> SchematicSheet {
    SnxSchematic::parse(text).unwrap().sheet
}

fn save(sheet: &SchematicSheet) -> String {
    SnxSchematic::new(sheet.clone()).write_string().unwrap()
}

fn rect(uuid: &str) -> SchDrawing {
    SchDrawing::Rect {
        uuid: Uuid::parse_str(uuid).unwrap(),
        start: Point::new(1.0, 1.0),
        end: Point::new(4.0, 3.0),
        width: 0.25,
        fill: FillType::None,
        stroke_color: None,
        locked: false,
    }
}

fn sheet_with_drawings() -> SchematicSheet {
    let mut sheet = empty_sheet();
    sheet.drawings = vec![rect(DRAWING), rect(OTHER)];
    sheet
}

#[test]
fn an_above_placement_round_trips_and_a_second_save_is_identical() {
    let mut sheet = sheet_with_drawings();
    sheet
        .drawing_placements
        .insert(Uuid::parse_str(DRAWING).unwrap(), DrawingPlacement::Above);

    let text = save(&sheet);
    assert!(text.contains("[extras.sheet.drawing_placements]"), "{text}");
    let again = load(&text);
    assert_eq!(again.drawing_placements, sheet.drawing_placements);
    assert_eq!(
        again.drawing_placement(Uuid::parse_str(OTHER).unwrap()),
        DrawingPlacement::Behind
    );
    assert_eq!(save(&again), text);
}

/// An old file, which has no placement table, loads as all-behind and is
/// written back unchanged.
#[test]
fn a_file_without_the_field_loads_behind_and_resaves_byte_identical() {
    let text = save(&sheet_with_drawings());
    assert!(!text.contains("drawing_placements"), "{text}");

    let sheet = load(&text);
    assert!(sheet.drawing_placements.is_empty());
    assert_eq!(save(&sheet), text);
}

/// Behind is the default and is never written, even when set explicitly.
#[test]
fn an_explicit_behind_writes_nothing() {
    let mut sheet = sheet_with_drawings();
    sheet
        .drawing_placements
        .insert(Uuid::parse_str(DRAWING).unwrap(), DrawingPlacement::Behind);

    assert_eq!(save(&sheet), save(&sheet_with_drawings()));
}

#[test]
fn an_entry_for_a_missing_drawing_is_dropped_on_load_and_save() {
    let ghost = "0192a8c0-0003-7000-8000-0000000000ff";
    let text = format!(
        "{}\n[extras.sheet.drawing_placements]\n{DRAWING} = \"above\"\n{ghost} = \"above\"\n\
         not-a-uuid = \"above\"\n{OTHER} = \"sideways\"\n",
        save(&sheet_with_drawings())
    );

    let sheet = load(&text);
    assert_eq!(sheet.drawing_placements.len(), 1);
    assert_eq!(
        sheet.drawing_placement(Uuid::parse_str(DRAWING).unwrap()),
        DrawingPlacement::Above
    );
    assert!(!save(&sheet).contains(ghost));
}
