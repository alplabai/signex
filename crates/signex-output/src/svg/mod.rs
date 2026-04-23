//! Schematic -> SVG path-based intermediate context.
//!
//! This module is the canonical geometry bridge:
//! schematic page -> SVG path elements -> PDF / preview backends.

use std::fmt::Write as _;

use signex_types::schematic::{
    FillType, Graphic, HAlign, LabelType, LibSymbol, Pin, Point, SchDrawing, Symbol, TextProp,
    VAlign,
};
use tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, Stroke};
use ttf_parser::{Face, GlyphId, OutlineBuilder};

use crate::pdf::layout::PageTransform;
use crate::pdf::{ColourMode, PdfOptions, PdfScale};
use crate::SheetSnapshot;

#[derive(Debug, Clone, Copy)]
pub enum SvgTextAlign {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Copy)]
pub enum SvgTextVAlign {
    Top,
    Center,
    Bottom,
}

#[derive(Debug, Clone, Copy)]
pub struct SvgPoint {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone)]
pub enum SvgPathCommand {
    MoveTo(SvgPoint),
    LineTo(SvgPoint),
    CubicTo(SvgPoint, SvgPoint, SvgPoint),
    Close,
}

#[derive(Debug, Clone, Copy)]
pub struct SvgStyle {
    pub stroke_rgb: Option<(f32, f32, f32)>,
    pub fill_rgb: Option<(f32, f32, f32)>,
    pub stroke_width: f32,
}

#[derive(Debug, Clone)]
pub enum SvgElement {
    Path {
        commands: Vec<SvgPathCommand>,
        style: SvgStyle,
    },
    Text {
        x: f32,
        y: f32,
        font_alias: &'static str,
        size_pt: f32,
        align: SvgTextAlign,
        v_align: SvgTextVAlign,
        rotation_deg: f32,
        fill_rgb: (f32, f32, f32),
        text: String,
    },
}

#[derive(Debug, Clone)]
pub struct SvgRenderContext {
    pub width: f32,
    pub height: f32,
    pub elements: Vec<SvgElement>,
    pub svg_document: String,
}

impl SvgRenderContext {
    pub fn from_sheet(
        sheet: &SheetSnapshot,
        opts: &PdfOptions,
        page_w_mm: f64,
        page_h_mm: f64,
        units_per_mm: f64,
    ) -> Self {
        let width = (page_w_mm * units_per_mm) as f32;
        let height = (page_h_mm * units_per_mm) as f32;

        let xform = PageTransform::new(
            sheet,
            page_w_mm,
            page_h_mm,
            &opts.margins,
            &opts.scale,
            units_per_mm,
        );

        let mut elements = Vec::new();

        // Always render the physical sheet boundary first so both preview
        // and PDF clearly reference the document page area.
        elements.push(rect_path(
            0.5,
            0.5,
            (width - 1.0).max(1.0),
            (height - 1.0).max(1.0),
            SvgStyle {
                stroke_rgb: Some((0.78, 0.78, 0.78)),
                fill_rgb: None,
                stroke_width: 1.0,
            },
        ));

        // Base primitives
        for wire in &sheet.schematic.wires {
            let stroke_mm = if wire.stroke_width > 0.0 {
                wire.stroke_width
            } else {
                0.15
            };
            elements.push(SvgElement::Path {
                commands: vec![
                    SvgPathCommand::MoveTo(pt(xform.x(wire.start.x), xform.px_y(wire.start.y))),
                    SvgPathCommand::LineTo(pt(xform.x(wire.end.x), xform.px_y(wire.end.y))),
                ],
                style: SvgStyle {
                    stroke_rgb: Some((0.0, 0.0, 0.0)),
                    fill_rgb: None,
                    stroke_width: (stroke_mm * xform.mm_to_unit) as f32,
                },
            });
        }

        for bus in &sheet.schematic.buses {
            elements.push(SvgElement::Path {
                commands: vec![
                    SvgPathCommand::MoveTo(pt(xform.x(bus.start.x), xform.px_y(bus.start.y))),
                    SvgPathCommand::LineTo(pt(xform.x(bus.end.x), xform.px_y(bus.end.y))),
                ],
                style: SvgStyle {
                    stroke_rgb: Some((0.28, 0.28, 0.28)),
                    fill_rgb: None,
                    stroke_width: (0.3 * xform.mm_to_unit) as f32,
                },
            });
        }

        for entry in &sheet.schematic.bus_entries {
            elements.push(SvgElement::Path {
                commands: vec![
                    SvgPathCommand::MoveTo(pt(
                        xform.x(entry.position.x),
                        xform.px_y(entry.position.y),
                    )),
                    SvgPathCommand::LineTo(pt(
                        xform.x(entry.position.x + entry.size.0),
                        xform.px_y(entry.position.y + entry.size.1),
                    )),
                ],
                style: SvgStyle {
                    stroke_rgb: Some((0.22, 0.22, 0.22)),
                    fill_rgb: None,
                    stroke_width: (0.2 * xform.mm_to_unit) as f32,
                },
            });
        }

        for j in &sheet.schematic.junctions {
            let dia_mm = if j.diameter > 0.0 { j.diameter } else { 0.7 };
            let side = (dia_mm * xform.mm_to_unit) as f32;
            let half = side / 2.0;
            let cx = xform.x(j.position.x);
            let cy = xform.px_y(j.position.y);
            elements.push(rect_path(
                cx - half,
                cy - half,
                side,
                side,
                SvgStyle {
                    stroke_rgb: Some((0.05, 0.05, 0.05)),
                    fill_rgb: Some((0.05, 0.05, 0.05)),
                    stroke_width: 0.8,
                },
            ));
        }

        for nc in &sheet.schematic.no_connects {
            let arm = (1.0 * xform.mm_to_unit) as f32;
            let cx = xform.x(nc.position.x);
            let cy = xform.px_y(nc.position.y);
            let style = SvgStyle {
                stroke_rgb: Some((0.22, 0.22, 0.22)),
                fill_rgb: None,
                stroke_width: 0.8,
            };
            elements.push(SvgElement::Path {
                commands: vec![
                    SvgPathCommand::MoveTo(pt(cx - arm, cy - arm)),
                    SvgPathCommand::LineTo(pt(cx + arm, cy + arm)),
                ],
                style,
            });
            elements.push(SvgElement::Path {
                commands: vec![
                    SvgPathCommand::MoveTo(pt(cx - arm, cy + arm)),
                    SvgPathCommand::LineTo(pt(cx + arm, cy - arm)),
                ],
                style,
            });
        }

        for label in &sheet.schematic.labels {
            let size_pt = label_size_pt(label.font_size, xform.mm_to_unit, &opts.scale);
            elements.push(SvgElement::Text {
                x: xform.x(label.position.x),
                y: xform.px_y(label.position.y),
                font_alias: "F3",
                size_pt,
                align: halign_to_svg(label.justify),
                v_align: SvgTextVAlign::Center,
                rotation_deg: label.rotation as f32,
                fill_rgb: label_colour(label.label_type),
                text: label.text.clone(),
            });
        }

        for note in &sheet.schematic.text_notes {
            let size_pt = label_size_pt(note.font_size, xform.mm_to_unit, &opts.scale);
            elements.push(SvgElement::Text {
                x: xform.x(note.position.x),
                y: xform.px_y(note.position.y),
                font_alias: "F1",
                size_pt,
                align: halign_to_svg(note.justify_h),
                v_align: valign_to_svg(note.justify_v),
                rotation_deg: note.rotation as f32,
                fill_rgb: (0.14, 0.14, 0.14),
                text: note.text.clone(),
            });
        }

        for child in &sheet.schematic.child_sheets {
            let x = xform.x(child.position.x);
            let y = xform.px_y(child.position.y);
            let w = (child.size.0 * xform.mm_to_unit) as f32;
            let h = (child.size.1 * xform.mm_to_unit) as f32;
            elements.push(rect_path(
                x,
                y,
                w,
                h,
                SvgStyle {
                    stroke_rgb: Some((0.25, 0.25, 0.25)),
                    fill_rgb: None,
                    stroke_width: ((if child.stroke_width > 0.0 {
                        child.stroke_width
                    } else {
                        0.2
                    }) * xform.mm_to_unit) as f32,
                },
            ));
            if !child.name.is_empty() {
                elements.push(SvgElement::Text {
                    x: x + 4.0,
                    y: y + 12.0,
                    font_alias: "F1",
                    size_pt: 8.5,
                    align: SvgTextAlign::Left,
                    v_align: SvgTextVAlign::Top,
                    rotation_deg: 0.0,
                    fill_rgb: (0.18, 0.18, 0.18),
                    text: child.name.clone(),
                });
            }
            if !child.filename.is_empty() {
                elements.push(SvgElement::Text {
                    x: x + 4.0,
                    y: y + 22.0,
                    font_alias: "F1",
                    size_pt: 7.5,
                    align: SvgTextAlign::Left,
                    v_align: SvgTextVAlign::Top,
                    rotation_deg: 0.0,
                    fill_rgb: (0.25, 0.25, 0.25),
                    text: child.filename.clone(),
                });
            }
        }

        for drawing in &sheet.schematic.drawings {
            push_sch_drawing_path(&mut elements, drawing, &xform);
        }

        // Full symbol body graphics from library definitions.
        for sym in &sheet.schematic.symbols {
            if let Some(lib) = sheet.schematic.lib_symbols.values().find(|ls| ls.id == sym.lib_id) {
                push_symbol_lib_graphics(&mut elements, sym, lib, &xform);
                push_symbol_pins(&mut elements, sym, lib, &xform);
            } else {
                // Fallback if library is missing.
                elements.push(rect_path(
                    xform.x(sym.position.x - 5.0),
                    xform.px_y(sym.position.y - 5.0),
                    (10.0 * xform.mm_to_unit) as f32,
                    (10.0 * xform.mm_to_unit) as f32,
                    SvgStyle {
                        stroke_rgb: Some((0.22, 0.22, 0.22)),
                        fill_rgb: None,
                        stroke_width: (0.1 * xform.mm_to_unit) as f32,
                    },
                ));
            }

            // Match canvas renderer parity:
            // - power-symbol references (#PWR...) are always hidden
            // - reference/value field visibility follows TextProp.hidden
            if !sym.reference.is_empty()
                && !sym.is_power
                && let Some(ref_text) = &sym.ref_text
                && !ref_text.hidden
            {
                elements.push(SvgElement::Text {
                    x: xform.x(ref_text.position.x),
                    y: xform.px_y(ref_text.position.y),
                    font_alias: "F3",
                    size_pt: (signex_types::schematic::SCHEMATIC_TEXT_MM * xform.mm_to_unit) as f32,
                    align: halign_to_svg(field_effective_style(ref_text, sym).1),
                    v_align: valign_to_svg(field_effective_style(ref_text, sym).2),
                    rotation_deg: field_effective_style(ref_text, sym).0 as f32,
                    fill_rgb: (0.1, 0.1, 0.1),
                    text: sym.reference.clone(),
                });
            }

            if !sym.value.is_empty()
                && let Some(val_text) = &sym.val_text
                && !val_text.hidden
            {
                elements.push(SvgElement::Text {
                    x: xform.x(val_text.position.x),
                    y: xform.px_y(val_text.position.y),
                    font_alias: "F1",
                    size_pt: (signex_types::schematic::SCHEMATIC_TEXT_MM * xform.mm_to_unit) as f32,
                    align: halign_to_svg(field_effective_style(val_text, sym).1),
                    v_align: valign_to_svg(field_effective_style(val_text, sym).2),
                    rotation_deg: field_effective_style(val_text, sym).0 as f32,
                    fill_rgb: (0.2, 0.2, 0.2),
                    text: sym.value.clone(),
                });
            }
        }

        let svg_document = encode_svg_document(width, height, &elements);

        Self {
            width,
            height,
            elements,
            svg_document,
        }
    }

    pub fn rasterize_rgba(&self, width: u32, height: u32) -> Option<Vec<u8>> {
        self.rasterize_rgba_with_colour_mode(width, height, ColourMode::Colour)
    }

    pub fn rasterize_rgba_with_colour_mode(
        &self,
        width: u32,
        height: u32,
        colour_mode: ColourMode,
    ) -> Option<Vec<u8>> {
        let mut pixmap = Pixmap::new(width, height)?;
        pixmap.fill(Color::WHITE);

        for element in &self.elements {
            match element {
                SvgElement::Path { commands, style } => {
                    if let Some(path) = path_to_tiny_skia(commands) {
                        if let Some((r, g, b)) = style.fill_rgb {
                            let mut paint = Paint::default();
                            let (mr, mg, mb) = map_colour_mode((r, g, b), colour_mode);
                            paint.set_color(rgb_to_color(mr, mg, mb));
                            pixmap.fill_path(
                                &path,
                                &paint,
                                FillRule::Winding,
                                Default::default(),
                                None,
                            );
                        }

                        if let Some((r, g, b)) = style.stroke_rgb {
                            let mut paint = Paint::default();
                            let (mr, mg, mb) = map_colour_mode((r, g, b), colour_mode);
                            paint.set_color(rgb_to_color(mr, mg, mb));
                            let stroke = Stroke {
                                width: style.stroke_width.max(0.5),
                                ..Stroke::default()
                            };
                            pixmap.stroke_path(&path, &paint, &stroke, Default::default(), None);
                        }
                    }
                }
                SvgElement::Text {
                    x,
                    y,
                    align,
                    v_align,
                    rotation_deg,
                    size_pt,
                    fill_rgb,
                    text,
                    font_alias,
                } => {
                    let mapped_fill = map_colour_mode(*fill_rgb, colour_mode);
                    draw_text_outline(
                        &mut pixmap,
                        *x,
                        *y,
                        *size_pt,
                        *align,
                        *v_align,
                        *rotation_deg,
                        mapped_fill,
                        font_alias,
                        text,
                    );
                }
            }
        }

        Some(pixmap.data().to_vec())
    }
}

fn push_sch_drawing_path(
    out: &mut Vec<SvgElement>,
    drawing: &SchDrawing,
    xform: &PageTransform,
) {
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
                    stroke_rgb: Some((0.2, 0.2, 0.2)),
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
                    stroke_rgb: Some((0.2, 0.2, 0.2)),
                    fill_rgb: fill_to_rgb(*fill, (0.2, 0.2, 0.2)),
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
                    stroke_rgb: Some((0.2, 0.2, 0.2)),
                    fill_rgb: fill_to_rgb(*fill, (0.2, 0.2, 0.2)),
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
                    stroke_rgb: Some((0.2, 0.2, 0.2)),
                    fill_rgb: fill_to_rgb(*fill, (0.2, 0.2, 0.2)),
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
                    stroke_rgb: Some((0.2, 0.2, 0.2)),
                    fill_rgb: fill_to_rgb(*fill, (0.2, 0.2, 0.2)),
                    stroke_width: (w_mm * xform.mm_to_unit) as f32,
                },
            });
        }
    }
}

fn push_symbol_lib_graphics(
    out: &mut Vec<SvgElement>,
    sym: &Symbol,
    lib: &LibSymbol,
    xform: &PageTransform,
) {
    for lg in &lib.graphics {
        if lg.unit != 0 && lg.unit != sym.unit {
            continue;
        }
        if lg.body_style != 0 && lg.body_style != 1 {
            continue;
        }

        match &lg.graphic {
            Graphic::Polyline {
                points,
                width,
                fill,
            } => {
                if points.len() < 2 {
                    continue;
                }
                let mut cmds = Vec::with_capacity(points.len() + 1);
                let (x0, y0) = symbol_world_point(sym, &points[0]);
                cmds.push(SvgPathCommand::MoveTo(pt(xform.x(x0), xform.px_y(y0))));
                for p in &points[1..] {
                    let (wx, wy) = symbol_world_point(sym, p);
                    cmds.push(SvgPathCommand::LineTo(pt(xform.x(wx), xform.px_y(wy))));
                }
                if points.len() > 2 {
                    let first = &points[0];
                    let last = &points[points.len() - 1];
                    if (first.x - last.x).abs() < 0.001 && (first.y - last.y).abs() < 0.001 {
                        cmds.push(SvgPathCommand::Close);
                    }
                }
                out.push(SvgElement::Path {
                    commands: cmds,
                    style: SvgStyle {
                        stroke_rgb: Some((0.22, 0.22, 0.22)),
                        fill_rgb: fill_to_rgb(*fill, (0.22, 0.22, 0.22)),
                        stroke_width: ((*width).max(0.15) * xform.mm_to_unit) as f32,
                    },
                });
            }
            Graphic::Rectangle {
                start,
                end,
                width,
                fill,
            } => {
                let corners = [
                    Point::new(start.x, start.y),
                    Point::new(end.x, start.y),
                    Point::new(end.x, end.y),
                    Point::new(start.x, end.y),
                ];
                let mut cmds = Vec::with_capacity(5);
                let (wx0, wy0) = symbol_world_point(sym, &corners[0]);
                cmds.push(SvgPathCommand::MoveTo(pt(xform.x(wx0), xform.px_y(wy0))));
                for c in &corners[1..] {
                    let (wx, wy) = symbol_world_point(sym, c);
                    cmds.push(SvgPathCommand::LineTo(pt(xform.x(wx), xform.px_y(wy))));
                }
                cmds.push(SvgPathCommand::Close);
                out.push(SvgElement::Path {
                    commands: cmds,
                    style: SvgStyle {
                        stroke_rgb: Some((0.22, 0.22, 0.22)),
                        fill_rgb: fill_to_rgb(*fill, (0.22, 0.22, 0.22)),
                        stroke_width: ((*width).max(0.15) * xform.mm_to_unit) as f32,
                    },
                });
            }
            Graphic::Circle {
                center,
                radius,
                width,
                fill,
            } => {
                let (wcx, wcy) = symbol_world_point(sym, center);
                let r = (*radius * xform.mm_to_unit) as f32;
                out.push(circle_path(
                    xform.x(wcx),
                    xform.px_y(wcy),
                    r,
                    SvgStyle {
                        stroke_rgb: Some((0.22, 0.22, 0.22)),
                        fill_rgb: fill_to_rgb(*fill, (0.22, 0.22, 0.22)),
                        stroke_width: ((*width).max(0.15) * xform.mm_to_unit) as f32,
                    },
                ));
            }
            Graphic::Arc {
                start,
                mid,
                end,
                width,
                fill,
            } => {
                let (sx, sy) = symbol_world_point(sym, start);
                let (mx, my) = symbol_world_point(sym, mid);
                let (ex, ey) = symbol_world_point(sym, end);
                let mut cmds = arc_path_commands(
                    pt(xform.x(sx), xform.px_y(sy)),
                    pt(xform.x(mx), xform.px_y(my)),
                    pt(xform.x(ex), xform.px_y(ey)),
                );
                if matches!(fill, FillType::Outline | FillType::Background) {
                    cmds.push(SvgPathCommand::Close);
                }
                out.push(SvgElement::Path {
                    commands: cmds,
                    style: SvgStyle {
                        stroke_rgb: Some((0.22, 0.22, 0.22)),
                        fill_rgb: fill_to_rgb(*fill, (0.22, 0.22, 0.22)),
                        stroke_width: ((*width).max(0.15) * xform.mm_to_unit) as f32,
                    },
                });
            }
            Graphic::Bezier {
                points,
                width,
                fill,
            } => {
                if points.len() != 4 {
                    continue;
                }
                let (p0x, p0y) = symbol_world_point(sym, &points[0]);
                let (c1x, c1y) = symbol_world_point(sym, &points[1]);
                let (c2x, c2y) = symbol_world_point(sym, &points[2]);
                let (p3x, p3y) = symbol_world_point(sym, &points[3]);
                out.push(SvgElement::Path {
                    commands: vec![
                        SvgPathCommand::MoveTo(pt(xform.x(p0x), xform.px_y(p0y))),
                        SvgPathCommand::CubicTo(
                            pt(xform.x(c1x), xform.px_y(c1y)),
                            pt(xform.x(c2x), xform.px_y(c2y)),
                            pt(xform.x(p3x), xform.px_y(p3y)),
                        ),
                    ],
                    style: SvgStyle {
                        stroke_rgb: Some((0.22, 0.22, 0.22)),
                        fill_rgb: fill_to_rgb(*fill, (0.22, 0.22, 0.22)),
                        stroke_width: ((*width).max(0.15) * xform.mm_to_unit) as f32,
                    },
                });
            }
            Graphic::Text {
                text,
                position,
                rotation,
                font_size,
                ..
            } => {
                let (wx, wy) = symbol_world_point(sym, position);
                out.push(SvgElement::Text {
                    x: xform.x(wx),
                    y: xform.px_y(wy),
                    font_alias: "F1",
                    size_pt: if *font_size > 0.0 {
                        (*font_size * xform.mm_to_unit) as f32
                    } else {
                        8.0
                    },
                    align: SvgTextAlign::Left,
                    v_align: SvgTextVAlign::Top,
                    rotation_deg: *rotation as f32,
                    fill_rgb: (0.15, 0.15, 0.15),
                    text: text.clone(),
                });
            }
            Graphic::TextBox {
                text,
                position,
                size,
                font_size,
                width,
                fill,
                ..
            } => {
                let (wx0, wy0) = symbol_world_point(sym, position);
                let (wx1, wy1) = symbol_world_point(sym, &Point::new(position.x + size.x, position.y + size.y));
                let x1 = xform.x(wx0).min(xform.x(wx1));
                let y1 = xform.px_y(wy0).min(xform.px_y(wy1));
                let x2 = xform.x(wx0).max(xform.x(wx1));
                let y2 = xform.px_y(wy0).max(xform.px_y(wy1));
                out.push(rect_path(
                    x1,
                    y1,
                    x2 - x1,
                    y2 - y1,
                    SvgStyle {
                        stroke_rgb: Some((0.22, 0.22, 0.22)),
                        fill_rgb: fill_to_rgb(*fill, (0.22, 0.22, 0.22)),
                        stroke_width: ((*width).max(0.15) * xform.mm_to_unit) as f32,
                    },
                ));
                out.push(SvgElement::Text {
                    x: x1 + 2.0,
                    y: y1 + 2.0,
                    font_alias: "F1",
                    size_pt: if *font_size > 0.0 {
                        (*font_size * xform.mm_to_unit) as f32
                    } else {
                        8.0
                    },
                    align: SvgTextAlign::Left,
                    v_align: SvgTextVAlign::Top,
                    rotation_deg: 0.0,
                    fill_rgb: (0.15, 0.15, 0.15),
                    text: text.clone(),
                });
            }
        }
    }
}

fn push_symbol_pins(
    out: &mut Vec<SvgElement>,
    sym: &Symbol,
    lib: &LibSymbol,
    xform: &PageTransform,
) {
    for lp in &lib.pins {
        if lp.unit != 0 && lp.unit != sym.unit {
            continue;
        }
        if lp.body_style != 0 && lp.body_style != 1 {
            continue;
        }

        let pin = &lp.pin;
        if !pin.visible {
            continue;
        }

        let (dir_x, dir_y) = pin_direction(pin);
        let length = if pin.length > 0.0 {
            pin.length
        } else {
            signex_types::schematic::PIN_LENGTH_MM
        };

        let body_end = Point::new(
            pin.position.x + dir_x * length,
            pin.position.y + dir_y * length,
        );

        let (wx1, wy1) = symbol_world_point(sym, &pin.position);
        let (wx2, wy2) = symbol_world_point(sym, &body_end);

        out.push(SvgElement::Path {
            commands: vec![
                SvgPathCommand::MoveTo(pt(xform.x(wx1), xform.px_y(wy1))),
                SvgPathCommand::LineTo(pt(xform.x(wx2), xform.px_y(wy2))),
            ],
            style: SvgStyle {
                stroke_rgb: Some((0.1, 0.1, 0.1)),
                fill_rgb: None,
                stroke_width: (0.15 * xform.mm_to_unit) as f32,
            },
        });

        // Direction vector in world/schematic space (for text placement).
        let (wdx, wdy) = {
            let (p0x, p0y) = symbol_world_point(sym, &pin.position);
            let (p1x, p1y) = symbol_world_point(
                sym,
                &Point::new(pin.position.x + dir_x, pin.position.y + dir_y),
            );
            let dx = p1x - p0x;
            let dy = p1y - p0y;
            let len = (dx * dx + dy * dy).sqrt();
            if len > 0.0 {
                (dx / len, dy / len)
            } else {
                (1.0, 0.0)
            }
        };

        if lib.show_pin_names && pin.name_visible && !pin.name.is_empty() && pin.name != "~" {
            let name_pos = if lib.pin_name_offset.abs() < 0.01 {
                // KiCad offset=0 mode: put name perpendicular to tip.
                let perp = (-wdy, wdx);
                (wx1 + perp.0 * 0.8, wy1 + perp.1 * 0.8)
            } else {
                // Along pin into body.
                (
                    wx2 + wdx * lib.pin_name_offset,
                    wy2 + wdy * lib.pin_name_offset,
                )
            };

            out.push(SvgElement::Text {
                x: xform.x(name_pos.0),
                y: xform.px_y(name_pos.1),
                font_alias: "F1",
                size_pt: (signex_types::schematic::SCHEMATIC_TEXT_MM * xform.mm_to_unit) as f32,
                align: if wdx >= 0.0 {
                    SvgTextAlign::Left
                } else {
                    SvgTextAlign::Right
                },
                v_align: SvgTextVAlign::Center,
                rotation_deg: if wdx.abs() < wdy.abs() {
                    if wdy < 0.0 { 90.0 } else { -90.0 }
                } else {
                    0.0
                },
                fill_rgb: (0.12, 0.12, 0.12),
                text: pin.name.clone(),
            });
        }

        if lib.show_pin_numbers && pin.number_visible && !pin.number.is_empty() {
            // Number just inside body end.
            let num_pos = (wx2 - wdx * 0.6, wy2 - wdy * 0.6);
            out.push(SvgElement::Text {
                x: xform.x(num_pos.0),
                y: xform.px_y(num_pos.1),
                font_alias: "F3",
                size_pt: (1.0 * xform.mm_to_unit) as f32,
                align: SvgTextAlign::Center,
                v_align: SvgTextVAlign::Center,
                rotation_deg: 0.0,
                fill_rgb: (0.1, 0.1, 0.1),
                text: pin.number.clone(),
            });
        }
    }
}

fn pin_direction(pin: &Pin) -> (f64, f64) {
    let deg = ((pin.rotation % 360.0) + 360.0) % 360.0;
    match deg as i32 {
        0 => (1.0, 0.0),
        90 => (0.0, 1.0),
        180 => (-1.0, 0.0),
        270 => (0.0, -1.0),
        _ => {
            let rad = deg.to_radians();
            (rad.cos(), rad.sin())
        }
    }
}

fn symbol_world_point(sym: &Symbol, local: &Point) -> (f64, f64) {
    let x = local.x;
    let y = -local.y;
    let rad = -sym.rotation.to_radians();
    let cos = rad.cos();
    let sin = rad.sin();
    let rx = x * cos - y * sin;
    let ry = x * sin + y * cos;
    let rx = if sym.mirror_y { -rx } else { rx };
    let ry = if sym.mirror_x { -ry } else { ry };
    (rx + sym.position.x, ry + sym.position.y)
}

fn path_to_tiny_skia(commands: &[SvgPathCommand]) -> Option<tiny_skia::Path> {
    let mut pb = PathBuilder::new();
    for c in commands {
        match c {
            SvgPathCommand::MoveTo(p) => pb.move_to(p.x, p.y),
            SvgPathCommand::LineTo(p) => pb.line_to(p.x, p.y),
            SvgPathCommand::CubicTo(c1, c2, p) => pb.cubic_to(c1.x, c1.y, c2.x, c2.y, p.x, p.y),
            SvgPathCommand::Close => pb.close(),
        }
    }
    pb.finish()
}

fn rect_path(x: f32, y: f32, w: f32, h: f32, style: SvgStyle) -> SvgElement {
    SvgElement::Path {
        commands: vec![
            SvgPathCommand::MoveTo(pt(x, y)),
            SvgPathCommand::LineTo(pt(x + w, y)),
            SvgPathCommand::LineTo(pt(x + w, y + h)),
            SvgPathCommand::LineTo(pt(x, y + h)),
            SvgPathCommand::Close,
        ],
        style,
    }
}

fn circle_path(cx: f32, cy: f32, r: f32, style: SvgStyle) -> SvgElement {
    let k = 0.552_284_8_f32 * r;
    SvgElement::Path {
        commands: vec![
            SvgPathCommand::MoveTo(pt(cx + r, cy)),
            SvgPathCommand::CubicTo(pt(cx + r, cy + k), pt(cx + k, cy + r), pt(cx, cy + r)),
            SvgPathCommand::CubicTo(pt(cx - k, cy + r), pt(cx - r, cy + k), pt(cx - r, cy)),
            SvgPathCommand::CubicTo(pt(cx - r, cy - k), pt(cx - k, cy - r), pt(cx, cy - r)),
            SvgPathCommand::CubicTo(pt(cx + k, cy - r), pt(cx + r, cy - k), pt(cx + r, cy)),
            SvgPathCommand::Close,
        ],
        style,
    }
}

fn arc_path_commands(start: SvgPoint, mid: SvgPoint, end: SvgPoint) -> Vec<SvgPathCommand> {
    if let Some((cx, cy, r)) = circle_from_three_points(start, mid, end) {
        let start_a = (start.y - cy).atan2(start.x - cx) as f64;
        let mid_a = (mid.y - cy).atan2(mid.x - cx) as f64;
        let end_a = (end.y - cy).atan2(end.x - cx) as f64;
        let (from, to) = arc_sweep(start_a, mid_a, end_a);
        let sweep = to - from;
        let seg_count = ((sweep.abs() / (std::f64::consts::FRAC_PI_2)).ceil() as usize).max(1);
        let step = sweep / seg_count as f64;

        let mut cmds = Vec::with_capacity(seg_count + 1);
        cmds.push(SvgPathCommand::MoveTo(start));

        for i in 0..seg_count {
            let a0 = from + step * i as f64;
            let a1 = from + step * (i + 1) as f64;
            let k = (4.0 / 3.0) * ((a1 - a0) / 4.0).tan();

            let p0 = (cx as f64 + r as f64 * a0.cos(), cy as f64 + r as f64 * a0.sin());
            let p3 = (cx as f64 + r as f64 * a1.cos(), cy as f64 + r as f64 * a1.sin());
            let c1 = (
                p0.0 - k * r as f64 * a0.sin(),
                p0.1 + k * r as f64 * a0.cos(),
            );
            let c2 = (
                p3.0 + k * r as f64 * -a1.sin(),
                p3.1 + k * r as f64 * a1.cos(),
            );

            cmds.push(SvgPathCommand::CubicTo(
                pt(c1.0 as f32, c1.1 as f32),
                pt(c2.0 as f32, c2.1 as f32),
                pt(p3.0 as f32, p3.1 as f32),
            ));
        }

        cmds
    } else {
        vec![SvgPathCommand::MoveTo(start), SvgPathCommand::LineTo(end)]
    }
}

fn encode_svg_document(width: f32, height: f32, elements: &[SvgElement]) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\">"
    );

    for element in elements {
        match element {
            SvgElement::Path { commands, style } => {
                let d = to_svg_path_d(commands);
                let stroke = style
                    .stroke_rgb
                    .map(rgb_css)
                    .unwrap_or_else(|| "none".to_string());
                let fill = style
                    .fill_rgb
                    .map(rgb_css)
                    .unwrap_or_else(|| "none".to_string());
                let _ = writeln!(
                    out,
                    "  <path d=\"{}\" stroke=\"{}\" fill=\"{}\" stroke-width=\"{}\" stroke-linejoin=\"miter\" stroke-linecap=\"square\" />",
                    d, stroke, fill, style.stroke_width
                );
            }
            SvgElement::Text {
                x,
                y,
                size_pt,
                align,
                v_align,
                rotation_deg,
                fill_rgb,
                text,
                ..
            } => {
                let anchor = match align {
                    SvgTextAlign::Left => "start",
                    SvgTextAlign::Center => "middle",
                    SvgTextAlign::Right => "end",
                };
                let baseline = match v_align {
                    SvgTextVAlign::Top => "hanging",
                    SvgTextVAlign::Center => "middle",
                    SvgTextVAlign::Bottom => "alphabetic",
                };
                let transform = if rotation_deg.abs() > 0.001 {
                    format!(" transform=\"rotate({} {} {})\"", rotation_deg, x, y)
                } else {
                    String::new()
                };
                let _ = writeln!(
                    out,
                    "  <text x=\"{x}\" y=\"{y}\" text-anchor=\"{anchor}\" dominant-baseline=\"{baseline}\"{transform} fill=\"{}\" font-size=\"{size_pt}\">{}</text>",
                    rgb_css(*fill_rgb),
                    escape_xml(text)
                );
            }
        }
    }

    out.push_str("</svg>\n");
    out
}

fn to_svg_path_d(commands: &[SvgPathCommand]) -> String {
    let mut out = String::new();
    for cmd in commands {
        match cmd {
            SvgPathCommand::MoveTo(p) => {
                let _ = write!(out, "M {} {} ", p.x, p.y);
            }
            SvgPathCommand::LineTo(p) => {
                let _ = write!(out, "L {} {} ", p.x, p.y);
            }
            SvgPathCommand::CubicTo(c1, c2, p) => {
                let _ = write!(out, "C {} {}, {} {}, {} {} ", c1.x, c1.y, c2.x, c2.y, p.x, p.y);
            }
            SvgPathCommand::Close => {
                out.push_str("Z ");
            }
        }
    }
    out.trim().to_string()
}

fn fill_to_rgb(fill: FillType, stroke: (f32, f32, f32)) -> Option<(f32, f32, f32)> {
    match fill {
        FillType::None => None,
        FillType::Outline => Some(stroke),
        FillType::Background => Some((0.96, 0.96, 0.96)),
    }
}

fn label_size_pt(font_size_mm: f64, mm_to_unit: f64, scale: &PdfScale) -> f32 {
    if font_size_mm > 0.0 {
        (font_size_mm * mm_to_unit) as f32
    } else {
        match scale {
            PdfScale::OneToOne | PdfScale::FitToPage | PdfScale::Percent(_) => 9.0,
        }
    }
}

fn label_colour(label_type: LabelType) -> (f32, f32, f32) {
    match label_type {
        LabelType::Net => (0.08, 0.08, 0.08),
        LabelType::Global => (0.14, 0.24, 0.52),
        LabelType::Hierarchical => (0.28, 0.2, 0.06),
        LabelType::Power => (0.42, 0.09, 0.09),
    }
}

fn rgb_css((r, g, b): (f32, f32, f32)) -> String {
    let to_u8 = |v: f32| -> u8 { (v.clamp(0.0, 1.0) * 255.0).round() as u8 };
    format!("rgb({},{},{})", to_u8(r), to_u8(g), to_u8(b))
}

fn rgb_to_color(r: f32, g: f32, b: f32) -> Color {
    Color::from_rgba(r.clamp(0.0, 1.0), g.clamp(0.0, 1.0), b.clamp(0.0, 1.0), 1.0)
        .unwrap_or(Color::BLACK)
}

fn map_colour_mode(rgb: (f32, f32, f32), mode: ColourMode) -> (f32, f32, f32) {
    let (r, g, b) = rgb;
    match mode {
        ColourMode::Colour => (r, g, b),
        ColourMode::Grayscale => {
            let y = 0.299 * r + 0.587 * g + 0.114 * b;
            (y, y, y)
        }
        ColourMode::BlackAndWhite => {
            let y = 0.299 * r + 0.587 * g + 0.114 * b;
            if y >= 0.5 {
                (1.0, 1.0, 1.0)
            } else {
                (0.0, 0.0, 0.0)
            }
        }
    }
}

fn draw_text_outline(
    pixmap: &mut Pixmap,
    x: f32,
    y: f32,
    size_pt: f32,
    align: SvgTextAlign,
    v_align: SvgTextVAlign,
    rotation_deg: f32,
    fill_rgb: (f32, f32, f32),
    font_alias: &str,
    text: &str,
) {
    let Some(face) = face_for_alias(font_alias) else {
        return;
    };
    let units_per_em = face.units_per_em() as f32;
    if units_per_em <= 0.0 {
        return;
    }
    let scale = size_pt.max(1.0) / units_per_em;
    let advance = measure_text_advance(&face, text, scale);

    let start_x = match align {
        SvgTextAlign::Left => x,
        SvgTextAlign::Center => x - advance * 0.5,
        SvgTextAlign::Right => x - advance,
    };

    let asc = face.ascender() as f32 * scale;
    let desc = face.descender() as f32 * scale;
    let baseline_y = match v_align {
        SvgTextVAlign::Top => y + asc,
        SvgTextVAlign::Center => y + (asc + desc) * 0.5,
        SvgTextVAlign::Bottom => y + desc,
    };

    let mut pen_x = start_x;
    let mut paint = Paint::default();
    paint.set_color(rgb_to_color(fill_rgb.0, fill_rgb.1, fill_rgb.2));

    for ch in text.chars() {
        if ch == '\n' || ch == '\r' {
            continue;
        }

        if let Some(gid) = face.glyph_index(ch) {
            let mut builder = TinyPathOutlineBuilder::new(
                pen_x,
                baseline_y,
                scale,
                x,
                y,
                rotation_deg,
            );
            if face.outline_glyph(gid, &mut builder).is_some()
                && let Some(path) = builder.finish()
            {
                pixmap.fill_path(&path, &paint, FillRule::Winding, Default::default(), None);
            }
            pen_x += glyph_advance(&face, gid, scale);
        } else {
            pen_x += size_pt * 0.5;
        }
    }
}

fn measure_text_advance(face: &Face<'_>, text: &str, scale: f32) -> f32 {
    let mut advance = 0.0_f32;
    for ch in text.chars() {
        if ch == '\n' || ch == '\r' {
            continue;
        }
        if let Some(gid) = face.glyph_index(ch) {
            advance += glyph_advance(face, gid, scale);
        } else {
            advance += 0.5 * scale * face.units_per_em() as f32;
        }
    }
    advance
}

fn glyph_advance(face: &Face<'_>, gid: GlyphId, scale: f32) -> f32 {
    face.glyph_hor_advance(gid)
        .map(|v| v as f32 * scale)
        .unwrap_or(0.5 * face.units_per_em() as f32 * scale)
}

fn face_for_alias(alias: &str) -> Option<Face<'static>> {
    match alias {
        "F1" => Face::parse(
            include_bytes!("../../../signex-app/assets/fonts/Roboto-Regular.ttf"),
            0,
        )
        .ok(),
        "F2" => Face::parse(
            include_bytes!("../../../signex-app/assets/fonts/Roboto-Bold.ttf"),
            0,
        )
        .ok(),
        "F3" => Face::parse(
            include_bytes!("../../../signex-app/assets/fonts/Iosevka-Regular.ttf"),
            0,
        )
        .ok(),
        "F4" => Face::parse(
            include_bytes!("../../../signex-app/assets/fonts/Iosevka-Bold.ttf"),
            0,
        )
        .ok(),
        _ => None,
    }
}

struct TinyPathOutlineBuilder {
    pb: PathBuilder,
    pen_x: f32,
    baseline_y: f32,
    scale: f32,
    anchor_x: f32,
    anchor_y: f32,
    rot_cos: f32,
    rot_sin: f32,
}

impl TinyPathOutlineBuilder {
    fn new(
        pen_x: f32,
        baseline_y: f32,
        scale: f32,
        anchor_x: f32,
        anchor_y: f32,
        rotation_deg: f32,
    ) -> Self {
        let rad = rotation_deg.to_radians();
        Self {
            pb: PathBuilder::new(),
            pen_x,
            baseline_y,
            scale,
            anchor_x,
            anchor_y,
            rot_cos: rad.cos(),
            rot_sin: rad.sin(),
        }
    }

    fn finish(self) -> Option<tiny_skia::Path> {
        self.pb.finish()
    }

    fn map_point(&self, x: f32, y: f32) -> (f32, f32) {
        let px = self.pen_x + x * self.scale;
        let py = self.baseline_y - y * self.scale;
        let dx = px - self.anchor_x;
        let dy = py - self.anchor_y;
        let rx = self.anchor_x + dx * self.rot_cos - dy * self.rot_sin;
        let ry = self.anchor_y + dx * self.rot_sin + dy * self.rot_cos;
        (rx, ry)
    }
}

fn halign_to_svg(h: HAlign) -> SvgTextAlign {
    match h {
        HAlign::Left => SvgTextAlign::Left,
        HAlign::Center => SvgTextAlign::Center,
        HAlign::Right => SvgTextAlign::Right,
    }
}

fn valign_to_svg(v: VAlign) -> SvgTextVAlign {
    match v {
        VAlign::Top => SvgTextVAlign::Top,
        VAlign::Center => SvgTextVAlign::Center,
        VAlign::Bottom => SvgTextVAlign::Bottom,
    }
}

fn field_effective_style(prop: &TextProp, sym: &Symbol) -> (f64, HAlign, VAlign) {
    let total = (sym.rotation + prop.rotation).rem_euclid(360.0);
    let (draw_rot, fold_h, fold_v) = match total.round() as i32 {
        0 => (0.0, false, false),
        90 => (90.0, false, false),
        180 => (0.0, true, false),
        270 => (90.0, false, true),
        _ => (total, false, false),
    };

    let flip_h = fold_h ^ sym.mirror_y;
    let flip_v = fold_v ^ sym.mirror_x;

    let h = if flip_h {
        match prop.justify_h {
            HAlign::Left => HAlign::Right,
            HAlign::Center => HAlign::Center,
            HAlign::Right => HAlign::Left,
        }
    } else {
        prop.justify_h
    };

    let v = if flip_v {
        match prop.justify_v {
            VAlign::Top => VAlign::Bottom,
            VAlign::Center => VAlign::Center,
            VAlign::Bottom => VAlign::Top,
        }
    } else {
        prop.justify_v
    };

    (draw_rot, h, v)
}

impl OutlineBuilder for TinyPathOutlineBuilder {
    fn move_to(&mut self, x: f32, y: f32) {
        let (mx, my) = self.map_point(x, y);
        self.pb.move_to(mx, my);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let (lx, ly) = self.map_point(x, y);
        self.pb.line_to(lx, ly);
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let (cx, cy) = self.map_point(x1, y1);
        let (px, py) = self.map_point(x, y);
        self.pb.quad_to(cx, cy, px, py);
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let (c1x, c1y) = self.map_point(x1, y1);
        let (c2x, c2y) = self.map_point(x2, y2);
        let (px, py) = self.map_point(x, y);
        self.pb.cubic_to(c1x, c1y, c2x, c2y, px, py);
    }

    fn close(&mut self) {
        self.pb.close();
    }
}

fn escape_xml(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn pt(x: f32, y: f32) -> SvgPoint {
    SvgPoint { x, y }
}

fn circle_from_three_points(a: SvgPoint, b: SvgPoint, c: SvgPoint) -> Option<(f32, f32, f32)> {
    let (ax, ay) = (a.x as f64, a.y as f64);
    let (bx, by) = (b.x as f64, b.y as f64);
    let (cx, cy) = (c.x as f64, c.y as f64);

    let d = 2.0 * (ax * (by - cy) + bx * (cy - ay) + cx * (ay - by));
    if d.abs() < 1e-12 {
        return None;
    }

    let ux = ((ax * ax + ay * ay) * (by - cy)
        + (bx * bx + by * by) * (cy - ay)
        + (cx * cx + cy * cy) * (ay - by))
        / d;
    let uy = ((ax * ax + ay * ay) * (cx - bx)
        + (bx * bx + by * by) * (ax - cx)
        + (cx * cx + cy * cy) * (bx - ax))
        / d;
    let r = ((ax - ux) * (ax - ux) + (ay - uy) * (ay - uy)).sqrt();

    Some((ux as f32, uy as f32, r as f32))
}

fn arc_sweep(start: f64, mid: f64, end: f64) -> (f64, f64) {
    use std::f64::consts::TAU;

    let norm = |a: f64| -> f64 {
        let mut t = a % TAU;
        if t < 0.0 {
            t += TAU;
        }
        t
    };

    let ccw_dist = |a: f64, b: f64| -> f64 {
        let d = b - a;
        if d < 0.0 { d + TAU } else { d }
    };

    let s = norm(start);
    let m = norm(mid);
    let e = norm(end);

    let s_to_m = ccw_dist(s, m);
    let s_to_e = ccw_dist(s, e);
    if s_to_m <= s_to_e {
        (start, start + s_to_e)
    } else {
        (start, start - (TAU - s_to_e))
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use signex_types::schematic::{SchematicSheet, Wire};
    use uuid::Uuid;

    use super::*;

    fn empty_sheet_snapshot() -> SheetSnapshot {
        SheetSnapshot {
            path: PathBuf::from("test.kicad_sch"),
            schematic: SchematicSheet {
                uuid: Uuid::nil(),
                version: 0,
                generator: String::new(),
                generator_version: String::new(),
                paper_size: "A4".to_string(),
                root_sheet_page: "1".to_string(),
                symbols: vec![],
                wires: vec![],
                junctions: vec![],
                labels: vec![],
                child_sheets: vec![],
                no_connects: vec![],
                text_notes: vec![],
                buses: vec![],
                bus_entries: vec![],
                drawings: vec![],
                no_erc_directives: vec![],
                title_block: Default::default(),
                lib_symbols: Default::default(),
            },
            sheet_name: "Sheet1".to_string(),
            sheet_number: 1,
            sheet_count: 1,
        }
    }

    #[test]
    fn svg_document_is_emitted() {
        let mut sheet = empty_sheet_snapshot();
        sheet.schematic.wires.push(Wire {
            uuid: Uuid::new_v4(),
            start: Point::new(0.0, 0.0),
            end: Point::new(25.0, 20.0),
            stroke_width: 0.15,
        });
        let opts = PdfOptions::default();
        let (w_mm, h_mm) = opts.page_size.dimensions_mm(opts.orientation);
        let svg = SvgRenderContext::from_sheet(&sheet, &opts, w_mm, h_mm, 72.0 / 25.4);
        assert!(svg.svg_document.starts_with("<svg"));
        assert!(svg.svg_document.contains("<path"));
        assert!(svg.svg_document.contains("</svg>"));
    }

    #[test]
    fn can_rasterize_context() {
        let sheet = empty_sheet_snapshot();
        let opts = PdfOptions::default();
        let (w_mm, h_mm) = opts.page_size.dimensions_mm(opts.orientation);
        let svg = SvgRenderContext::from_sheet(&sheet, &opts, w_mm, h_mm, 96.0 / 25.4);
        let rgba = svg.rasterize_rgba((w_mm * 96.0 / 25.4) as u32, (h_mm * 96.0 / 25.4) as u32);
        assert!(rgba.is_some());
    }
}
