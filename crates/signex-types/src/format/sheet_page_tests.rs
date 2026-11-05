//! Sheet page settings persistence (#2034): paper colour and coordinate
//! origin, stored in `[extras.sheet.page]` only when changed, so a sheet at
//! the default writes the bytes it always did.

use super::*;
use crate::schematic::{PageOrigin, SchematicSheet, SheetPageSettings};
use crate::theme::Color;

fn write(sheet: &SchematicSheet) -> String {
    SnxSchematic::new(sheet.clone()).write_string().unwrap()
}

fn custom() -> SheetPageSettings {
    SheetPageSettings {
        color: Some(Color::new(0xfb, 0xf4, 0xe0, 0xff)),
        origin: PageOrigin::UpperLeft,
    }
}

fn parse(text: &str) -> SchematicSheet {
    SnxSchematic::parse(text).expect("the sheet opens").sheet
}

#[test]
fn a_sheet_at_the_default_page_writes_no_page_table() {
    let text = write(&SchematicSheet::default());
    assert!(!text.contains("extras.sheet.page"), "{text}");
    let back = parse(&text);
    assert_eq!(back.page, SheetPageSettings::default());
    assert_eq!(back.page.origin, PageOrigin::LowerLeft);
    assert_eq!(back.page.color, None, "absent = the theme's paper");
    assert_eq!(write(&back), text);
}

#[test]
fn changed_page_settings_round_trip() {
    let text = write(&SchematicSheet {
        page: custom(),
        ..Default::default()
    });
    let table = &text[text.find("[extras.sheet.page]").expect(&text)..];
    assert!(table.contains("color = \"#fbf4e0ff\""), "{table}");
    assert!(table.contains("origin = \"upper_left\""), "{table}");
    let back = parse(&text);
    assert_eq!(back.page, custom());
    assert_eq!(write(&back), text, "the encoding must be stable");
}

/// A file written before the page table existed loads at the defaults and
/// re-saves byte-identical.
#[test]
fn an_old_file_without_the_table_loads_unchanged() {
    let old = write(&SchematicSheet::default());
    let back = parse(&old);
    assert!(back.page.is_default());
    assert_eq!(write(&back), old);
}

/// The `[extras.sheet.page]` table of `text` replaced by `table`.
fn with_page_table(table: &str) -> String {
    let text = write(&SchematicSheet {
        page: custom(),
        ..Default::default()
    });
    let start = text.find("[extras.sheet.page]").unwrap();
    format!("{}[extras.sheet.page]\n{table}\n", &text[..start])
}

#[test]
fn unknown_page_keys_are_kept_and_bad_values_open_leniently() {
    let back = parse(&with_page_table(
        "color = \"cream\"\norigin = 7\npaper_grain = \"long\"",
    ));
    assert!(back.page.is_default(), "bad values read as the default");
    let saved = write(&back);
    assert!(saved.contains("paper_grain = \"long\""), "{saved}");
    assert_eq!(write(&parse(&saved)), saved);
}
