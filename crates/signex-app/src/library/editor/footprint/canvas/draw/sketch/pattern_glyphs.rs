//! Pattern glyphs (#187, #191): each sketch array draws a small label by
//! its source ("L×4", "G×3×4", "P×8") that a click selects and a
//! right-click opens the pattern menu on. While a grid or polar pattern is
//! selected, each of its instance slots draws a ring a click suppresses or
//! brings back: a filled ring for a kept instance, a hollow struck ring for
//! a suppressed one.

use iced::widget::canvas::{self, Path, Stroke};
use iced::{Color, Point};

use super::dimension_labels::{LabelInk, LabelPaint, draw_label};
use crate::library::editor::footprint::canvas::glyph_layout::{Glyph, GlyphFace};

/// Border of a pattern glyph at rest and when selected, in px.
const BORDER_PX: f32 = 1.0;
const BORDER_SELECTED_PX: f32 = 1.6;
/// Ring width of an instance slot, in px.
const RING_PX: f32 = 1.5;

/// A pattern's glyph: accent border and text when selected.
pub(super) fn draw_pattern_glyph(frame: &mut canvas::Frame, glyph: &Glyph, ink: &LabelInk) {
    let GlyphFace::Pattern { text, selected } = &glyph.face else {
        return;
    };
    let paint = if *selected {
        LabelPaint {
            fill: ink.fill,
            border: ink.accent,
            border_px: BORDER_SELECTED_PX,
            text: ink.accent,
            caret: false,
        }
    } else {
        LabelPaint {
            fill: ink.fill,
            border: ink.border,
            border_px: BORDER_PX,
            text: ink.text,
            caret: false,
        }
    };
    draw_label(frame, glyph.rect, text, paint);
}

/// One instance slot of the selected pattern.
pub(super) fn draw_instance(frame: &mut canvas::Frame, glyph: &Glyph, ink: &LabelInk) {
    let GlyphFace::Instance { suppressed } = glyph.face else {
        return;
    };
    let r = glyph.rect.width / 2.0;
    let c = glyph.anchor;
    let ring = Path::circle(c, r);
    if suppressed {
        let muted = Color {
            a: 0.6,
            ..ink.border
        };
        let stroke = Stroke::default().with_width(RING_PX).with_color(muted);
        frame.stroke(&ring, stroke);
        let d = r * std::f32::consts::FRAC_1_SQRT_2;
        frame.stroke(
            &Path::line(Point::new(c.x - d, c.y + d), Point::new(c.x + d, c.y - d)),
            stroke,
        );
    } else {
        frame.fill(
            &ring,
            Color {
                a: 0.25,
                ..ink.accent
            },
        );
        frame.stroke(
            &ring,
            Stroke::default().with_width(RING_PX).with_color(ink.accent),
        );
    }
}
