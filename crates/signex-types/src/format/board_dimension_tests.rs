//! #2868 — the look keys of a `[[free.dimensions]]` row: written only where
//! they differ from the look before the settings, read back whole.

use uuid::Uuid;

use super::tests::empty_board;
use super::*;
use crate::format_version::FormatVersion;
use crate::pcb::Point;
use crate::pcb_dimension::LinearDimension;
use crate::pcb_dimension_style::{
    ArrowStyle, BoardFont, DimensionDefaults, DimensionStyle, DimensionTextPosition, DimensionUnits,
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
    d.text_at = d.above_text_at();
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
    moved.style.text_position = DimensionTextPosition::Manual;
    moved.text_at = Point::new(0.0, 1.0);
    assert!(dimension_uses_style(&moved), "a Manual point is a look key");
    let mut unshown = d.clone();
    unshown.text_at = Point::new(0.0, 1.0);
    assert!(
        !dimension_uses_style(&unshown),
        "the point of a value Above is not drawn, so not written"
    );
    board.free.dimensions.push(filled);
    assert!(board_uses_dimension_style(&board));
    let text = SnxPcb::new(board).write_string().expect("writes");
    assert!(text.contains("arrow_style"), "{text}");
}

fn token_of(text: &str) -> &str {
    let first = text.lines().next().unwrap_or_default();
    first
        .strip_prefix("format = \"")
        .and_then(|rest| rest.strip_suffix('"'))
        .unwrap_or_default()
}

/// A dimension whose arrows are filled: a look key is written.
fn styled() -> LinearDimension {
    let mut d = plain(21);
    d.style.arrow_style = ArrowStyle::Filled;
    d
}

/// A dimension with the look of its own text height: no look key.
fn plain(n: u128) -> LinearDimension {
    let mut d = dimension();
    d.uuid = Uuid::from_u128(n);
    d.text_height = 2.0;
    d.style = DimensionStyle::for_text_height(2.0, d.line_width);
    d.text_at = d.above_text_at();
    d
}

/// DECISIONS 2026-11-24: a non-default dimension look writes `snxpcb/1.11`,
/// which this build opens for editing and re-saves byte-identical.
#[test]
fn a_dimension_look_writes_snxpcb_1_11_and_round_trips() {
    let d = styled();
    let text = saved(d.clone());
    assert_eq!(token_of(&text), SNXPCB_FORMAT_V1_11, "{text}");
    let file = SnxPcb::parse(&text).expect("reads");
    assert!(!file.is_from_newer_minor(), "this build reads 1.11");
    assert_eq!(file.board.free.dimensions, vec![d]);
    let again = file.write_string().expect("writes again");
    assert_eq!(again, text, "a re-save is byte-identical");
}

/// A plain board, and a board whose dimensions all have the default look
/// and no pour cutout, keep the token they had before the look settings.
#[test]
fn a_board_with_only_default_look_dimensions_keeps_its_token() {
    assert_eq!(
        token_of(&SnxPcb::new(empty_board()).write_string().unwrap()),
        SNXPCB_FORMAT_V1
    );
    let mut board = empty_board();
    board.free.dimensions = vec![plain(21), plain(22)];
    assert!(!board_uses_dimension_style(&board));
    let before = SnxPcb::new(board.clone()).write_string().unwrap();
    assert_eq!(token_of(&before), SNXPCB_FORMAT_V1);
    board.free.dimensions[1].style.arrow_style = ArrowStyle::Filled;
    let raised = SnxPcb::new(board.clone()).write_string().unwrap();
    assert_eq!(token_of(&raised), SNXPCB_FORMAT_V1_11);
    board.free.dimensions[1] = plain(22);
    let back = SnxPcb::new(board).write_string().unwrap();
    assert_eq!(back, before, "the file is what it was");
}

/// A build whose policy stops at `snxpcb/1.10` sees a board with a dimension
/// look as newer: it opens it read-only and refuses its saves, instead of
/// dropping the look keys.
#[test]
fn a_dimension_look_opens_read_only_in_a_build_that_knows_only_1_10() {
    let one_ten = FormatVersion {
        current: SNXPCB_FORMAT_V1_10,
        readable: &[
            "snxpcb/1",
            "snxpcb/1.1",
            "snxpcb/1.2",
            "snxpcb/1.3",
            SNXPCB_FORMAT_V1,
            SNXPCB_FORMAT_LAYER_COLORS,
            SNXPCB_FORMAT_ROOMS,
            SNXPCB_FORMAT_BOARD_SKETCH_TARGETS,
            SNXPCB_FORMAT_V1_8,
            SNXPCB_FORMAT_V1_9,
            SNXPCB_FORMAT_V1_10,
        ],
        ..SNXPCB_VERSION
    };
    let text = saved(styled());
    let token = token_of(&text);
    assert_eq!(token, SNXPCB_FORMAT_V1_11);
    assert!(one_ten.is_newer_minor(token), "{token}");
    assert!(one_ten.refuse_save(token).is_err(), "{token}");
    assert!(!SNXPCB_VERSION.is_newer_minor(token), "this build reads it");
    assert!(SNXPCB_VERSION.refuse_save(token).is_ok());
}

/// A dimension as the app makes one, its value Above with the point where
/// Above puts it (never the middle of the line), writes no look key, so a
/// board of them keeps its token.
#[test]
fn a_dimension_with_the_defaults_uses_no_style() {
    for offset in [2.5, -2.5] {
        let new = LinearDimension::new(
            Uuid::from_u128(21),
            Point::new(0.0, 0.0),
            Point::new(20.0, 0.0),
            offset,
        );
        assert!(!dimension_uses_style(&new), "new, offset {offset}");
        let d = new.with_defaults(&DimensionDefaults::default());
        assert_ne!(d.text_at, Point::new(0.0, 0.0), "Above is off the line");
        assert!(!dimension_uses_style(&d), "with defaults, offset {offset}");
        let text = saved(d.clone());
        assert_eq!(token_of(&text), SNXPCB_FORMAT_V1, "{text}");
        let back = SnxPcb::parse(&text).expect("reads").board;
        assert_eq!(back.free.dimensions, vec![d]);
    }
}

/// A row that is not Manual reads its point where Above puts the value, as
/// the app would place it, and re-saves byte-identical.
#[test]
fn a_row_that_is_not_manual_reads_its_point_above() {
    for position in [DimensionTextPosition::Above, DimensionTextPosition::Inline] {
        let mut d = plain(21);
        d.style.text_position = position;
        d.text_at = Point::new(4.0, 4.0);
        let text = saved(d);
        assert!(!text.contains("text_along"), "{text}");
        let file = SnxPcb::parse(&text).expect("reads");
        let read = &file.board.free.dimensions[0];
        assert_eq!(read.text_at, read.above_text_at(), "{position:?}");
        assert_eq!(file.write_string().expect("writes again"), text);
    }
}

/// A Manual value whose text was moved writes its point and `snxpcb/1.11`,
/// and reads it back; one at the middle of the line writes no point.
#[test]
fn a_moved_manual_value_writes_snxpcb_1_11_and_round_trips_its_point() {
    let mut d = plain(21);
    d.style.text_position = DimensionTextPosition::Manual;
    d.text_at = Point::new(1.5, -0.75);
    let text = saved(d.clone());
    assert_eq!(token_of(&text), SNXPCB_FORMAT_V1_11, "{text}");
    assert!(text.contains("text_along = 1500000"), "{text}");
    assert!(text.contains("text_across = -750000"), "{text}");
    let file = SnxPcb::parse(&text).expect("reads");
    assert_eq!(file.board.free.dimensions, vec![d]);
    assert_eq!(file.write_string().expect("writes again"), text);

    let mut middle = plain(21);
    middle.style.text_position = DimensionTextPosition::Manual;
    middle.text_at = Point::new(0.0, 0.0);
    let text = saved(middle.clone());
    assert!(!text.contains("text_along"), "{text}");
    let back = SnxPcb::parse(&text).expect("reads").board;
    assert_eq!(back.free.dimensions, vec![middle]);
}
