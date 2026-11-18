//! #2059 part C: the Document Options pick lists read as the `Display`
//! text the types gave them before, and pick by value.

use super::*;
use crate::panels::{SchematicSheetColor, SheetColor};
use signex_types::schematic::PageOrigin;

fn en() -> std::sync::Arc<I18n> {
    I18n::en_us()
}

#[test]
fn each_orientation_keeps_its_english_name() {
    let all = [SheetOrientation::Landscape, SheetOrientation::Portrait];
    let choices = Choice::named(&all, orientation_label, &en());
    for orientation in all {
        let english = orientation.to_string();
        assert_eq!(
            en().message_plain(orientation_label(orientation), &[]),
            english
        );
        assert_eq!(
            chosen(&choices, &orientation).map(|c| c.label),
            Some(english)
        );
    }
}

#[test]
fn each_origin_keeps_its_english_name() {
    let all = [PageOrigin::UpperLeft, PageOrigin::LowerLeft];
    let choices = Choice::named(&all, origin_label, &en());
    for origin in all {
        let english = origin.to_string();
        assert_eq!(en().message_plain(origin_label(origin), &[]), english);
        assert_eq!(chosen(&choices, &origin).map(|c| c.label), Some(english));
    }
}

#[test]
fn each_sheet_colour_keeps_its_english_name() {
    let custom =
        SchematicSheetColor::Custom(signex_types::theme::Color::new(0x12, 0x34, 0x56, 0xFF));
    let all: Vec<SchematicSheetColor> = SchematicSheetColor::options()
        .into_iter()
        .chain([custom])
        .collect();
    assert!(all.contains(&SchematicSheetColor::Preset(SheetColor::Cream)));
    let choices: Vec<Choice<SchematicSheetColor>> =
        all.iter().map(|&c| sheet_color_choice(&en(), c)).collect();
    for color in all {
        let english = color.to_string();
        assert_eq!(sheet_color_choice(&en(), color).label, english, "{color:?}");
        assert_eq!(chosen(&choices, &color).map(|c| c.label), Some(english));
    }
}
