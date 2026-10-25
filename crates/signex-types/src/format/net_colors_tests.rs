//! Net colour overrides persist per sheet in `[extras.sheet.net_colors]`.

use super::tests::empty_sheet;
use super::*;
use crate::schematic::SchematicSheet;
use crate::theme::Color;

const WIRE: &str = "0192a8c0-0002-7000-8000-000000000001";
const JUNCTION: &str = "0192a8c0-0002-7000-8000-000000000002";

fn load(text: &str) -> SchematicSheet {
    SnxSchematic::parse(text).unwrap().sheet
}

fn save(sheet: &SchematicSheet) -> String {
    SnxSchematic::new(sheet.clone()).write_string().unwrap()
}

fn coloured_sheet() -> SchematicSheet {
    let mut sheet = empty_sheet();
    sheet
        .net_colors
        .insert(Uuid::parse_str(WIRE).unwrap(), Color::new(255, 0, 0, 255));
    sheet.net_colors.insert(
        Uuid::parse_str(JUNCTION).unwrap(),
        Color::new(0x12, 0x34, 0x56, 0x78),
    );
    sheet
}

#[test]
fn net_colors_round_trip() {
    let sheet = coloured_sheet();
    let text = save(&sheet);
    assert!(text.contains("[extras.sheet.net_colors]"), "{text}");
    assert_eq!(load(&text).net_colors, sheet.net_colors);
    assert_eq!(save(&load(&text)), text, "a second save is byte-identical");
}

#[test]
fn file_without_the_field_loads_as_empty_and_resaves_byte_identical() {
    let text = save(&empty_sheet());
    assert!(!text.contains("net_colors"));
    let sheet = load(&text);
    assert!(sheet.net_colors.is_empty());
    assert_eq!(save(&sheet), text);
}

#[test]
fn other_sheet_extras_without_colours_write_no_net_colors_table() {
    let mut sheet = empty_sheet();
    sheet.title_block.insert("title".into(), "T".into());
    let text = save(&sheet);
    assert!(!text.contains("net_colors"), "{text}");
    assert_eq!(save(&load(&text)), text);
}

#[test]
fn colours_do_not_disturb_other_sheet_extras() {
    let mut sheet = coloured_sheet();
    sheet.title_block.insert("title".into(), "T".into());
    let again = load(&save(&sheet));
    assert_eq!(again.title_block, sheet.title_block);
    assert_eq!(again.net_colors, sheet.net_colors);
}

#[test]
fn malformed_entries_are_dropped_and_the_rest_load() {
    let text = format!(
        "{}\n[extras.sheet.net_colors]\n{WIRE} = \"#ff0000\"\nnot-a-uuid = \"#00ff00\"\n\
         {JUNCTION} = \"red\"\n",
        save(&empty_sheet())
    );
    let sheet = load(&text);
    assert_eq!(sheet.net_colors.len(), 1);
    assert_eq!(
        sheet.net_colors[&Uuid::parse_str(WIRE).unwrap()],
        Color::new(255, 0, 0, 255)
    );
}

#[test]
fn colour_hex_round_trips_and_rejects_bad_input() {
    let c = Color::new(1, 171, 255, 128);
    assert_eq!(Color::parse_hex(&c.to_hex()), Some(c));
    assert_eq!(
        Color::parse_hex("#102030"),
        Some(Color::new(16, 32, 48, 255))
    );
    for bad in [
        "", "#", "102030", "#12345", "#1234567", "#gg0000", "#é00000",
    ] {
        assert_eq!(Color::parse_hex(bad), None, "{bad:?}");
    }
}
