//! The sketch canvas ink (#187, #188): the colours sketch geometry,
//! constraint marks and dimension labels draw in, as one palette.
//!
//! Sketch mode paints the dark board ground on every theme (decision 7,
//! Caner 2026-11-20; `background::draw_backdrop`), and the sketch follows
//! the mockups on it: green fully constrained, blue under, red over, grey
//! for construction and an unsolved sketch, gold centerlines, orange
//! selection, pill-shaped marks and labels, reference dimensions in purple.
//! Like the board palette ([`crate::styles::board_canvas_colors`]) it does
//! not change with the theme. A conflict keeps the theme's danger colour.

use iced::Color;

/// Colours of the sketch overlay.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct SketchInk {
    /// Under-constrained geometry.
    pub under: Color,
    /// Fully constrained geometry.
    pub full: Color,
    /// Over-constrained geometry.
    pub over: Color,
    /// Geometry before any solve, and a circle's stroke before any solve.
    pub unsolved: Color,
    /// Construction lines (dashed).
    pub construction: Color,
    /// Centerlines.
    pub centerline: Color,
    /// Selected geometry and the accent of an edited value.
    pub selection: Color,
    /// DOF direction arrows.
    pub dof_arrow: Color,
    /// Text of a value label or constraint mark.
    pub label: Color,
    /// Inside of a label or mark.
    pub pill: Color,
    /// Border of a label or mark at rest.
    pub pill_border: Color,
    /// Dimension lines and their arrowheads.
    pub dimension: Color,
    /// A reference (driven) dimension: its label and lines.
    pub reference: Color,
}

/// `0xRRGGBB` as an opaque colour.
const fn hex(rgb: u32) -> Color {
    Color::from_rgb(
        ((rgb >> 16) & 0xFF) as f32 / 255.0,
        ((rgb >> 8) & 0xFF) as f32 / 255.0,
        (rgb & 0xFF) as f32 / 255.0,
    )
}

impl SketchInk {
    /// The mockup palette on the board ground (graphite mockups, the
    /// shared sketcher block's `C` palette).
    pub(super) const BOARD: Self = Self {
        under: hex(0xA9C2FF),
        full: hex(0x7FD1A0),
        over: hex(0xF2A093),
        unsolved: hex(0x8B8D93),
        construction: hex(0x8B8D93),
        centerline: hex(0xC9A13B),
        selection: hex(0xE0865C),
        dof_arrow: hex(0x1A99E6),
        label: hex(0xCFD1D6),
        pill: hex(0x1D1F23),
        pill_border: hex(0x34363C),
        dimension: hex(0x7C7F86),
        reference: hex(0xC9A0FF),
    };
}

#[cfg(test)]
mod tests {
    use super::super::super::graphite::board_look;
    use super::*;

    /// Perceived brightness of `c`, 0 (black) to 1 (white).
    fn luminance(c: Color) -> f32 {
        0.2126 * c.r + 0.7152 * c.g + 0.0722 * c.b
    }

    #[test]
    fn the_sketch_takes_the_mockup_palette() {
        let ink = SketchInk::BOARD;
        assert_eq!(ink.full, hex(0x7FD1A0), "green when fully constrained");
        assert_eq!(ink.under, hex(0xA9C2FF));
        assert_eq!(ink.over, hex(0xF2A093));
        assert_eq!(ink.reference, hex(0xC9A0FF));
        assert_eq!(ink.pill, hex(0x1D1F23));
    }

    #[test]
    fn every_mark_reads_on_the_board_ground() {
        let ink = SketchInk::BOARD;
        let ground = board_look().bg;
        for c in [ink.under, ink.full, ink.label, ink.reference] {
            assert!(
                (luminance(c) - luminance(ground)).abs() > 0.3,
                "{c:?} on {ground:?}"
            );
        }
    }
}
