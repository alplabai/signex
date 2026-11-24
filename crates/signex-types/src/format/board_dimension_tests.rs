//! #2868 — the look keys of a `[[free.dimensions]]` row: written only where
//! they differ from the look before the settings, read back whole.

use uuid::Uuid;

use super::tests::empty_board;
use super::*;
use crate::pcb::Point;
use crate::pcb_dimension::LinearDimension;
use crate::pcb_dimension_style::{
    ArrowStyle, BoardFont, DimensionStyle, DimensionTextPosition, DimensionUnits,
};

/// The keys a dimension row had before the look settings.
const OLD_KEYS: [&str; 10] = [
    "uuid",
    "start_x",
    "start_y",
    "end_x",
    "end_y",
    "offset",
    "layer",
    "line_width",
    "text_height",
    "precision",
];

fn dimension() -> LinearDimension {
    LinearDimension::new(
        Uuid::from_u128(21),
        Point::new(0.0, 0.0),
        Point::new(20.0, 0.0),
        -2.5,
    )
}

fn saved(d: LinearDimension) -> String {
    let mut board = empty_board();
    board.free.dimensions.push(d);
    SnxPcb::new(board).write_string().expect("writes")
}

/// The keys of the `[[free.dimensions]]` table of `text`.
fn row_keys(text: &str) -> Vec<String> {
    let start = text.find("[[free.dimensions]]").expect("a dimension row");
    text[start..]
        .lines()
        .skip(1)
        .take_while(|l| !l.starts_with('['))
        .filter_map(|l| l.split_once(" = ").map(|(k, _)| k.trim().to_string()))
        .collect()
}

#[test]
fn a_dimension_of_the_old_look_writes_only_the_old_keys() {
    let mut d = dimension();
    d.text_height = 2.0;
    d.line_width = 0.25;
    d.style = DimensionStyle::for_text_height(2.0, 0.25);
    let text = saved(d.clone());
    for key in row_keys(&text) {
        assert!(OLD_KEYS.contains(&key.as_str()), "new key {key} in {text}");
    }
    let back = SnxPcb::parse(&text).expect("reads").board;
    assert_eq!(back.free.dimensions, vec![d]);
}

/// A row with only the old keys, as an older build wrote it, reads with the
/// look it had then: open arrows one text height long, the value above.
#[test]
fn an_old_row_reads_with_the_look_of_its_text_height() {
    let mut d = dimension();
    d.text_height = 1.5;
    d.style = DimensionStyle::for_text_height(1.5, d.line_width);
    let text = saved(d);
    let back = SnxPcb::parse(&text).expect("reads").board;
    let read = &back.free.dimensions[0];
    assert_eq!(read.style.arrow_style, ArrowStyle::Open);
    assert_eq!(read.style.arrow_length, 1.5);
    assert_eq!(read.style.text_position, DimensionTextPosition::Above);
    assert_eq!(read.style.extension_width, read.line_width);
    assert_eq!(read.value_text(), "20.00mm");
    let again = SnxPcb::new(back).write_string().expect("writes again");
    assert_eq!(again, text, "an old row re-saves byte-identical");
}

#[test]
fn every_look_key_round_trips() {
    let mut d = dimension();
    d.style = DimensionStyle {
        arrow_style: ArrowStyle::Filled,
        arrow_length: 1.25,
        arrow_width: 0.5,
        font: BoardFont::Sans,
        bold: true,
        italic: true,
        text_width: 0.75,
        text_position: DimensionTextPosition::Manual,
        text_gap: 0.3,
        extension_offset: 0.4,
        extension_overshoot: 0.8,
        extension_width: 0.15,
        unit: DimensionUnits::Mil,
        show_unit: false,
        prefix: "L \"=\" ".into(),
        suffix: " ±0.1".into(),
        trailing_zeros: false,
    };
    d.text_at = Point::new(1.5, -2.0);
    let text = saved(d.clone());
    for key in [
        "arrow_style = \"filled\"",
        "text_position = \"manual\"",
        "unit = \"mil\"",
        "font = \"sans\"",
        "text_along = 1500000",
        "text_across = -2000000",
        "extension_width = 150000",
    ] {
        assert!(text.contains(key), "{key} missing from {text}");
    }
    let back = SnxPcb::parse(&text).expect("reads back").board;
    assert_eq!(back.free.dimensions, vec![d]);
    let again = SnxPcb::new(back).write_string().expect("writes again");
    assert_eq!(again, text, "a second save is byte-identical");
}

#[test]
fn a_non_finite_look_length_is_refused_on_write() {
    let mut d = dimension();
    d.style.arrow_width = f64::NAN;
    let mut board = empty_board();
    board.free.dimensions.push(d);
    assert!(matches!(
        SnxPcb::new(board).write_string(),
        Err(FormatError::NonFiniteField { .. })
    ));
}

/// The predicate the format token bump reads: true exactly when a save
/// writes a look key.
#[test]
fn a_board_uses_a_dimension_style_only_when_a_look_key_is_written() {
    let mut board = empty_board();
    assert!(!board_uses_dimension_style(&board), "no dimension");
    let mut d = dimension();
    d.text_height = 2.0;
    d.style = DimensionStyle::for_text_height(2.0, d.line_width);
    board.free.dimensions.push(d.clone());
    assert!(!board_uses_dimension_style(&board), "the old look");
    assert!(!dimension_uses_style(&d));

    let mut filled = d.clone();
    filled.style.arrow_style = ArrowStyle::Filled;
    assert!(dimension_uses_style(&filled));
    let mut moved = d.clone();
    moved.text_at = Point::new(0.0, 1.0);
    assert!(dimension_uses_style(&moved), "a Manual point is a look key");
    board.free.dimensions.push(filled);
    assert!(board_uses_dimension_style(&board));
    let text = SnxPcb::new(board).write_string().expect("writes");
    assert!(text.contains("arrow_style"), "{text}");
}
