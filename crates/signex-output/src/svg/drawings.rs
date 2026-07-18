//! Free-standing schematic drawing emitter.
//!
//! Converts each `SchDrawing` (line / rect / polyline / circle / arc)
//! into `SvgElement` path primitives.
//!
//! Extracted verbatim from the SVG exporter (`svg/mod.rs`); pure code
//! motion, zero behaviour change.

use super::*;
use super::geometry::{arc_path_commands, circle_path, rect_path};
use crate::pdf::layout::PageTransform;
use crate::pdf::palette::SchematicPalette;
use signex_types::schematic::{FillType, SchDrawing};

pub(super) fn push_sch_drawing_path(
    out: &mut Vec<SvgElement>,
    drawing: &SchDrawing,
    xform: &PageTransform,
    palette: &SchematicPalette,
) {
    let stroke = palette.symbol_stroke;
    let body_fill = palette.symbol_fill;
    match drawing {
        SchDrawing::Line {
            start, end, width, ..
        } => {
            let w_mm = if *width > 0.0 { *width } else { 0.15 };
            out.push(SvgElement::Path {
                commands: vec![
                    SvgPathCommand::MoveTo(pt(xform.x(start.x), xform.px_y(start.y))),
                    SvgPathCommand::LineTo(pt(xform.x(end.x), xform.px_y(end.y))),
                ],
                style: SvgStyle {
                    stroke_rgb: Some(stroke),
                    fill_rgb: None,
                    stroke_width: (w_mm * xform.mm_to_unit) as f32,
                },
            });
        }
        SchDrawing::Rect {
            start,
            end,
            width,
            fill,
            ..
        } => {
            let w_mm = if *width > 0.0 { *width } else { 0.15 };
            let x1 = xform.x(start.x).min(xform.x(end.x));
            let y1 = xform.px_y(start.y).min(xform.px_y(end.y));
            let x2 = xform.x(start.x).max(xform.x(end.x));
            let y2 = xform.px_y(start.y).max(xform.px_y(end.y));
            out.push(rect_path(
                x1,
                y1,
                x2 - x1,
                y2 - y1,
                SvgStyle {
                    stroke_rgb: Some(stroke),
                    fill_rgb: fill_to_rgb(*fill, stroke, body_fill),
                    stroke_width: (w_mm * xform.mm_to_unit) as f32,
                },
            ));
        }
        SchDrawing::Polyline {
            points,
            width,
            fill,
            ..
        } => {
            if points.len() < 2 {
                return;
            }
            let w_mm = if *width > 0.0 { *width } else { 0.15 };
            let mut cmds = Vec::with_capacity(points.len() + 1);
            cmds.push(SvgPathCommand::MoveTo(pt(
                xform.x(points[0].x),
                xform.px_y(points[0].y),
            )));
            for p in &points[1..] {
                cmds.push(SvgPathCommand::LineTo(pt(xform.x(p.x), xform.px_y(p.y))));
            }
            if matches!(fill, FillType::Outline | FillType::Background) && points.len() > 2 {
                cmds.push(SvgPathCommand::Close);
            }
            out.push(SvgElement::Path {
                commands: cmds,
                style: SvgStyle {
                    stroke_rgb: Some(stroke),
                    fill_rgb: fill_to_rgb(*fill, stroke, body_fill),
                    stroke_width: (w_mm * xform.mm_to_unit) as f32,
                },
            });
        }
        SchDrawing::Circle {
            center,
            radius,
            width,
            fill,
            ..
        } => {
            let w_mm = if *width > 0.0 { *width } else { 0.15 };
            let (cx, cy) = (xform.x(center.x), xform.px_y(center.y));
            let r = (radius * xform.mm_to_unit).abs() as f32;
            out.push(circle_path(
                cx,
                cy,
                r,
                SvgStyle {
                    stroke_rgb: Some(stroke),
                    fill_rgb: fill_to_rgb(*fill, stroke, body_fill),
                    stroke_width: (w_mm * xform.mm_to_unit) as f32,
                },
            ));
        }
        SchDrawing::Arc {
            start,
            mid,
            end,
            width,
            fill,
            ..
        } => {
            let w_mm = if *width > 0.0 { *width } else { 0.15 };
            let mut cmds = arc_path_commands(
                pt(xform.x(start.x), xform.px_y(start.y)),
                pt(xform.x(mid.x), xform.px_y(mid.y)),
                pt(xform.x(end.x), xform.px_y(end.y)),
            );
            if matches!(fill, FillType::Outline | FillType::Background) {
                cmds.push(SvgPathCommand::Close);
            }
            out.push(SvgElement::Path {
                commands: cmds,
                style: SvgStyle {
                    stroke_rgb: Some(stroke),
                    fill_rgb: fill_to_rgb(*fill, stroke, body_fill),
                    stroke_width: (w_mm * xform.mm_to_unit) as f32,
                },
            });
        }
    }
}
