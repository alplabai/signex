//! Wire and junction styling for the SVG page (and so for the PDF and the
//! print preview, which are lowered from it).
//!
//! Precedence, per property, matches the canvas:
//!
//! - **colour** - the sheet's per-net colour override, else the net class
//!   colour, else the palette's wire colour;
//! - **width** - the class width, else the wire's own width, else 0.15 mm;
//! - **line style** - the class line style, else solid.
//!
//! A dashed or dotted wire is emitted as a run of short sub-paths in one
//! path element, so the page model needs no dash attribute and every output
//! (SVG text, raster, PDF operators) draws it the same way.

use std::collections::HashMap;

use signex_types::net_class::{NetClassStyle, NetLineStyle};
use signex_types::schematic::Wire;
use signex_types::theme::Color;
use uuid::Uuid;

/// Default wire width in mm when neither the wire nor its class sets one.
const DEFAULT_WIRE_MM: f64 = 0.15;

/// `(mark, gap)` in units of the stroke width. The numbers are the canvas's
/// screen-pixel pattern (`LineSegment::DASH_PX` / `DASH_GAP_PX` and `DOT_PX` /
/// `DOT_GAP_PX` in signex-gfx), read as multiples of the stroke width.
const DASH: (f64, f64) = (8.0, 5.0);
const DOT: (f64, f64) = (2.0, 4.0);

pub(super) fn rgb(color: Color) -> (f32, f32, f32) {
    (
        f32::from(color.r) / 255.0,
        f32::from(color.g) / 255.0,
        f32::from(color.b) / 255.0,
    )
}

/// The stroke colour of `wire`: override, else class, else `palette`.
pub(super) fn wire_rgb(
    wire: &Wire,
    net_colors: &HashMap<Uuid, Color>,
    classes: &HashMap<Uuid, NetClassStyle>,
    palette: (f32, f32, f32),
) -> (f32, f32, f32) {
    net_colors
        .get(&wire.uuid)
        .copied()
        .or_else(|| classes.get(&wire.uuid).and_then(|style| style.color))
        .map_or(palette, rgb)
}

/// The stroke width of `wire` in mm.
pub(super) fn wire_width_mm(wire: &Wire, class: Option<&NetClassStyle>) -> f64 {
    let own = if wire.stroke_width > 0.0 {
        wire.stroke_width
    } else {
        DEFAULT_WIRE_MM
    };
    class.and_then(|style| style.wire_width_mm).unwrap_or(own)
}

/// One drawn piece of a patterned line: start and end points.
type Mark = ((f64, f64), (f64, f64));

/// Split the segment `a`-`b` into the marks of `line`, in the segment's own
/// units. `width` is the stroke width in the same units. A solid line, or a
/// segment too short to hold a gap, comes back whole.
pub(super) fn line_marks(
    a: (f64, f64),
    b: (f64, f64),
    line: NetLineStyle,
    width: f64,
) -> Vec<Mark> {
    let (mark, gap) = match line {
        NetLineStyle::Solid => return vec![(a, b)],
        NetLineStyle::Dashed => DASH,
        NetLineStyle::Dotted => DOT,
    };
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let length = dx.hypot(dy);
    let (mark, gap) = (mark * width, gap * width);
    if length <= mark || width <= 0.0 {
        return vec![(a, b)];
    }
    let (ux, uy) = (dx / length, dy / length);
    let at = |d: f64| (a.0 + ux * d, a.1 + uy * d);
    let mut out = Vec::new();
    let mut start = 0.0;
    while start < length {
        let end = (start + mark).min(length);
        out.push((at(start), at(end)));
        start += mark + gap;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wire(id: u128, width: f64) -> Wire {
        Wire {
            uuid: Uuid::from_u128(id),
            stroke_width: width,
            ..Default::default()
        }
    }

    fn style(color: Option<Color>, width: Option<f64>) -> NetClassStyle {
        NetClassStyle {
            color,
            wire_width_mm: width,
            line: NetLineStyle::Dashed,
        }
    }

    #[test]
    fn override_beats_class_beats_palette() {
        let w = wire(1, 0.0);
        let (red, blue) = (Color::new(255, 0, 0, 255), Color::new(0, 0, 255, 255));
        let palette = (0.5, 0.5, 0.5);
        let mut classes = HashMap::new();
        classes.insert(w.uuid, style(Some(blue), None));
        let mut overrides = HashMap::new();

        assert_eq!(wire_rgb(&w, &overrides, &HashMap::new(), palette), palette);
        assert_eq!(wire_rgb(&w, &overrides, &classes, palette), (0.0, 0.0, 1.0));
        overrides.insert(w.uuid, red);
        assert_eq!(wire_rgb(&w, &overrides, &classes, palette), (1.0, 0.0, 0.0));
    }

    #[test]
    fn class_width_replaces_own_and_default_width() {
        assert_eq!(wire_width_mm(&wire(1, 0.0), None), 0.15);
        assert_eq!(wire_width_mm(&wire(1, 0.3), None), 0.3);
        assert_eq!(
            wire_width_mm(&wire(1, 0.3), Some(&style(None, Some(0.8)))),
            0.8
        );
        assert_eq!(wire_width_mm(&wire(1, 0.3), Some(&style(None, None))), 0.3);
    }

    #[test]
    fn solid_is_one_whole_segment() {
        let marks = line_marks((0.0, 0.0), (10.0, 0.0), NetLineStyle::Solid, 0.5);
        assert_eq!(marks, vec![((0.0, 0.0), (10.0, 0.0))]);
    }

    #[test]
    fn dashed_splits_with_gaps_and_stays_on_the_segment() {
        let marks = line_marks((0.0, 0.0), (20.0, 0.0), NetLineStyle::Dashed, 0.5);
        // mark 4.0, gap 2.5: starts at 0, 6.5, 13.0, 19.5.
        assert_eq!(marks.len(), 4);
        assert_eq!(marks[0], ((0.0, 0.0), (4.0, 0.0)));
        assert_eq!(marks[1].0, (6.5, 0.0));
        assert_eq!(marks[3], ((19.5, 0.0), (20.0, 0.0)));
    }

    #[test]
    fn dotted_has_shorter_marks_than_dashed() {
        let dashed = line_marks((0.0, 0.0), (20.0, 0.0), NetLineStyle::Dashed, 0.5);
        let dotted = line_marks((0.0, 0.0), (20.0, 0.0), NetLineStyle::Dotted, 0.5);
        let mark = |m: &[Mark]| m[0].1.0 - m[0].0.0;
        assert!(mark(&dotted) < mark(&dashed));
        assert!(dotted.len() > dashed.len());
    }

    #[test]
    fn a_diagonal_segment_is_split_along_its_direction() {
        let marks = line_marks((0.0, 0.0), (30.0, 40.0), NetLineStyle::Dashed, 1.0);
        let (s, e) = marks[0];
        assert!((e.0 - s.0) * 4.0 - (e.1 - s.1) * 3.0 < 1e-9);
    }

    #[test]
    fn a_segment_shorter_than_one_mark_stays_whole() {
        let marks = line_marks((0.0, 0.0), (1.0, 0.0), NetLineStyle::Dashed, 0.5);
        assert_eq!(marks.len(), 1);
    }
}
