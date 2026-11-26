//! Free-standing schematic drawing emitter.
//!
//! Converts each `SchDrawing` (line / rect / polyline / circle / arc)
//! into `SvgElement` path primitives.
//!
//! Extracted verbatim from the SVG exporter (`svg/mod.rs`); pure code
//! motion, zero behaviour change.

use std::collections::BTreeSet;

use super::geometry::{arc_path_commands, circle_path, rect_path};
use super::*;
use crate::pdf::layout::PageTransform;
use crate::pdf::palette::SchematicPalette;
use signex_types::schematic::{
    DIMENSION_TEXT_MM, DrawingPlacement, FillType, Point, SchBezier, SchDimension, SchDrawing,
    SchematicDefaults, SchematicSheet, StrokeColor, power_net_names,
};

/// The drawings of `placement`, in file order; an image's picture comes
/// from `images` (#1116).
pub(super) fn push_placed_drawings(
    out: &mut Vec<SvgElement>,
    sheet: &SchematicSheet,
    placement: DrawingPlacement,
    xform: &PageTransform,
    palette: &SchematicPalette,
    defaults: &SchematicDefaults,
    images: &super::ExportImages,
) {
    let power_nets = power_net_names(sheet);
    for drawing in &sheet.drawings {
        if sheet.drawing_placement(drawing.uuid()) == placement {
            if let SchDrawing::Image(image) = drawing {
                super::images::push_image_picture(out, image, xform, images);
            }
            push_sch_drawing_path(out, drawing, xform, palette, defaults, &power_nets);
        }
    }
}

pub(super) fn push_sch_drawing_path(
    out: &mut Vec<SvgElement>,
    drawing: &SchDrawing,
    xform: &PageTransform,
    palette: &SchematicPalette,
    defaults: &SchematicDefaults,
    power_nets: &BTreeSet<String>,
) {
    let stroke = palette.symbol_stroke;
    let body_fill = palette.symbol_fill;
    match drawing {
        SchDrawing::Bezier(b) => push_bezier(out, b, xform, palette, defaults),
        SchDrawing::Dimension(d) => push_dimension(out, d, xform, palette, defaults),
        SchDrawing::Table(t) => {
            super::sch_table::push_table(out, t, xform, palette, defaults, power_nets);
        }
        // #1116: an image's border line, over the picture
        // `push_placed_drawings` put down (docs/SCHEMATIC_IMAGES.md, "Export").
        SchDrawing::Image(i) => {
            if !i.border {
                return;
            }
            let (lo, hi) = i.bounds();
            let w_mm = defaults.shape_stroke_mm(0.0);
            let x1 = xform.x(lo.x).min(xform.x(hi.x));
            let y1 = xform.px_y(lo.y).min(xform.px_y(hi.y));
            let x2 = xform.x(lo.x).max(xform.x(hi.x));
            let y2 = xform.px_y(lo.y).max(xform.px_y(hi.y));
            out.push(rect_path(
                x1,
                y1,
                x2 - x1,
                y2 - y1,
                SvgStyle {
                    stroke_alpha: 1.0,
                    stroke_rgb: Some(stroke),
                    fill_rgb: None,
                    stroke_width: (w_mm * xform.mm_to_unit) as f32,
                },
            ));
        }
        SchDrawing::Line {
            start, end, width, ..
        } => {
            let w_mm = defaults.shape_stroke_mm(*width);
            out.push(SvgElement::Path {
                commands: vec![
                    SvgPathCommand::MoveTo(pt(xform.x(start.x), xform.px_y(start.y))),
                    SvgPathCommand::LineTo(pt(xform.x(end.x), xform.px_y(end.y))),
                ],
                style: SvgStyle {
                    stroke_alpha: 1.0,
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
            let w_mm = defaults.shape_stroke_mm(*width);
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
                    stroke_alpha: 1.0,
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
            let w_mm = defaults.shape_stroke_mm(*width);
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
                    stroke_alpha: 1.0,
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
            let w_mm = defaults.shape_stroke_mm(*width);
            let (cx, cy) = (xform.x(center.x), xform.px_y(center.y));
            let r = (radius * xform.mm_to_unit).abs() as f32;
            out.push(circle_path(
                cx,
                cy,
                r,
                SvgStyle {
                    stroke_alpha: 1.0,
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
            let w_mm = defaults.shape_stroke_mm(*width);
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
                    stroke_alpha: 1.0,
                    stroke_rgb: Some(stroke),
                    fill_rgb: fill_to_rgb(*fill, stroke, body_fill),
                    stroke_width: (w_mm * xform.mm_to_unit) as f32,
                },
            });
        }
    }
}

/// A drawing's own colour as an SVG RGB, else `fallback`.
pub(super) fn stroke_rgb(color: Option<StrokeColor>, fallback: (f32, f32, f32)) -> (f32, f32, f32) {
    color.map_or(fallback, |c| {
        (
            f32::from(c.r) / 255.0,
            f32::from(c.g) / 255.0,
            f32::from(c.b) / 255.0,
        )
    })
}

/// Unfilled segments as one path element.
pub(super) fn segments_path(
    segments: &[[Point; 2]],
    xform: &PageTransform,
    rgb: (f32, f32, f32),
    width_mm: f64,
) -> SvgElement {
    let mut commands = Vec::with_capacity(segments.len() * 2);
    for [a, b] in segments {
        commands.push(SvgPathCommand::MoveTo(pt(xform.x(a.x), xform.px_y(a.y))));
        commands.push(SvgPathCommand::LineTo(pt(xform.x(b.x), xform.px_y(b.y))));
    }
    SvgElement::Path {
        commands,
        style: SvgStyle {
            stroke_alpha: 1.0,
            stroke_rgb: Some(rgb),
            fill_rgb: None,
            stroke_width: (width_mm * xform.mm_to_unit) as f32,
        },
    }
}

/// A filled arrowhead `[tip, wing, wing]`.
fn arrow_path(head: &[Point; 3], xform: &PageTransform, rgb: (f32, f32, f32)) -> SvgElement {
    let at = |p: &Point| pt(xform.x(p.x), xform.px_y(p.y));
    SvgElement::Path {
        commands: vec![
            SvgPathCommand::MoveTo(at(&head[0])),
            SvgPathCommand::LineTo(at(&head[1])),
            SvgPathCommand::LineTo(at(&head[2])),
            SvgPathCommand::Close,
        ],
        style: SvgStyle {
            stroke_alpha: 1.0,
            stroke_rgb: Some(rgb),
            fill_rgb: Some(rgb),
            stroke_width: 0.0,
        },
    }
}

/// A Bezier: one cubic path when solid, else its flattened dash marks;
/// then its arrowheads.
fn push_bezier(
    out: &mut Vec<SvgElement>,
    b: &SchBezier,
    xform: &PageTransform,
    palette: &SchematicPalette,
    defaults: &SchematicDefaults,
) {
    let rgb = stroke_rgb(b.stroke_color, palette.symbol_stroke);
    let w_mm = defaults.shape_stroke_mm(b.width);
    let at = |p: Point| pt(xform.x(p.x), xform.px_y(p.y));
    if matches!(b.line_style, signex_types::net_class::NetLineStyle::Solid) {
        out.push(SvgElement::Path {
            commands: vec![
                SvgPathCommand::MoveTo(at(b.start)),
                SvgPathCommand::CubicTo(at(b.c1), at(b.c2), at(b.end)),
            ],
            style: SvgStyle {
                stroke_alpha: 1.0,
                stroke_rgb: Some(rgb),
                fill_rgb: None,
                stroke_width: (w_mm * xform.mm_to_unit) as f32,
            },
        });
    } else {
        let marks: Vec<[Point; 2]> = b
            .flatten()
            .windows(2)
            .flat_map(|w| {
                super::wire_style::line_marks(
                    (w[0].x, w[0].y),
                    (w[1].x, w[1].y),
                    b.line_style,
                    w_mm,
                )
            })
            .map(|(p, q)| [Point::new(p.0, p.1), Point::new(q.0, q.1)])
            .collect();
        out.push(segments_path(&marks, xform, rgb, w_mm));
    }
    for head in b.arrow_heads() {
        out.push(arrow_path(&head, xform, rgb));
    }
}

/// A dimension: extension lines, the dimension line, its heads and text.
fn push_dimension(
    out: &mut Vec<SvgElement>,
    d: &SchDimension,
    xform: &PageTransform,
    palette: &SchematicPalette,
    defaults: &SchematicDefaults,
) {
    let rgb = stroke_rgb(d.stroke_color, palette.symbol_stroke);
    let w_mm = defaults.shape_stroke_mm(d.width);
    let g = d.geometry();
    let mut segments = vec![g.line, g.extensions[0], g.extensions[1]];
    segments.extend(g.outside_tails.iter().copied());
    if !d.arrows.is_filled() {
        for head in &g.arrows {
            segments.push([head[1], head[0]]);
            segments.push([head[0], head[2]]);
        }
    }
    out.push(segments_path(&segments, xform, rgb, w_mm));
    if d.arrows.is_filled() {
        for head in &g.arrows {
            out.push(arrow_path(head, xform, rgb));
        }
    }
    out.push(SvgElement::Text {
        x: xform.x(g.text_at.x),
        y: xform.px_y(g.text_at.y),
        font_alias: "F3",
        size_pt: super::text::em_for_glyph_height(
            "F3",
            (DIMENSION_TEXT_MM * xform.mm_to_unit) as f32,
        ),
        align: SvgTextAlign::Center,
        v_align: if g.text_hangs {
            SvgTextVAlign::Top
        } else {
            SvgTextVAlign::Bottom
        },
        rotation_deg: g.text_angle_deg as f32,
        fill_rgb: rgb,
        text: d.label(),
    });
}

/// Every point an annotation drawing (Bezier, dimension, table, image)
/// reaches, for the page's content box; empty for the classic shapes.
pub(crate) fn annotation_extent(drawing: &SchDrawing) -> Vec<Point> {
    match drawing {
        SchDrawing::Bezier(b) => b.flatten(),
        SchDrawing::Dimension(d) => d.outline_points(),
        SchDrawing::Table(t) => {
            let (lo, hi) = t.bounds();
            vec![lo, hi]
        }
        SchDrawing::Image(i) => {
            let (lo, hi) = i.bounds();
            vec![lo, hi]
        }
        SchDrawing::Line { .. }
        | SchDrawing::Rect { .. }
        | SchDrawing::Circle { .. }
        | SchDrawing::Arc { .. }
        | SchDrawing::Polyline { .. } => Vec::new(),
    }
}
