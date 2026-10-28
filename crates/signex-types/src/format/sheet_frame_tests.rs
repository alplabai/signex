//! Sheet frame settings persistence (#1134): stored in
//! `[extras.sheet.frame]` only when changed, so a sheet at the default
//! writes the bytes it always did.

use super::*;
use crate::schematic::{SchematicSheet, SheetFrameSettings};

fn write(sheet: &SchematicSheet) -> String {
    SnxSchematic::new(sheet.clone()).write_string().unwrap()
}

fn custom() -> SheetFrameSettings {
    SheetFrameSettings {
        show_border: false,
        show_title_block: false,
        horizontal_zones: 12,
        vertical_zones: 0,
    }
}

#[test]
fn a_sheet_at_the_default_frame_writes_no_frame_table() {
    let text = write(&SchematicSheet::default());
    assert!(!text.contains("frame"), "nothing to write:\n{text}");
    let back = SnxSchematic::parse(&text).unwrap().sheet;
    assert_eq!(back.frame, SheetFrameSettings::default());
    assert_eq!(write(&back), text);
}

#[test]
fn changed_frame_settings_round_trip() {
    let sheet = SchematicSheet {
        frame: custom(),
        ..Default::default()
    };
    let text = write(&sheet);
    assert!(text.contains("[extras.sheet.frame]"), "{text}");
    let back = SnxSchematic::parse(&text).unwrap().sheet;
    assert_eq!(back.frame, custom());
    assert_eq!(write(&back), text, "the encoding must be stable");
}

#[test]
fn returning_to_the_default_returns_the_file_to_its_default_bytes() {
    let default_bytes = write(&SchematicSheet::default());
    let mut sheet = SnxSchematic::parse(&write(&SchematicSheet {
        frame: custom(),
        ..Default::default()
    }))
    .unwrap()
    .sheet;
    sheet.frame = SheetFrameSettings::default();
    assert_eq!(write(&sheet), default_bytes);
}

#[test]
fn a_frame_table_naming_one_setting_keeps_the_rest_at_their_defaults() {
    let back = SnxSchematic::parse(&with_frame_table("show_border = false"))
        .unwrap()
        .sheet;
    assert_eq!(
        back.frame,
        SheetFrameSettings {
            show_border: false,
            ..SheetFrameSettings::default()
        }
    );
}

/// The `[extras.sheet.frame]` table of `text` replaced by `table`.
fn with_frame_table(table: &str) -> String {
    let text = write(&SchematicSheet {
        frame: custom(),
        ..Default::default()
    });
    let start = text.find("[extras.sheet.frame]").unwrap();
    format!("{}[extras.sheet.frame]\n{table}\n", &text[..start])
}

#[test]
fn only_the_settings_that_changed_are_written() {
    let text = write(&SchematicSheet {
        frame: SheetFrameSettings {
            show_border: false,
            ..SheetFrameSettings::default()
        },
        ..Default::default()
    });
    let table = &text[text.find("[extras.sheet.frame]").unwrap()..];
    assert_eq!(
        table.trim_end(),
        "[extras.sheet.frame]\nshow_border = false"
    );
}

/// A key a newer build wrote in the frame table survives a load and save
/// here (review of #1192: a frame margin is the likely next key).
#[test]
fn a_key_this_build_does_not_know_is_kept_on_save() {
    let text = with_frame_table("border_margin_mm = 15.0\nshow_border = false");
    let back = SnxSchematic::parse(&text).unwrap().sheet;
    assert!(!back.frame.show_border);
    let saved = write(&back);
    assert!(saved.contains("border_margin_mm = 15.0"), "{saved}");
    assert!(saved.contains("show_border = false"), "{saved}");
    assert_eq!(write(&SnxSchematic::parse(&saved).unwrap().sheet), saved);
}

/// A hand-edited or foreign value never stops the sheet from opening: out
/// of range clamps to `MAX_ZONES` (so the model, the picker and the drawing
/// agree), and a value of the wrong type reads as the default.
#[test]
fn out_of_range_and_wrongly_typed_values_open_leniently() {
    let text = with_frame_table(
        "horizontal_zones = 300\nvertical_zones = -4\nshow_border = \"no\"\nshow_title_block = 1",
    );
    let back = SnxSchematic::parse(&text).expect("the sheet opens").sheet;
    assert_eq!(
        back.frame,
        SheetFrameSettings {
            show_border: true,
            show_title_block: true,
            horizontal_zones: crate::schematic::MAX_ZONES,
            vertical_zones: 0,
        }
    );

    let text = with_frame_table("horizontal_zones = \"many\"\nvertical_zones = 200");
    let back = SnxSchematic::parse(&text).expect("the sheet opens").sheet;
    assert_eq!(back.frame.horizontal_zones, 8, "wrong type: the default");
    assert_eq!(back.frame.vertical_zones, crate::schematic::MAX_ZONES);
}
