//! #2059 part C: the Document Options pick lists read as the `Display`
//! text the types gave them before, and pick by value.

use super::*;
use crate::choice::chosen;
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

/// Mockup row 40: the Canvas Font field reads the font and its size in
/// points ("Iosevka · 10 pt", not "Iosevka, 10px").
#[test]
fn the_canvas_font_field_reads_the_family_and_its_points() {
    let summary = canvas_font_summary(&en(), "Iosevka", 10.0).into_string();
    assert_eq!(
        summary.replace(['\u{2068}', '\u{2069}'], ""),
        "Iosevka · 10 pt"
    );
    let rounded = canvas_font_summary(&en(), "Iosevka", 10.6).into_string();
    assert!(rounded.contains("11"), "{rounded:?}");
}

/// Mockup row 40, as laid out: the field shows the summary, no px size,
/// and a press on it opens the font settings.
#[test]
fn the_canvas_font_field_draws_its_summary_and_opens_the_settings() {
    use crate::layout_probe::{lay_out_texts, messages_on_click};
    let window = iced::Size::new(320.0, 200.0);
    let row = || {
        form_font_link_row(
            t!(en(), ids::PROPERTIES_CANVAS_FONT),
            canvas_font_summary(&en(), "Iosevka", 11.0),
            signex_types::theme::ThemeId::GraphiteLight,
            iced::Color::BLACK,
        )
    };
    let summary = canvas_font_summary(&en(), "Iosevka", 11.0).into_string();
    let texts: Vec<String> = lay_out_texts(row(), window)
        .into_iter()
        .map(|t| t.text)
        .collect();
    assert!(texts.contains(&summary), "{texts:?}");
    assert!(!texts.iter().any(|t| t.ends_with("px")), "{texts:?}");
    let pressed = messages_on_click(row(), window, &summary);
    assert!(
        matches!(pressed.as_slice(), [PanelMsg::OpenCanvasFontPopup]),
        "{pressed:?}"
    );
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
