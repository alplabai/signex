//! Square tab variant: flat corners, no bottom edge, 2 px top accent.

use super::{ACCENT_HEIGHT, BORDER_WIDTH, TabPillStyle, pill_quad_bounds};
use iced::advanced::renderer::Quad;
use iced::{Background, Border, Rectangle, border};

/// Outline of a square pill: 1 px left, right and top; nothing below, so
/// the tab merges into the panel under it.
pub(super) fn square_pill_border(style: &TabPillStyle) -> Border {
    let edge = border::Side::default()
        .color(style.border)
        .width(BORDER_WIDTH);
    Border {
        color: style.border,
        width: BORDER_WIDTH,
        radius: border::Radius::default(),
        top: edge,
        right: edge,
        bottom: border::Side::default().width(0.0),
        left: edge,
    }
}

/// Bounds of the 2 px top accent for a pill laid out in `bounds`.
pub(super) fn square_accent_bounds(bounds: Rectangle) -> Rectangle {
    Rectangle {
        height: ACCENT_HEIGHT,
        ..bounds
    }
}

/// Every quad a square pill paints, in paint order.
pub(super) fn square_pill_quads(
    bounds: Rectangle,
    style: &TabPillStyle,
) -> Vec<(Quad, Background)> {
    let mut quads = vec![(
        Quad {
            bounds: pill_quad_bounds(bounds, style),
            border: square_pill_border(style),
            ..Quad::default()
        },
        Background::Color(style.fill),
    )];
    if style.is_active {
        quads.push((
            Quad {
                bounds: square_accent_bounds(bounds),
                ..Quad::default()
            },
            Background::Color(style.accent),
        ));
    }
    quads
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tab_pill::AccentPosition;
    use iced::Color;

    fn style(is_active: bool) -> TabPillStyle {
        TabPillStyle {
            fill: Color::BLACK,
            border: Color::WHITE,
            accent: Color::WHITE,
            is_active,
            is_last: true,
            accent_position: AccentPosition::Bottom,
        }
    }

    fn bounds() -> Rectangle {
        Rectangle {
            x: 10.0,
            y: 4.0,
            width: 80.0,
            height: 24.0,
        }
    }

    #[test]
    fn square_border_has_no_rounding_and_no_bottom_edge() {
        let b = square_pill_border(&style(false));
        assert_eq!(b.radius, border::Radius::default());
        assert_eq!(b.bottom.width, Some(0.0));
        assert_eq!(b.top.width, Some(BORDER_WIDTH));
    }

    #[test]
    fn active_square_pill_paints_a_two_px_accent_on_top() {
        let quads = square_pill_quads(bounds(), &style(true));
        assert_eq!(quads.len(), 2);
        let accent = quads[1].0.bounds;
        assert_eq!((accent.y, accent.height), (4.0, 2.0));
        assert_eq!(accent.width, 80.0);
    }

    #[test]
    fn inactive_square_pill_has_no_accent() {
        assert_eq!(square_pill_quads(bounds(), &style(false)).len(), 1);
    }
}
