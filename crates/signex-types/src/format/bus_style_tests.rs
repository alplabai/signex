//! A bus row's `stroke_width` and `color` keys (#2813): omitted while unset,
//! round-tripped byte for byte, and written under `snxsch/1.4`.

use super::tests::empty_sheet;
use super::*;
use crate::schematic::{Bus, Point, SchematicSheet};
use crate::theme::Color;

const TINT: Color = Color::new(0x12, 0x34, 0x56, 0x78);

fn bus(n: u128, stroke_width: f64, color: Option<Color>) -> Bus {
    Bus {
        uuid: Uuid::from_u128(n),
        start: Point::new(0.0, 0.0),
        end: Point::new(10.0, 0.0),
        stroke_width,
        color,
    }
}

fn sheet_of(buses: Vec<Bus>) -> SchematicSheet {
    let mut sheet = empty_sheet();
    sheet.buses = buses;
    sheet
}

fn save(sheet: &SchematicSheet) -> String {
    SnxSchematic::new(sheet.clone()).write_string().unwrap()
}

fn load(text: &str) -> Result<SchematicSheet, FormatError> {
    SnxSchematic::parse(text).map(|file| file.sheet)
}

fn starts_with_token(text: &str, token: &str) -> bool {
    text.starts_with(&format!("format = \"{token}\""))
}

#[test]
fn an_unstyled_bus_writes_no_style_key_and_stays_at_the_low_token() {
    let text = save(&sheet_of(vec![bus(1, 0.0, None)]));
    assert!(starts_with_token(&text, SNXSCH_FORMAT_V1), "{text}");
    // Other tables have these columns too, so compare with a sheet with no bus.
    let without = save(&sheet_of(Vec::new()));
    for key in ["stroke_width", "color"] {
        assert_eq!(text.matches(key).count(), without.matches(key).count(), "{text}");
    }
    assert_eq!(save(&load(&text).unwrap()), text);
}

#[test]
fn a_styled_bus_round_trips_byte_identical_under_1_4() {
    let sheet = sheet_of(vec![bus(1, 1.0, Some(TINT)), bus(2, 0.0, None)]);
    let text = save(&sheet);
    assert!(starts_with_token(&text, SNXSCH_FORMAT_BUS_STYLE), "{text}");
    assert!(text.contains("#12345678"), "{text}");

    let back = load(&text).unwrap();
    assert_eq!(back.buses, sheet.buses);
    assert_eq!(save(&back), text, "a second save is byte-identical");
}

#[test]
fn width_alone_or_colour_alone_needs_1_4() {
    for styled in [bus(1, 0.5, None), bus(1, 0.0, Some(TINT))] {
        let text = save(&sheet_of(vec![styled.clone()]));
        assert!(starts_with_token(&text, "snxsch/1.4"), "{text}");
        assert_eq!(load(&text).unwrap().buses, [styled]);
    }
}

#[test]
fn clearing_the_last_bus_style_lowers_the_token() {
    let mut sheet = sheet_of(vec![bus(1, 1.0, Some(TINT))]);
    assert!(starts_with_token(&save(&sheet), SNXSCH_FORMAT_BUS_STYLE));
    sheet.buses[0].stroke_width = 0.0;
    sheet.buses[0].color = None;
    assert!(starts_with_token(&save(&sheet), SNXSCH_FORMAT_V1));
}

/// A 1.3 build drops keys it does not know from a typed bus row, so it must
/// see this token as a newer minor and refuse the save.
#[test]
fn a_1_3_build_opens_a_styled_bus_read_only() {
    let one_three = FormatVersion {
        current: SNXSCH_FORMAT_REFINEMENT,
        readable: &[
            "snxsch/1",
            SNXSCH_FORMAT_V1,
            SNXSCH_FORMAT_NO_ERC_RULES,
            SNXSCH_FORMAT_REFINEMENT,
        ],
        ..SNXSCH_VERSION
    };
    assert!(one_three.is_newer_minor(SNXSCH_FORMAT_BUS_STYLE));
    assert!(one_three.refuse_save(SNXSCH_FORMAT_BUS_STYLE).is_err());
}

#[test]
fn an_unreadable_bus_colour_refuses_the_file_and_names_the_value() {
    let good = save(&sheet_of(vec![bus(1, 0.0, Some(TINT))]));
    let bad = good.replace("#12345678", "#nothex");
    assert_ne!(bad, good);
    let error = load(&bad).expect_err("nothing may be dropped silently");
    assert!(error.to_string().contains("#nothex"), "{error}");
}
