//! #2868: the look of a board dimension — arrows, text position, extension
//! lines and the written value.

use uuid::Uuid;

use super::*;
use crate::pcb_dimension::{EXTENSION_STROKES, LinearDimension};
use crate::schematic::Point;

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn close(a: Point, b: Point) -> bool {
    (a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9
}

/// A 10 mm dimension standing 2 mm above its points.
fn dimension() -> LinearDimension {
    LinearDimension::new(Uuid::from_u128(1), p(0.0, 0.0), p(10.0, 0.0), 2.0)
}

#[test]
fn the_default_style_is_the_look_from_before_the_settings() {
    let style = DimensionStyle::default();
    assert_eq!(style, DimensionStyle::for_text_height(1.0, 0.1));
    assert_eq!(style.arrow_style, ArrowStyle::Open);
    assert_eq!(style.text_position, DimensionTextPosition::Above);
    assert_eq!(style.unit, DimensionUnits::Mm);
    assert!(style.show_unit && style.trailing_zeros);
    assert_eq!(DimensionDefaults::default().style, style);
}

#[test]
fn filled_arrows_are_triangles_and_draw_no_arrow_strokes() {
    let mut d = dimension();
    d.style.arrow_style = ArrowStyle::Filled;
    d.style.arrow_length = 2.0;
    d.style.arrow_width = 1.0;
    let drawing = d.drawing().unwrap();
    assert_eq!(drawing.strokes.len(), EXTENSION_STROKES + 1);
    assert_eq!(drawing.filled_arrows.len(), 2);
    let [tip, left, right] = drawing.filled_arrows[0];
    assert!(close(tip, p(0.0, 2.0)));
    assert!(close(left, p(2.0, 2.5)) && close(right, p(2.0, 1.5)));
    let [tip, ..] = drawing.filled_arrows[1];
    assert!(close(tip, p(10.0, 2.0)));
}

#[test]
fn no_arrows_draws_only_the_lines() {
    let mut d = dimension();
    d.style.arrow_style = ArrowStyle::None;
    let drawing = d.drawing().unwrap();
    assert_eq!(drawing.strokes.len(), EXTENSION_STROKES + 1);
    assert!(drawing.filled_arrows.is_empty());
}

#[test]
fn open_arrows_take_the_length_and_width() {
    let mut d = dimension();
    d.style.arrow_length = 1.5;
    d.style.arrow_width = 0.8;
    let s = d.drawing().unwrap().strokes;
    assert!(close(s[3].1, p(1.5, 2.4)) && close(s[4].1, p(1.5, 1.6)));
    assert!(close(s[5].1, p(8.5, 2.4)) && close(s[6].1, p(8.5, 1.6)));
}

#[test]
fn above_stands_the_value_its_gap_off_the_line() {
    let mut d = dimension();
    d.style.text_gap = 1.0;
    let drawing = d.drawing().unwrap();
    // The gap, then half the text height.
    assert!(
        close(drawing.label_at, p(5.0, 3.5)),
        "{:?}",
        drawing.label_at
    );
}

#[test]
fn inline_puts_the_value_on_the_line_and_breaks_the_line_around_it() {
    let mut d = dimension();
    d.style.text_position = DimensionTextPosition::Inline;
    d.style.text_gap = 0.5;
    d.style.text_width = 0.5;
    let drawing = d.drawing().unwrap();
    assert!(close(drawing.label_at, p(5.0, 2.0)));
    // "10.00mm" is 7 characters, 3.5 mm; half of it and the gap is 2.25.
    let line = &drawing.strokes[EXTENSION_STROKES..EXTENSION_STROKES + 2];
    assert!(close(line[0].0, p(0.0, 2.0)) && close(line[0].1, p(2.75, 2.0)));
    assert!(close(line[1].0, p(7.25, 2.0)) && close(line[1].1, p(10.0, 2.0)));
}

#[test]
fn an_inline_value_wider_than_the_line_leaves_no_line() {
    let mut d = dimension();
    d.style.text_position = DimensionTextPosition::Inline;
    d.style.arrow_style = ArrowStyle::None;
    d.style.text_width = 5.0;
    let drawing = d.drawing().unwrap();
    assert_eq!(drawing.strokes.len(), EXTENSION_STROKES);
}

#[test]
fn manual_puts_the_value_at_its_point_in_the_dimensions_frame() {
    let mut d = LinearDimension::new(Uuid::from_u128(1), p(0.0, 0.0), p(0.0, 10.0), 2.0);
    d.style.text_position = DimensionTextPosition::Manual;
    d.text_at = p(3.0, 1.0);
    let drawing = d.drawing().unwrap();
    // Along runs +y, the quarter turn -x; the line stands at x = -2.
    assert!(
        close(drawing.label_at, p(-3.0, 8.0)),
        "{:?}",
        drawing.label_at
    );
}

#[test]
fn extension_lines_start_their_offset_off_the_points_and_overshoot() {
    let mut d = dimension();
    d.style.extension_offset = 0.5;
    d.style.extension_overshoot = 1.0;
    d.style.extension_width = 0.3;
    let s = d.drawing().unwrap().strokes;
    assert!(close(s[0].0, p(0.0, 0.5)) && close(s[0].1, p(0.0, 3.0)));
    assert!(close(s[1].0, p(10.0, 0.5)) && close(s[1].1, p(10.0, 3.0)));
    assert_eq!(d.stroke_width(0), 0.3);
    assert_eq!(d.stroke_width(1), 0.3);
    assert_eq!(d.stroke_width(EXTENSION_STROKES), d.line_width);
}

#[test]
fn the_value_is_written_in_its_unit_with_prefix_suffix_and_zeros() {
    let mut d = dimension();
    d.end = p(25.4, 0.0);
    assert_eq!(d.value_text(), "25.40mm");
    d.style.unit = DimensionUnits::In;
    assert_eq!(d.value_text(), "1.00in");
    d.style.unit = DimensionUnits::Mil;
    d.precision = 1;
    assert_eq!(d.value_text(), "1000.0mil");
    d.style.trailing_zeros = false;
    assert_eq!(d.value_text(), "1000mil");
    d.style.show_unit = false;
    d.style.prefix = "W=".into();
    d.style.suffix = " typ".into();
    assert_eq!(d.value_text(), "W=1000 typ");
    d.style.unit = DimensionUnits::Mm;
    d.precision = 3;
    assert_eq!(d.value_text(), "W=25.4 typ");
}

#[test]
fn trailing_zeros_off_keeps_a_whole_number_whole() {
    let style = DimensionStyle {
        trailing_zeros: false,
        ..DimensionStyle::default()
    };
    assert_eq!(style.value_text(10.0, 2), "10mm");
    assert_eq!(style.value_text(10.0, 0), "10mm");
}

#[test]
fn a_new_dimension_takes_the_defaults_and_a_manual_value_starts_above() {
    let defaults = DimensionDefaults {
        line_width: 0.2,
        text_height: 2.0,
        precision: 1,
        style: DimensionStyle {
            text_position: DimensionTextPosition::Manual,
            text_gap: 0.25,
            arrow_style: ArrowStyle::Filled,
            ..DimensionStyle::default()
        },
    };
    let d = dimension().with_defaults(&defaults);
    assert_eq!(d.line_width, 0.2);
    assert_eq!(d.text_height, 2.0);
    assert_eq!(d.precision, 1);
    assert_eq!(d.style, defaults.style);
    assert!(close(d.text_at, p(0.0, 1.25)), "{:?}", d.text_at);
}

#[test]
fn a_style_length_that_is_not_finite_draws_nothing() {
    let mut d = dimension();
    d.style.arrow_length = f64::NAN;
    assert!(d.drawing().is_none());
}
