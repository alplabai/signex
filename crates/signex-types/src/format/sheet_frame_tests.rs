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
    let text = write(&SchematicSheet {
        frame: custom(),
        ..Default::default()
    });
    let start = text.find("[extras.sheet.frame]").unwrap();
    let table_end = text[start..].find("\n\n").map_or(text.len(), |i| start + i);
    let trimmed = format!(
        "{}[extras.sheet.frame]\nshow_border = false{}",
        &text[..start],
        &text[table_end..]
    );
    let back = SnxSchematic::parse(&trimmed).unwrap().sheet;
    assert_eq!(
        back.frame,
        SheetFrameSettings {
            show_border: false,
            ..SheetFrameSettings::default()
        }
    );
}
