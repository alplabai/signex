use super::*;

fn upright() -> TextPlacement {
    TextPlacement {
        anchor: PointNm::new(0, 0),
        rotation_deg: 0.0,
        mirrored: false,
    }
}

fn bounds(strokes: &[Vec<PointNm>]) -> (i64, i64, i64, i64) {
    let points = strokes.iter().flatten();
    let xs = points.clone().map(|p| p.x);
    let ys = points.map(|p| p.y);
    (
        xs.clone().min().unwrap(),
        xs.max().unwrap(),
        ys.clone().min().unwrap(),
        ys.max().unwrap(),
    )
}

#[test]
fn every_printable_ascii_glyph_is_drawn() {
    for glyph in '!'..='~' {
        assert!(glyph_source(glyph).is_some(), "{glyph:?}");
        let strokes = glyph_strokes(glyph);
        assert!(!strokes.is_empty(), "{glyph:?}");
        for (x, y) in strokes.iter().flatten() {
            assert!((0.0..=GRID_WIDTH).contains(x), "{glyph:?}");
            assert!((-2.0..=GRID_CAP).contains(y), "{glyph:?}");
        }
    }
    assert!(glyph_strokes(' ').is_empty());
}

#[test]
fn a_glyph_the_font_lacks_is_a_box() {
    assert_eq!(glyph_strokes('\u{2603}'), glyph_strokes_of(MISSING));
}

fn glyph_strokes_of(source: &str) -> Vec<Vec<(f64, f64)>> {
    source
        .split('|')
        .map(|p| {
            p.split_whitespace()
                .map(|q| {
                    let (x, y) = q.split_once(',').unwrap();
                    (x.parse().unwrap(), y.parse().unwrap())
                })
                .collect()
        })
        .collect()
}

#[test]
fn a_capital_fills_the_cap_height_centred_on_the_anchor() {
    let strokes = text_strokes("H", &upright(), 1.0, HAlign::Center).unwrap();
    let (left, right, top, bottom) = bounds(&strokes);
    // One glyph is half the cap height wide, centred; the cap height is
    // centred on the anchor (y down).
    assert_eq!((left, right), (-250_000, 250_000));
    assert_eq!((top, bottom), (-500_000, 500_000));
}

#[test]
fn the_line_is_as_long_as_its_glyphs() {
    assert_eq!(stroke_text_width_mm("", 1.0), 0.0);
    assert_eq!(stroke_text_width_mm("R", 1.0), 0.5);
    assert_eq!(stroke_text_width_mm("R1", 1.0), 1.25);
    // `H` fills its cell to both edges, so the strokes span the line.
    let strokes = text_strokes("RH", &upright(), 2.0, HAlign::Left).unwrap();
    let (left, right, _, _) = bounds(&strokes);
    assert_eq!((left, right), (0, 2_500_000));
    let strokes = text_strokes("RH", &upright(), 2.0, HAlign::Right).unwrap();
    let (left, right, _, _) = bounds(&strokes);
    assert_eq!((left, right), (-2_500_000, 0));
}

#[test]
fn a_turned_text_turns_its_strokes() {
    let at = TextPlacement {
        rotation_deg: 90.0,
        ..upright()
    };
    let strokes = text_strokes("-", &at, 1.0, HAlign::Left).unwrap();
    // The bar runs along the line, which a quarter turn lays down the
    // board's y axis.
    let (left, right, top, bottom) = bounds(&strokes);
    assert_eq!(left, right);
    assert_eq!((top, bottom), (0, 500_000));
}

#[test]
fn a_dot_is_one_point() {
    let strokes = text_strokes(".", &upright(), 1.0, HAlign::Center).unwrap();
    assert_eq!(strokes.len(), 1);
    assert_eq!(strokes[0].len(), 1);
}

#[test]
fn a_size_that_is_not_a_length_is_refused() {
    for size in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(text_strokes("R1", &upright(), size, HAlign::Center).is_err());
    }
}

#[test]
fn the_pen_is_a_share_of_the_height() {
    assert_eq!(text_stroke_width_mm(2.0), 2.0 * TEXT_STROKE_EM);
}

#[test]
fn a_line_too_long_for_its_room_is_condensed_and_a_short_one_kept() {
    // "RH" at 2 mm is 2.5 mm long; in 2 mm of room it is squeezed to 2 mm,
    // centred on the anchor, its height unchanged.
    let strokes = text_strokes_within("RH", &upright(), 2.0, HAlign::Center, 2.0).unwrap();
    let (left, right, top, bottom) = bounds(&strokes);
    assert_eq!((left, right), (-1_000_000, 1_000_000));
    assert_eq!((top, bottom), (-1_000_000, 1_000_000));
    // With room to spare it is written as `text_strokes` writes it.
    let roomy = text_strokes_within("RH", &upright(), 2.0, HAlign::Center, 10.0).unwrap();
    let plain = text_strokes("RH", &upright(), 2.0, HAlign::Center).unwrap();
    assert_eq!(roomy, plain);
}
