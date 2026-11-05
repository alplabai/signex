//! Geometry and headless pixel tests for the tab pill quads.
//!
//! `reference_quads` is a verbatim copy of the drawing code from before
//! the per-side border API was adopted. The pixel tests rasterise both it
//! and `pill_quads` through the real `iced_tiny_skia` renderer and require
//! every channel to be within 1/255.

use super::*;
use iced::advanced::Renderer as _;
use iced::advanced::renderer;
use iced::{Font, Pixels};
use iced_tiny_skia::graphics::Viewport;

const W: u32 = 120;
const H: u32 = 32;

fn style(is_last: bool) -> TabPillStyle {
    TabPillStyle {
        fill: Color::BLACK,
        border: Color::WHITE,
        accent: Color::WHITE,
        is_active: false,
        is_last,
        accent_position: AccentPosition::Bottom,
    }
}

fn bounds() -> Rectangle {
    Rectangle {
        x: 10.0,
        y: 0.0,
        width: 80.0,
        height: 24.0,
    }
}

#[test]
fn last_tab_quad_matches_its_layout_bounds() {
    assert_eq!(pill_quad_bounds(bounds(), &style(true)), bounds());
}

#[test]
fn non_last_tab_right_border_lands_under_the_next_tab_left_border() {
    let quad = pill_quad_bounds(bounds(), &style(false));
    let next_left_border_x = bounds().x + bounds().width;
    // The quad's right border spans [right - BORDER_WIDTH, right]; it must
    // be exactly the next tab's left border column, not a second column.
    assert_eq!(quad.x + quad.width - BORDER_WIDTH, next_left_border_x);
    assert_eq!(quad.x, bounds().x);
}

#[test]
fn outline_is_one_pixel_on_every_side_in_the_border_colour() {
    let mut s = style(true);
    s.border = Color::from_rgb(0.2, 0.4, 0.6);
    let b = pill_border(&s);
    for side in [b.top, b.right, b.bottom, b.left] {
        assert_eq!(side.width.unwrap_or(b.width), BORDER_WIDTH);
        assert_eq!(side.color.unwrap_or(b.color), s.border);
    }
}

#[test]
fn rounded_corners_face_away_from_the_strip() {
    let doc = pill_border(&style(true)).radius;
    assert_eq!((doc.top_left, doc.top_right), (TOP_RADIUS, TOP_RADIUS));
    assert_eq!((doc.bottom_left, doc.bottom_right), (0.0, 0.0));

    let mut s = style(true);
    s.accent_position = AccentPosition::Top;
    let panel = pill_border(&s).radius;
    assert_eq!(
        (panel.bottom_left, panel.bottom_right),
        (TOP_RADIUS, TOP_RADIUS)
    );
    assert_eq!((panel.top_left, panel.top_right), (0.0, 0.0));
}

#[test]
fn only_the_active_pill_paints_an_accent_strip() {
    let mut s = style(true);
    assert_eq!(pill_quads(bounds(), &s).len(), 1);
    s.is_active = true;
    let quads = pill_quads(bounds(), &s);
    assert_eq!(quads.len(), 2);
    let strip = quads[1].0.bounds;
    assert_eq!(strip.height, ACCENT_HEIGHT);
    assert_eq!(strip.y + strip.height, bounds().y + bounds().height);
    s.accent_position = AccentPosition::Top;
    assert_eq!(pill_quads(bounds(), &s)[1].0.bounds.y, bounds().y);
}

/// Pre-per-side drawing code, kept as the pixel reference.
fn reference_quads(bounds: Rectangle, style: &TabPillStyle) -> Vec<(Quad, Background)> {
    let radius = match style.accent_position {
        AccentPosition::Bottom => iced::border::Radius::default()
            .top_left(TOP_RADIUS)
            .top_right(TOP_RADIUS),
        AccentPosition::Top => iced::border::Radius::default()
            .bottom_left(TOP_RADIUS)
            .bottom_right(TOP_RADIUS),
    };
    let mut quads = vec![(
        Quad {
            bounds: pill_quad_bounds(bounds, style),
            border: Border {
                width: BORDER_WIDTH,
                radius,
                color: style.border,
                ..Border::default()
            },
            ..Quad::default()
        },
        Background::Color(style.fill),
    )];
    if style.is_active {
        let accent_y = match style.accent_position {
            AccentPosition::Bottom => bounds.y + bounds.height - ACCENT_HEIGHT,
            AccentPosition::Top => bounds.y,
        };
        quads.push((
            Quad {
                bounds: Rectangle {
                    x: bounds.x,
                    y: accent_y,
                    width: bounds.width,
                    height: ACCENT_HEIGHT,
                },
                ..Quad::default()
            },
            Background::Color(style.accent),
        ));
    }
    quads
}

/// Rasterise `quads` on a mid-grey surface; returns RGBA bytes.
fn rasterise(quads: Vec<(Quad, Background)>) -> Vec<u8> {
    let mut renderer = iced_tiny_skia::Renderer::new(renderer::Settings {
        font: Font::DEFAULT,
        text_size: Pixels(16.0),
        ..renderer::Settings::default()
    });
    for (quad, background) in quads {
        renderer.fill_quad(quad, background);
    }
    let mut pixmap = tiny_skia::Pixmap::new(W, H).expect("pixmap");
    let mut mask = tiny_skia::Mask::new(W, H).expect("mask");
    let viewport = Viewport::with_physical_size(Size::new(W, H), Default::default());
    renderer.draw(
        &mut pixmap.as_mut(),
        &mut mask,
        &viewport,
        &[Rectangle::with_size(Size::new(W as f32, H as f32))],
        Color::from_rgb(0.5, 0.5, 0.5),
    );
    pixmap.data().to_vec()
}

/// Two adjacent pills (first non-last, second last) as a tab row paints
/// them, built with `quads`.
fn row(
    quads: fn(Rectangle, &TabPillStyle) -> Vec<(Quad, Background)>,
    first: &TabPillStyle,
    second: &TabPillStyle,
) -> Vec<u8> {
    let a = Rectangle {
        x: 4.0,
        y: 4.0,
        width: 50.0,
        height: 22.0,
    };
    let b = Rectangle { x: 54.0, ..a };
    let mut all = quads(a, first);
    all.extend(quads(b, second));
    rasterise(all)
}

fn assert_close(expected: &[u8], actual: &[u8], what: &str) {
    assert_eq!(expected.len(), actual.len());
    for (i, (e, a)) in expected.iter().zip(actual).enumerate() {
        assert!(
            e.abs_diff(*a) <= 1,
            "{what}: byte {i} (px {}, {}) differs: {e} vs {a}",
            (i / 4) as u32 % W,
            (i / 4) as u32 / W
        );
    }
}

fn pill(active: bool, last: bool, position: AccentPosition, hover: bool) -> TabPillStyle {
    TabPillStyle {
        fill: if hover {
            Color::from_rgb(0.20, 0.22, 0.28)
        } else {
            Color::from_rgb(0.12, 0.13, 0.16)
        },
        border: Color::from_rgb(0.30, 0.32, 0.38),
        accent: Color::from_rgb(0.25, 0.55, 0.95),
        is_active: active,
        is_last: last,
        accent_position: position,
    }
}

#[test]
fn rows_render_identically_to_the_reference_in_every_state() {
    for position in [AccentPosition::Bottom, AccentPosition::Top] {
        for first_active in [false, true] {
            for second_active in [false, true] {
                for hover in [false, true] {
                    let first = pill(first_active, false, position, false);
                    let second = pill(second_active, true, position, hover);
                    assert_close(
                        &row(reference_quads, &first, &second),
                        &row(pill_quads, &first, &second),
                        &format!(
                            "{position:?} active=({first_active},{second_active}) hover={hover}"
                        ),
                    );
                }
            }
        }
    }
}

#[test]
fn single_last_pill_renders_identically_to_the_reference() {
    for position in [AccentPosition::Bottom, AccentPosition::Top] {
        for active in [false, true] {
            let s = pill(active, true, position, false);
            let b = bounds();
            assert_close(
                &rasterise(reference_quads(b, &s)),
                &rasterise(pill_quads(b, &s)),
                &format!("{position:?} active={active}"),
            );
        }
    }
}

#[test]
fn shared_divider_is_a_single_border_column() {
    let first = pill(false, false, AccentPosition::Bottom, false);
    let second = pill(false, true, AccentPosition::Bottom, false);
    let px = row(pill_quads, &first, &second);
    let at = |x: u32, y: u32| {
        let i = ((y * W + x) * 4) as usize;
        // The tiny-skia backend writes BGRA; read it back as RGB.
        [px[i + 2], px[i + 1], px[i]]
    };
    let border = [0.30f32, 0.32, 0.38].map(|c| (c * 255.0).round() as u8);
    let fill = [0.12f32, 0.13, 0.16].map(|c| (c * 255.0).round() as u8);
    let close = |a: [u8; 3], b: [u8; 3]| a.iter().zip(b).all(|(x, y)| x.abs_diff(y) <= 1);
    // Divider column is x = 54 (next tab's left border); neighbours are fill.
    assert!(close(at(54, 15), border), "{:?}", at(54, 15));
    assert!(close(at(53, 15), fill), "{:?}", at(53, 15));
    assert!(close(at(55, 15), fill), "{:?}", at(55, 15));
}
