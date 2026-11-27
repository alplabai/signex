//! A block diagram as an SVG file (#2796, mockup row 171): the marks of
//! [`crate::block_diagram`] on white paper. One unit of the view box is one
//! diagram mm, so the file prints 1:1. The same diagram always gives the
//! same bytes. Rules: `docs/BLOCK_DIAGRAM_EXPORT.md`.

use std::fmt::Write as _;

use signex_types::schematic::Point;

use super::board::{hex, num};
use crate::block_diagram::{BlockDiagramDrawing, BlockMark};
use crate::markup_escape::escape_markup;

/// Blank paper around the drawing, mm.
const MARGIN_MM: f64 = 5.0;
/// The font a text asks for; a viewer falls back to its sans-serif.
const FONT_FAMILY: &str = "Roboto, Arial, sans-serif";

/// `drawing` as a standalone SVG document.
pub fn block_diagram_svg(drawing: &BlockDiagramDrawing) -> String {
    let origin = drawing.min;
    let w = num(drawing.max.x - drawing.min.x + 2.0 * MARGIN_MM);
    let h = num(drawing.max.y - drawing.min.y + 2.0 * MARGIN_MM);
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    let _ = writeln!(
        out,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}mm\" height=\"{h}mm\" viewBox=\"0 0 {w} {h}\">"
    );
    let _ = writeln!(
        out,
        "<rect x=\"0\" y=\"0\" width=\"{w}\" height=\"{h}\" fill=\"#ffffff\"/>"
    );
    out.push_str("<g stroke-linecap=\"round\" stroke-linejoin=\"round\">\n");
    for mark in &drawing.marks {
        write_mark(&mut out, origin, mark);
    }
    out.push_str("</g>\n</svg>\n");
    out
}

fn at(origin: Point, p: Point) -> (String, String) {
    (
        num(p.x - origin.x + MARGIN_MM),
        num(p.y - origin.y + MARGIN_MM),
    )
}

fn write_mark(out: &mut String, origin: Point, mark: &BlockMark) {
    match mark {
        BlockMark::Stroke {
            points,
            closed,
            width_mm,
            rgb,
        } => {
            if points.len() < 2 {
                return;
            }
            let _ = writeln!(
                out,
                "<path d=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{}\"/>",
                path(origin, points, *closed),
                hex(*rgb),
                num(*width_mm)
            );
        }
        BlockMark::Fill { ring, rgb } => {
            if ring.len() < 3 {
                return;
            }
            let _ = writeln!(
                out,
                "<path d=\"{}\" fill=\"{}\" stroke=\"none\"/>",
                path(origin, ring, true),
                hex(*rgb)
            );
        }
        BlockMark::Text {
            at: point,
            size_mm,
            text,
            rgb,
        } => {
            if text.trim().is_empty() || !(size_mm.is_finite() && *size_mm > 0.0) {
                return;
            }
            let (x, y) = at(origin, *point);
            let _ = writeln!(
                out,
                "<text x=\"{x}\" y=\"{y}\" font-family=\"{FONT_FAMILY}\" font-size=\"{}\" text-anchor=\"middle\" dominant-baseline=\"central\" fill=\"{}\">{}</text>",
                num(*size_mm),
                hex(*rgb),
                escape_markup(text)
            );
        }
    }
}

fn path(origin: Point, points: &[Point], closed: bool) -> String {
    let mut d = String::new();
    for (i, p) in points.iter().enumerate() {
        let (x, y) = at(origin, *p);
        if i > 0 {
            d.push(' ');
        }
        let op = if i == 0 { 'M' } else { 'L' };
        let _ = write!(d, "{op}{x} {y}");
    }
    if closed {
        d.push_str(" Z");
    }
    d
}
