//! A board assembly drawing as an SVG file (#2486): the marks of
//! [`crate::assembly`] in board mm on white paper, the bottom side mirrored
//! if asked. One unit of the view box is one board mm, so the file prints
//! 1:1. Rules: `docs/BOARD_OUTPUT.md`, "Assembly drawings".

use std::fmt::Write as _;

use signex_types::pcb::Point;

use crate::assembly::{AssemblyDrawing, AssemblyInk, AssemblyMark};
use crate::markup_escape::escape_markup;

/// Blank paper around the board, mm.
const MARGIN_MM: f64 = 2.0;
/// The thinnest line drawn, mm, so a stroke saved with no width shows.
const HAIRLINE_MM: f64 = 0.05;
/// The font a text asks for; a viewer falls back to its sans-serif.
const FONT_FAMILY: &str = "Roboto, Arial, sans-serif";

/// How an assembly drawing is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AssemblySvgOptions {
    /// Drawn left to right reversed, as seen from below.
    pub mirrored: bool,
    /// Every mark in black.
    pub monochrome: bool,
}

/// Where a board point lands in the file.
struct Frame {
    min: Point,
    max: Point,
    mirrored: bool,
}

impl Frame {
    fn at(&self, p: Point) -> (f64, f64) {
        let u = if self.mirrored {
            self.max.x - p.x
        } else {
            p.x - self.min.x
        };
        (u + MARGIN_MM, p.y - self.min.y + MARGIN_MM)
    }
}

/// `drawing` as a standalone SVG document.
pub fn assembly_svg(drawing: &AssemblyDrawing, options: &AssemblySvgOptions) -> String {
    let frame = Frame {
        min: drawing.min,
        max: drawing.max,
        mirrored: options.mirrored,
    };
    let w = drawing.max.x - drawing.min.x + 2.0 * MARGIN_MM;
    let h = drawing.max.y - drawing.min.y + 2.0 * MARGIN_MM;
    let (w, h) = (num(w), num(h));
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
        write_mark(&mut out, &frame, mark, options);
    }
    out.push_str("</g>\n</svg>\n");
    out
}

fn write_mark(out: &mut String, frame: &Frame, mark: &AssemblyMark, options: &AssemblySvgOptions) {
    let colour = |ink: AssemblyInk| hex(ink.rgb(options.monochrome));
    match mark {
        AssemblyMark::Stroke {
            points,
            closed,
            width_mm,
            ink,
        } => {
            if points.len() < 2 {
                return;
            }
            let width = if width_mm.is_finite() {
                width_mm.max(HAIRLINE_MM)
            } else {
                HAIRLINE_MM
            };
            let _ = writeln!(
                out,
                "<path d=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{}\"/>",
                path(frame, points, *closed),
                colour(*ink),
                num(width)
            );
        }
        AssemblyMark::Fill { rings, ink } => {
            let d: Vec<String> = rings
                .iter()
                .filter(|r| r.len() >= 3)
                .map(|r| path(frame, r, true))
                .collect();
            if d.is_empty() {
                return;
            }
            let _ = writeln!(
                out,
                "<path d=\"{}\" fill=\"{}\" fill-rule=\"evenodd\" stroke=\"none\"/>",
                d.join(" "),
                colour(*ink)
            );
        }
        AssemblyMark::Text {
            at,
            rotation_deg,
            size_mm,
            text,
            ink,
        } => {
            if text.trim().is_empty() || !(size_mm.is_finite() && *size_mm > 0.0) {
                return;
            }
            let (x, y) = frame.at(*at);
            // The y axis runs down, so a turn in the board's sense is a
            // clockwise turn here, reversed when the page is mirrored.
            let turn = if frame.mirrored {
                -rotation_deg
            } else {
                *rotation_deg
            };
            let (x, y) = (num(x), num(y));
            let _ = writeln!(
                out,
                "<text x=\"{x}\" y=\"{y}\" font-family=\"{FONT_FAMILY}\" font-size=\"{}\" text-anchor=\"middle\" dominant-baseline=\"central\" fill=\"{}\" transform=\"rotate({} {x} {y})\">{}</text>",
                num(*size_mm),
                colour(*ink),
                num(turn),
                escape_markup(text)
            );
        }
    }
}

fn path(frame: &Frame, points: &[Point], closed: bool) -> String {
    let mut d = String::new();
    for (i, p) in points.iter().enumerate() {
        let (x, y) = frame.at(*p);
        let op = if i == 0 { 'M' } else { 'L' };
        if i > 0 {
            d.push(' ');
        }
        let _ = write!(d, "{op}{} {}", num(x), num(y));
    }
    if closed {
        d.push_str(" Z");
    }
    d
}

/// `#rrggbb` of a 0..1 colour.
pub(crate) fn hex((r, g, b): (f32, f32, f32)) -> String {
    let byte = |c: f32| (c.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{:02x}{:02x}{:02x}", byte(r), byte(g), byte(b))
}

/// A number to the micrometre, trailing zeros dropped, never `-0`.
pub(crate) fn num(v: f64) -> String {
    if !v.is_finite() {
        return "0".to_string();
    }
    let text = format!("{v:.3}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    match text {
        "-0" | "" => "0".to_string(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests;
