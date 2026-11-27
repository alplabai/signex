//! The flag a directive carries on the sheet (#1489): its lines and its box,
//! shared by the canvas and the exports so both draw the same flag. Design
//! note: `docs/SCHEMATIC_DIRECTIVES.md`, "Drawing".

use super::{Aabb, Point, SchDirective};

/// Flag text height, line pitch, inner margin and stem, in mm.
pub const DIRECTIVE_FLAG_TEXT_MM: f64 = 1.27;
pub const DIRECTIVE_FLAG_LINE_MM: f64 = 1.9;
pub const DIRECTIVE_FLAG_PAD_MM: f64 = 0.7;
const FLAG_STEM_MM: f64 = 3.81;
/// The advance of one flag character, generous so the box always holds its
/// text (both mockup flags were narrower than their text).
const FLAG_CHAR_MM: f64 = DIRECTIVE_FLAG_TEXT_MM * 0.66;
/// How far below a Net Blanket its class flag sits, clear of the edge.
const BLANKET_FLAG_GAP_MM: f64 = 2.54;

/// A directive's flag: its box, the stem from the directive to it, and its
/// lines. Fixed English, like every sheet output (DECISIONS 2026-11-07).
#[derive(Debug, Clone, PartialEq)]
pub struct DirectiveFlag {
    pub rect: Aabb,
    pub stem: Option<(Point, Point)>,
    pub lines: Vec<String>,
}

/// `value` mm without trailing zeros.
fn mm(value: f64) -> String {
    let text = format!("{value:.3}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// The lines a directive's flag shows; empty when it has no flag.
pub fn directive_flag_lines(directive: &SchDirective) -> Vec<String> {
    match directive {
        SchDirective::NetParameters {
            rules,
            show_on_flag,
            ..
        } => {
            let mut lines = vec![match directive.net_class() {
                Some(class) => format!("Net Class = {class}"),
                None => "Net Parameters".to_string(),
            }];
            if *show_on_flag {
                let mut sizes = Vec::new();
                if let Some(w) = rules.width_mm {
                    sizes.push(format!("Width {} mm", mm(w)));
                }
                if let Some(c) = rules.clearance_mm {
                    sizes.push(format!("Clr {} mm", mm(c)));
                }
                if !sizes.is_empty() {
                    lines.push(sizes.join(" · "));
                }
                if let (Some(d), Some(h)) = (rules.via_diameter_mm, rules.via_hole_mm) {
                    lines.push(format!("Via {} / {} mm", mm(d), mm(h)));
                }
                if let Some(pair) = directive.diff_pair_class() {
                    lines.push(format!("Diff Pair · {pair}"));
                }
            }
            lines
        }
        SchDirective::Blanket { .. } => directive
            .net_class()
            .map(|class| vec![format!("Net Class = {class}")])
            .unwrap_or_default(),
        SchDirective::DiffPair { .. } => directive
            .diff_pair_class()
            .map(|class| vec![format!("Diff Pair · {class}")])
            .unwrap_or_default(),
        SchDirective::CompileMask { .. } => Vec::new(),
    }
}

/// The flag of `directive`, sized to its text; `None` when it has none.
pub fn directive_flag(directive: &SchDirective) -> Option<DirectiveFlag> {
    let lines = directive_flag_lines(directive);
    let longest = lines.iter().map(|l| l.chars().count()).max()?;
    let w = longest as f64 * FLAG_CHAR_MM + 2.0 * DIRECTIVE_FLAG_PAD_MM;
    let h = lines.len() as f64 * DIRECTIVE_FLAG_LINE_MM + 2.0 * DIRECTIVE_FLAG_PAD_MM;
    let at = directive.anchor();
    let (rect, stem) = match directive {
        // Above the wire, on a stem.
        SchDirective::NetParameters { .. } => {
            let bottom = at.y - FLAG_STEM_MM;
            (
                Aabb::new(at.x, bottom - h, at.x + w, bottom),
                Some((at, Point::new(at.x, bottom))),
            )
        }
        // Below the pair, on a stem.
        SchDirective::DiffPair { .. } => {
            let top = at.y + FLAG_STEM_MM;
            (
                Aabb::new(at.x, top, at.x + w, top + h),
                Some((at, Point::new(at.x, top))),
            )
        }
        // Under the outline's right end, clear of its edge.
        SchDirective::Blanket { .. } => {
            let r = directive.region()?;
            let top = r.max_y + BLANKET_FLAG_GAP_MM;
            (Aabb::new(r.max_x - w, top, r.max_x, top + h), None)
        }
        SchDirective::CompileMask { .. } => return None,
    };
    Some(DirectiveFlag { rect, stem, lines })
}

/// The top-left corner of line `i` of `flag`'s text.
pub fn directive_flag_line_origin(flag: &DirectiveFlag, i: usize) -> Point {
    Point::new(
        flag.rect.min_x + DIRECTIVE_FLAG_PAD_MM,
        flag.rect.min_y + DIRECTIVE_FLAG_PAD_MM + i as f64 * DIRECTIVE_FLAG_LINE_MM,
    )
}
