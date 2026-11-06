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

    // A colour alone writes no origin line: Lower Left is the default.
    let colour_only = SheetPageSettings {
        origin: PageOrigin::LowerLeft,
        ..custom()
    };
    let text = write(&SchematicSheet {
        page: colour_only,
        ..Default::default()
    });
    let table = &text[text.find("[extras.sheet.page]").expect(&text)..];
    assert!(table.contains("color = "), "{table}");
    assert!(!table.contains("origin"), "{table}");
    assert_eq!(parse(&text).page, colour_only);
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

/// Review of #2034: an unknown key and a value this build cannot read (a
/// newer build's colour name or origin) open as the default and are written
/// back unchanged; a value the user sets replaces the raw one.
#[test]
fn unknown_keys_and_unreadable_values_are_kept_raw() {
    let back = parse(&with_page_table(
        "color = \"cream\"\norigin = 7\npaper_grain = \"long\"",
    ));
    assert!(
        back.page.is_default(),
        "unreadable values show as the default"
    );
    let saved = write(&back);
    for kept in ["color = \"cream\"", "origin = 7", "paper_grain = \"long\""] {
        assert!(saved.contains(kept), "{kept} kept:\n{saved}");
    }
    assert_eq!(write(&parse(&saved)), saved);

    let mut picked = parse(&saved);
    // A value set in the model (an engine pick also drops the raw key; the
    // writer never writes the key twice either way).
    picked.page.origin = PageOrigin::UpperLeft;
    let saved = write(&picked);
    assert!(saved.contains("origin = \"upper_left\""), "{saved}");
    assert!(!saved.contains("origin = 7"), "{saved}");
    assert!(saved.contains("color = \"cream\""), "{saved}");
}

/// Review of #2034: a `page` value that is not a table opens the sheet, is
/// written back, and gives way to a page table once a value is set.
#[test]
fn a_page_value_that_is_not_a_table_opens_and_is_kept() {
    // A default font makes the writer emit an `[extras.sheet]` table.
    let text = write(&SchematicSheet {
        font_family: Some("Roboto".into()),
        ..Default::default()
    });
    let start = text.find("[extras.sheet]").expect("an extras.sheet table");
    let end = start + "[extras.sheet]".len();
    let text = format!("{}\npage = 5{}", &text[..end], &text[end..]);
    let back = parse(&text);
    assert!(back.page.is_default());
    let saved = write(&back);
    assert!(saved.contains("page = 5"), "{saved}");
    assert_eq!(write(&parse(&saved)), saved);

    let mut picked = parse(&saved);
    picked.page.origin = PageOrigin::UpperLeft;
    let saved = write(&picked);
    assert!(!saved.contains("page = 5"), "{saved}");
    assert_eq!(parse(&saved).page.origin, PageOrigin::UpperLeft);
}
