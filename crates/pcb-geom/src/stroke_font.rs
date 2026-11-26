//! The board's stroke font (#2368): every glyph a few strokes on a small
//! grid, so a text can be written where only strokes exist (a Gerber image).
//! Rules: `docs/RENDERING_RULES.md`, "pcb-stroke-font".
//!
//! The grid is `GRID_CAP` units to the cap height, y up from the baseline;
//! a glyph is `GRID_WIDTH` wide and the next one starts `GRID_ADVANCE` on.
//! Lower case reaches `GRID_X_HEIGHT`, descenders go two units below the
//! baseline. A glyph is polylines: points `x,y` apart by spaces, polylines
//! apart by `|`. A one-point polyline is a dot.

use signex_types::schematic::HAlign;

use crate::text::TextPlacement;
use crate::units::{GeomError, PointNm};

/// Grid units to the cap height: the text height.
const GRID_CAP: f64 = 8.0;
/// Grid units from one glyph's left edge to the next one's.
const GRID_ADVANCE: f64 = 6.0;
/// Grid units a glyph is wide.
const GRID_WIDTH: f64 = 4.0;

/// The stroke width, as a share of the text height.
pub const TEXT_STROKE_EM: f64 = 0.15;

/// The pen width a text `size_mm` high is written with, mm.
pub fn text_stroke_width_mm(size_mm: f64) -> f64 {
    size_mm * TEXT_STROKE_EM
}

/// How long a line of `content` is when written `size_mm` high, mm: from
/// the first glyph's left edge to the last one's right edge.
pub fn stroke_text_width_mm(content: &str, size_mm: f64) -> f64 {
    let glyphs = content.chars().count();
    if glyphs == 0 {
        return 0.0;
    }
    let units = GRID_ADVANCE * (glyphs - 1) as f64 + GRID_WIDTH;
    units * size_mm / GRID_CAP
}

/// Where the line starts along its baseline, from the anchor, mm: the
/// anchor is its left end, middle or right end.
pub fn justify_start_mm(justify: HAlign, width_mm: f64) -> f64 {
    match justify {
        HAlign::Left => 0.0,
        HAlign::Center => -width_mm / 2.0,
        HAlign::Right => -width_mm,
    }
}

/// The strokes that write `content` `size_mm` high at `at`, justified by
/// `justify`, its cap height centred on the anchor. Each stroke is a
/// polyline in board nm; a dot is one point. Refuses a text the geometry
/// cannot hold.
pub fn text_strokes(
    content: &str,
    at: &TextPlacement,
    size_mm: f64,
    justify: HAlign,
) -> Result<Vec<Vec<PointNm>>, GeomError> {
    scaled_strokes(content, at, size_mm, justify, 1.0)
}

/// [`text_strokes`] condensed along the line, never stretched, so the line
/// is at most `width_mm` long: a text frame line, whose wrap assumes a
/// narrower advance than this font's.
pub fn text_strokes_within(
    content: &str,
    at: &TextPlacement,
    size_mm: f64,
    justify: HAlign,
    width_mm: f64,
) -> Result<Vec<Vec<PointNm>>, GeomError> {
    let natural = stroke_text_width_mm(content, size_mm);
    let squeeze = if width_mm.is_finite() && natural > width_mm {
        width_mm.max(0.0) / natural
    } else {
        1.0
    };
    scaled_strokes(content, at, size_mm, justify, squeeze)
}

/// The strokes of `content`, each glyph's x and advance scaled by `squeeze`.
fn scaled_strokes(
    content: &str,
    at: &TextPlacement,
    size_mm: f64,
    justify: HAlign,
    squeeze: f64,
) -> Result<Vec<Vec<PointNm>>, GeomError> {
    if !(size_mm.is_finite() && size_mm > 0.0) {
        return Err(GeomError::Unrepresentable("text.size"));
    }
    let unit = size_mm / GRID_CAP;
    let along = unit * squeeze;
    let start = justify_start_mm(justify, stroke_text_width_mm(content, size_mm) * squeeze);
    let mut strokes = Vec::new();
    for (n, glyph) in content.chars().enumerate() {
        let left = start + n as f64 * GRID_ADVANCE * along;
        for polyline in glyph_strokes(glyph) {
            let points = polyline
                .iter()
                .map(|&(x, y)| {
                    // Grid y is up from the baseline; the text's own y is
                    // down, with the cap height centred on the anchor.
                    at.offset_mm(left + x * along, (GRID_CAP / 2.0 - y) * unit)
                })
                .collect::<Result<Vec<_>, _>>()?;
            if !points.is_empty() {
                strokes.push(points);
            }
        }
    }
    Ok(strokes)
}

/// The polylines of `glyph` in grid units. A glyph the font lacks is a box.
fn glyph_strokes(glyph: char) -> Vec<Vec<(f64, f64)>> {
    let source = glyph_source(glyph).unwrap_or(MISSING);
    source
        .split('|')
        .filter(|polyline| !polyline.trim().is_empty())
        .map(|polyline| {
            polyline
                .split_whitespace()
                .filter_map(|point| {
                    let (x, y) = point.split_once(',')?;
                    Some((x.parse().ok()?, y.parse().ok()?))
                })
                .collect()
        })
        .collect()
}

/// The box a glyph the font lacks is drawn as.
const MISSING: &str = "0,0 4,0 4,8 0,8 0,0";

fn glyph_source(glyph: char) -> Option<&'static str> {
    Some(match glyph {
        ' ' => "",
        '0' => "1,0 3,0 4,1 4,7 3,8 1,8 0,7 0,1 1,0|0,1 4,7",
        '1' => "1,6 2,8 2,0|1,0 3,0",
        '2' => "0,7 1,8 3,8 4,7 4,5 0,0 4,0",
        '3' => "0,7 1,8 3,8 4,7 4,5 3,4 1,4|3,4 4,3 4,1 3,0 1,0 0,1",
        '4' => "3,0 3,8 0,2 4,2",
        '5' => "4,8 0,8 0,4 3,4 4,3 4,1 3,0 0,0",
        '6' => "4,7 3,8 1,8 0,7 0,1 1,0 3,0 4,1 4,3 3,4 0,4",
        '7' => "0,8 4,8 1,0",
        '8' => "1,4 0,5 0,7 1,8 3,8 4,7 4,5 3,4 1,4 0,3 0,1 1,0 3,0 4,1 4,3 3,4",
        '9' => "0,1 1,0 3,0 4,1 4,7 3,8 1,8 0,7 0,5 1,4 4,4",
        'A' => "0,0 0,6 2,8 4,6 4,0|0,4 4,4",
        'B' => "0,0 0,8 3,8 4,7 4,5 3,4 0,4|3,4 4,3 4,1 3,0 0,0",
        'C' => "4,7 3,8 1,8 0,7 0,1 1,0 3,0 4,1",
        'D' => "0,0 0,8 3,8 4,7 4,1 3,0 0,0",
        'E' => "4,8 0,8 0,0 4,0|0,4 3,4",
        'F' => "4,8 0,8 0,0|0,4 3,4",
        'G' => "4,7 3,8 1,8 0,7 0,1 1,0 3,0 4,1 4,4 2,4",
        'H' => "0,0 0,8|4,0 4,8|0,4 4,4",
        'I' => "1,8 3,8|2,8 2,0|1,0 3,0",
        'J' => "2,8 4,8|3,8 3,1 2,0 1,0 0,1",
        'K' => "0,0 0,8|4,8 0,3|1,4 4,0",
        'L' => "0,8 0,0 4,0",
        'M' => "0,0 0,8 2,5 4,8 4,0",
        'N' => "0,0 0,8 4,0 4,8",
        'O' => "1,0 3,0 4,1 4,7 3,8 1,8 0,7 0,1 1,0",
        'P' => "0,0 0,8 3,8 4,7 4,5 3,4 0,4",
        'Q' => "1,0 3,0 4,1 4,7 3,8 1,8 0,7 0,1 1,0|2,2 4,0",
        'R' => "0,0 0,8 3,8 4,7 4,5 3,4 0,4|2,4 4,0",
        'S' => "4,7 3,8 1,8 0,7 0,5 1,4 3,4 4,3 4,1 3,0 1,0 0,1",
        'T' => "0,8 4,8|2,8 2,0",
        'U' => "0,8 0,1 1,0 3,0 4,1 4,8",
        'V' => "0,8 2,0 4,8",
        'W' => "0,8 1,0 2,4 3,0 4,8",
        'X' => "0,0 4,8|0,8 4,0",
        'Y' => "0,8 2,4 4,8|2,4 2,0",
        'Z' => "0,8 4,8 0,0 4,0",
        'a' => "1,5 3,5 4,4 4,0|4,3 1,3 0,2 0,1 1,0 3,0 4,1",
        'b' => "0,8 0,0 3,0 4,1 4,4 3,5 0,5",
        'c' => "4,5 1,5 0,4 0,1 1,0 4,0",
        'd' => "4,8 4,0 1,0 0,1 0,4 1,5 4,5",
        'e' => "0,3 4,3 4,4 3,5 1,5 0,4 0,1 1,0 4,0",
        'f' => "4,8 3,8 2,7 2,0|0,5 4,5",
        'g' => "4,0 1,0 0,1 0,4 1,5 4,5 4,-1 3,-2 0,-2",
        'h' => "0,8 0,0|0,4 1,5 3,5 4,4 4,0",
        'i' => "1,5 2,5 2,0|1,0 3,0|2,7",
        'j' => "2,5 3,5 3,-1 2,-2 1,-2|3,7",
        'k' => "0,8 0,0|4,5 0,2|1,3 4,0",
        'l' => "1,8 2,8 2,1 3,0",
        'm' => "0,0 0,5|0,4 1,5 2,4 2,0|2,4 3,5 4,4 4,0",
        'n' => "0,0 0,5|0,4 1,5 3,5 4,4 4,0",
        'o' => "1,0 3,0 4,1 4,4 3,5 1,5 0,4 0,1 1,0",
        'p' => "0,-2 0,5 3,5 4,4 4,1 3,0 0,0",
        'q' => "4,-2 4,5 1,5 0,4 0,1 1,0 4,0",
        'r' => "0,0 0,5|0,3 2,5 4,5",
        's' => "4,5 1,5 0,4 1,3 3,2 4,1 3,0 0,0",
        't' => "2,7 2,1 3,0 4,0|0,5 4,5",
        'u' => "0,5 0,1 1,0 3,0 4,1|4,5 4,0",
        'v' => "0,5 2,0 4,5",
        'w' => "0,5 1,0 2,3 3,0 4,5",
        'x' => "0,0 4,5|0,5 4,0",
        'y' => "0,5 2,0|4,5 1,-2",
        'z' => "0,5 4,5 0,0 4,0",
        '!' => "2,8 2,2|2,0",
        '"' => "1,8 1,6|3,8 3,6",
        '#' => "1,0 1,8|3,0 3,8|0,3 4,3|0,5 4,5",
        '$' => "4,7 1,7 0,6 1,4 3,4 4,3 3,1 0,1|2,8 2,0",
        '%' => "0,0 4,8|0,8 1,8 1,7 0,7 0,8|3,1 4,1 4,0 3,0 3,1",
        '&' => "4,0 0,6 1,8 2,8 3,7 3,6 0,3 0,1 1,0 2,0 4,3",
        '\'' => "2,8 2,6",
        '(' => "3,8 2,7 1,5 1,3 2,1 3,0",
        ')' => "1,8 2,7 3,5 3,3 2,1 1,0",
        '*' => "2,6 2,2|0,5 4,3|0,3 4,5",
        '+' => "2,6 2,2|0,4 4,4",
        ',' => "2,1 2,0 1,-1",
        '-' => "0,4 4,4",
        '.' => "2,0",
        '/' => "0,0 4,8",
        ':' => "2,5|2,1",
        ';' => "2,5|2,1 2,0 1,-1",
        '<' => "4,7 0,4 4,1",
        '=' => "0,5 4,5|0,3 4,3",
        '>' => "0,7 4,4 0,1",
        '?' => "0,7 1,8 3,8 4,7 4,6 2,4 2,2|2,0",
        '@' => "3,3 3,5 2,5 1,4 1,3 2,2 3,3 4,3 4,7 3,8 1,8 0,7 0,1 1,0 4,0",
        '[' => "3,8 1,8 1,0 3,0",
        '\\' => "0,8 4,0",
        ']' => "1,8 3,8 3,0 1,0",
        '^' => "0,6 2,8 4,6",
        '_' => "0,-1 4,-1",
        '`' => "1,8 2,7",
        '{' => "3,8 2,7 2,5 1,4 2,3 2,1 3,0",
        '|' => "2,8 2,0",
        '}' => "1,8 2,7 2,5 3,4 2,3 2,1 1,0",
        '~' => "0,4 1,5 3,3 4,4",
        '\u{b5}' => "0,-2 0,5|0,1 1,0 3,0 4,1|4,5 4,0",
        '\u{3a9}' => "0,0 1,0 1,1 0,3 0,6 1,8 3,8 4,6 4,3 3,1 3,0 4,0",
        '\u{b0}' => "1,7 2,8 3,7 2,6 1,7",
        '\u{b1}' => "2,7 2,3|0,5 4,5|0,1 4,1",
        _ => return None,
    })
}

#[cfg(test)]
#[path = "stroke_font_tests.rs"]
mod tests;
