//! Stored text widths (#2029): `[extras.sheet.text_extents]` round-trips, a
//! sheet without any is byte-identical to before, and an older build keeps
//! the table.

use super::*;
use crate::schematic::{
    HAlign, Label, LabelType, Point, SchematicSheet, TextExtents, TextField, VAlign,
};

fn label(n: u128, text: &str) -> Label {
    Label {
        uuid: Uuid::from_u128(n),
        text: text.into(),
        position: Point { x: 1.0, y: 2.0 },
        rotation: 0.0,
        label_type: LabelType::Net,
        shape: String::new(),
        font_size: 1.27,
        font_family: Some("Fira Mono".into()),
        justify: HAlign::Left,
        justify_v: VAlign::Bottom,
        unknown_columns: Default::default(),
        locked: false,
    }
}

fn plain_sheet() -> SchematicSheet {
    SchematicSheet {
        labels: vec![label(1, "NET1"), label(2, "NET2")],
        ..Default::default()
    }
}

fn with_extents() -> SchematicSheet {
    let mut sheet = plain_sheet();
    sheet.text_extents = TextExtents::default().refreshed(&sheet, |p| {
        Some(if p.id == Uuid::from_u128(1) {
            12.345678
        } else {
            3.0
        })
    });
    sheet
}

fn write(sheet: &SchematicSheet) -> String {
    SnxSchematic::new(sheet.clone()).write_string().unwrap()
}

fn load(text: &str) -> SchematicSheet {
    SnxSchematic::parse(text).unwrap().sheet
}

#[test]
fn a_sheet_without_extents_writes_no_table() {
    let text = write(&plain_sheet());
    assert!(!text.contains("text_extents"), "{text}");
    assert_eq!(write(&load(&text)), text);
}

#[test]
fn extents_round_trip_and_the_file_is_stable() {
    let sheet = with_extents();
    let text = write(&sheet);
    assert!(text.contains("[extras.sheet.text_extents."), "{text}");
    assert!(text.contains("width_nm = 12345678"), "{text}");
    let loaded = load(&text);
    assert_eq!(loaded.text_extents, sheet.text_extents);
    assert_eq!(write(&loaded), text);
}

#[test]
fn a_record_whose_inputs_changed_or_whose_item_is_gone_is_not_written() {
    let mut sheet = with_extents();
    sheet.labels[0].text = "RENAMED".into();
    sheet.labels.pop();
    let text = write(&sheet);
    assert!(!text.contains("text_extents"), "{text}");
}

#[test]
fn unreadable_entries_are_dropped_on_load() {
    let ghost = Uuid::from_u128(9);
    let text = write(&with_extents()).replacen(
        "[extras.sheet.text_extents.",
        &format!(
            "[extras.sheet.text_extents.\"not-a-key\"]\nwidth_nm = 1\nsize_nm = 1\ntext = \"x\"\n\n\
             [extras.sheet.text_extents.\"{ghost}:bogus\"]\nwidth_nm = 1\nsize_nm = 1\ntext = \"x\"\n\n\
             [extras.sheet.text_extents."
        ),
        1,
    );
    let loaded = load(&text);
    assert_eq!(loaded.text_extents, with_extents().text_extents);
}

/// What `[extras.sheet]` is to a build that predates the table: unknown keys
/// kept in `UnknownFields` and written back (builds since #658).
#[derive(serde::Serialize, serde::Deserialize)]
struct OlderBuildSheetExtras {
    #[serde(flatten)]
    unknown: unknown::UnknownFields,
}

#[test]
fn an_older_build_that_keeps_unknown_keys_keeps_the_nested_table() {
    let text = write(&with_extents());
    let root: toml::Table = toml::from_str(&text).unwrap();
    let sheet_table = root["extras"]["sheet"].clone();
    let older: OlderBuildSheetExtras = sheet_table.clone().try_into().unwrap();
    let resaved = toml::Value::try_from(&older).unwrap();
    assert_eq!(resaved["text_extents"], sheet_table["text_extents"]);
    assert!(resaved["text_extents"].is_table());
}

#[test]
fn the_field_names_in_keys_are_label_note_ref_value() {
    let names: Vec<_> = [
        TextField::Label,
        TextField::Note,
        TextField::SymbolReference,
        TextField::SymbolValue,
    ]
    .map(|f| f.file_key())
    .to_vec();
    assert_eq!(names, ["label", "note", "ref", "value"]);
}
