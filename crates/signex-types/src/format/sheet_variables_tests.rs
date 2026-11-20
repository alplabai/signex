//! Sheet variables persist per sheet in `[extras.sheet.variables]` (#1987).

use super::tests::empty_sheet;
use super::*;
use crate::schematic::SchematicSheet;

fn load(text: &str) -> SchematicSheet {
    SnxSchematic::parse(text).unwrap().sheet
}

fn save(sheet: &SchematicSheet) -> String {
    SnxSchematic::new(sheet.clone()).write_string().unwrap()
}

fn sheet_with_variables() -> SchematicSheet {
    let mut sheet = empty_sheet();
    sheet.variables.insert("VCC".into(), "3.3V".into());
    sheet.variables.insert("VREF".into(), "= VCC / 2".into());
    sheet
        .variables
        .insert("NOTE".into(), "==literal \"quoted\"".into());
    sheet
}

#[test]
fn variables_round_trip_with_their_source_text() {
    let sheet = sheet_with_variables();
    let text = save(&sheet);
    assert!(text.contains("[extras.sheet.variables]"), "{text}");
    // The source is saved, never the evaluated value.
    assert!(text.contains("= VCC / 2"), "{text}");
    assert!(!text.contains("1.65"), "{text}");
    assert_eq!(load(&text).variables, sheet.variables);
    assert_eq!(save(&load(&text)), text, "a second save is byte-identical");
}

#[test]
fn a_sheet_without_variables_writes_no_table_and_resaves_byte_identical() {
    let text = save(&empty_sheet());
    assert!(!text.contains("variables"), "{text}");
    let sheet = load(&text);
    assert!(sheet.variables.is_empty());
    assert_eq!(save(&sheet), text);

    let mut titled = empty_sheet();
    titled.title_block.insert("title".into(), "T".into());
    let text = save(&titled);
    assert!(!text.contains("variables"), "{text}");
    assert_eq!(save(&load(&text)), text);
}

#[test]
fn variables_do_not_disturb_other_sheet_extras() {
    let mut sheet = sheet_with_variables();
    sheet.title_block.insert("title".into(), "= VCC".into());
    let again = load(&save(&sheet));
    assert_eq!(again.title_block, sheet.title_block);
    assert_eq!(again.variables, sheet.variables);
}
