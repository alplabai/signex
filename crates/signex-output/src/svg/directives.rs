//! Schematic directives on paper (#2656), drawn as on the canvas in the
//! directive colour (`no_connect`): a Net Blanket as a dashed outline and a
//! Net Parameters directive as a flag on a stem. Compile Masks and
//! Differential Pairs are not printed. Flags come from the one geometry the
//! canvas uses (`signex_types::schematic::directive_flag`). PDF pages and
//! print are drawn from these elements. Design note:
//! `docs/SCHEMATIC_DIRECTIVES.md`, "Drawing".

use signex_types::net_class::NetLineStyle;
use signex_types::schematic::{
    Aabb, DIRECTIVE_FLAG_TEXT_MM, Point, SCHEMATIC_NO_CONNECT_STROKE_MM, SchDirective,
    SchematicSheet, directive_flag, directive_flag_line_origin,
};

use super::family::alias_for_family;
use super::labels::label_size_pt;
use super::wire_style::line_marks;
use super::{SvgElement, SvgPathCommand, SvgStyle, SvgTextAlign, SvgTextVAlign, pt};
use crate::pdf::PdfOptions;
use crate::pdf::layout::PageTransform;
use crate::pdf::palette::SchematicPalette;

/// Whether the export options draw `directive`: Net Blankets follow
/// "Blankets", Net Parameters directives "Parameter Sets". A Compile Mask and
/// a Differential Pair are never printed (#2656 asks for the other two only).
fn is_drawn(directive: &SchDirective, opts: &PdfOptions) -> bool {
    match directive {
        SchDirective::Blanket { .. } => opts.include_blankets,
        SchDirective::NetParameters { .. } => opts.include_parameter_sets,
        SchDirective::CompileMask { .. } | SchDirective::DiffPair { .. } => false,
    }
}

/// Append every directive of `sheet` the options draw to `elements`.
pub(super) fn push_directives(
    elements: &mut Vec<SvgElement>,
    sheet: &SchematicSheet,
    xform: &PageTransform,
    palette: &SchematicPalette,
    opts: &PdfOptions,
) {
    let style = SvgStyle {
        stroke_alpha: 1.0,
        stroke_rgb: Some(palette.no_connect),
        fill_rgb: None,
        stroke_width: (SCHEMATIC_NO_CONNECT_STROKE_MM * xform.mm_to_unit) as f32,
    };
    let face = alias_for_family(None, sheet.font_family.as_deref(), "F3");
    for directive in sheet.directives.iter().filter(|d| is_drawn(d, opts)) {
        let mut commands = Vec::new();
        if let Some(r) = directive.region() {
            let dashed = NetLineStyle::Dashed;
            for (a, b) in edges(&r) {
                push_marks(&mut commands, xform, a, b, dashed, style.stroke_width);
            }
        }
        let flag = directive_flag(directive);
        if let Some(flag) = &flag {
            if let Some((a, b)) = flag.stem {
                push_segment(&mut commands, xform, a, b);
            }
            for (a, b) in edges(&flag.rect) {
                push_segment(&mut commands, xform, a, b);
            }
        }
        if !commands.is_empty() {
            elements.push(SvgElement::Path { commands, style });
        }
        let Some(flag) = flag else {
            continue;
        };
        for (i, line) in flag.lines.iter().enumerate() {
            // The first line is bold, as on the canvas.
            let font_alias = match (i, face) {
                (0, "F1") => "F2",
                (0, _) => "F4",
                _ => face,
            };
            let at = directive_flag_line_origin(&flag, i);
            elements.push(SvgElement::Text {
                x: xform.x(at.x),
                y: xform.px_y(at.y),
                font_alias,
                size_pt: label_size_pt(
                    DIRECTIVE_FLAG_TEXT_MM,
                    xform.mm_to_unit,
                    &opts.scale,
                    font_alias,
                ),
                align: SvgTextAlign::Left,
                v_align: SvgTextVAlign::Top,
                rotation_deg: 0.0,
                fill_rgb: palette.no_connect,
                text: line.clone(),
            });
        }
    }
}

/// The four edges of `r`.
fn edges(r: &Aabb) -> [(Point, Point); 4] {
    let corners = [
        Point::new(r.min_x, r.min_y),
        Point::new(r.max_x, r.min_y),
        Point::new(r.max_x, r.max_y),
        Point::new(r.min_x, r.max_y),
    ];
    [0, 1, 2, 3].map(|i| (corners[i], corners[(i + 1) % 4]))
}

fn push_segment(out: &mut Vec<SvgPathCommand>, xform: &PageTransform, a: Point, b: Point) {
    out.push(SvgPathCommand::MoveTo(pt(xform.x(a.x), xform.px_y(a.y))));
    out.push(SvgPathCommand::LineTo(pt(xform.x(b.x), xform.px_y(b.y))));
}

/// The edge `a`-`b` split into the marks of `line`, in page units.
fn push_marks(
    out: &mut Vec<SvgPathCommand>,
    xform: &PageTransform,
    a: Point,
    b: Point,
    line: NetLineStyle,
    width: f32,
) {
    let from = (f64::from(xform.x(a.x)), f64::from(xform.px_y(a.y)));
    let to = (f64::from(xform.x(b.x)), f64::from(xform.px_y(b.y)));
    for (p, q) in line_marks(from, to, line, f64::from(width)) {
        out.push(SvgPathCommand::MoveTo(pt(p.0 as f32, p.1 as f32)));
        out.push(SvgPathCommand::LineTo(pt(q.0 as f32, q.1 as f32)));
    }
}

#[cfg(test)]
#[path = "directives_tests.rs"]
mod tests;
