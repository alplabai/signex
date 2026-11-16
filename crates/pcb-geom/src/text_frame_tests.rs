use super::*;
use uuid::Uuid;

fn frame(text: &str, a: (f64, f64), b: (f64, f64), layer: &str) -> BoardGraphic {
    BoardGraphic {
        uuid: Uuid::nil(),
        graphic_type: "text_frame".into(),
        layer: layer.into(),
        width: 0.1,
        start: Some(Point::new(a.0, a.1)),
        end: Some(Point::new(b.0, b.1)),
        center: None,
        radius: 0.0,
        points: Vec::new(),
        text: text.into(),
        font_size: 1.0,
    }
}

#[test]
fn words_wrap_at_spaces_and_a_long_word_is_cut() {
    assert_eq!(wrap("one two three", 7), ["one two", "three"]);
    assert_eq!(wrap("abcdefghij", 4), ["abcd", "efgh", "ij"]);
    assert_eq!(wrap("a\n\nb", 10), ["a", "", "b"]);
    assert_eq!(wrap("", 3), [""]);
}

/// #2424: lines run down from the top of the box, centred across it, and
/// stop at its bottom.
#[test]
fn lines_fill_the_box_from_the_top_and_stop_at_its_bottom() {
    // 10 mm wide at 1 mm text: (10 - 0.5) / 0.6 = 15 columns; 4 mm tall:
    // (4 - 0.5) / 1.4 = 2 rows.
    let g = frame(
        "first line here second line here third",
        (0.0, 0.0),
        (10.0, 4.0),
        "TopSilkscreen",
    );
    let lines = text_frame_lines(&g).unwrap();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].text, "first line here");
    assert_eq!(lines[1].text, "second line");
    let first = lines[0].placement.anchor.to_mm();
    let second = lines[1].placement.anchor.to_mm();
    assert!((first.x - 5.0).abs() < 1e-6, "{first:?}");
    assert!((first.y - (0.25 + 0.7)).abs() < 1e-6, "{first:?}");
    assert!((second.y - first.y - 1.4).abs() < 1e-6);
    assert!(!lines[0].placement.mirrored);
}

#[test]
fn a_bottom_frame_is_read_from_below_and_other_drawings_have_no_lines() {
    let bottom = frame("REV A", (0.0, 0.0), (10.0, 4.0), "BottomSilkscreen");
    assert!(text_frame_lines(&bottom).unwrap()[0].placement.mirrored);
    let mut line = bottom.clone();
    line.graphic_type = "line".into();
    assert!(text_frame_lines(&line).unwrap().is_empty());
    let mut bad = bottom;
    bad.start = Some(Point::new(f64::NAN, 0.0));
    assert!(text_frame_lines(&bad).is_err());
}
