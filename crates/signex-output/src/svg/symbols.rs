//! Symbol emitters — library body graphics and pins.
//!
//! Emits each placed symbol's `LibSymbol` body graphics and its pins
//! (stubs, names, numbers), plus the world-space transform and field
//! style helpers those emitters rely on.
//!
//! Extracted verbatim from the SVG exporter (`svg/mod.rs`); pure code
//! motion, zero behaviour change.

use super::*;
use super::geometry::{arc_path_commands, circle_path, rect_path};
use crate::pdf::layout::PageTransform;
use crate::pdf::palette::SchematicPalette;
use signex_types::markup::ExpressionEvalContext;
use signex_types::schematic::{
    FillType, Graphic, HAlign, LibSymbol, Pin, Point, Symbol, TextProp, VAlign,
};
use std::collections::HashMap;

pub(super) fn push_symbol_lib_graphics(
    out: &mut Vec<SvgElement>,
    sym: &Symbol,
    lib: &LibSymbol,
    xform: &PageTransform,
    palette: &SchematicPalette,
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
                        stroke_rgb: Some(palette.symbol_stroke),
                        fill_rgb: fill_to_rgb(*fill, palette.symbol_stroke, palette.symbol_fill),
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
                        stroke_rgb: Some(palette.symbol_stroke),
                        fill_rgb: fill_to_rgb(*fill, palette.symbol_stroke, palette.symbol_fill),
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
                        stroke_rgb: Some(palette.symbol_stroke),
                        fill_rgb: fill_to_rgb(*fill, palette.symbol_stroke, palette.symbol_fill),
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
                        stroke_rgb: Some(palette.symbol_stroke),
                        fill_rgb: fill_to_rgb(*fill, palette.symbol_stroke, palette.symbol_fill),
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
                        stroke_rgb: Some(palette.symbol_stroke),
                        fill_rgb: fill_to_rgb(*fill, palette.symbol_stroke, palette.symbol_fill),
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
                    text: normalize_standard_text(text),
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
                let (wx1, wy1) =
                    symbol_world_point(sym, &Point::new(position.x + size.x, position.y + size.y));
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
                        stroke_rgb: Some(palette.symbol_stroke),
                        fill_rgb: fill_to_rgb(*fill, palette.symbol_stroke, palette.symbol_fill),
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
                    text: normalize_standard_text(text),
                });
            }
        }
    }
}

pub(super) fn push_symbol_pins(
    out: &mut Vec<SvgElement>,
    sym: &Symbol,
    lib: &LibSymbol,
    xform: &PageTransform,
    eval_ctx: &ExpressionEvalContext<'_>,
    pin_net_names: Option<&HashMap<String, String>>,
    palette: &SchematicPalette,
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
                stroke_rgb: Some(palette.pin),
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
            let mut pin_eval_ctx = eval_ctx.clone();
            pin_eval_ctx.current_pin = Some(pin.number.as_str());
            pin_eval_ctx.net_name_by_pin = pin_net_names;

            let (name_pos, align, v_align, rotation_deg) = if lib.pin_name_offset.abs() < 0.01 {
                let (nwx, nwy) = (wx1, wy1);
                if wdx.abs() > wdy.abs() {
                    (
                        (nwx, nwy + 0.508),
                        SvgTextAlign::Center,
                        SvgTextVAlign::Bottom,
                        0.0,
                    )
                } else {
                    (
                        (nwx + 0.508, nwy),
                        SvgTextAlign::Left,
                        SvgTextVAlign::Center,
                        0.0,
                    )
                }
            } else {
                let name_pos = (
                    wx2 + wdx * lib.pin_name_offset,
                    wy2 + wdy * lib.pin_name_offset,
                );
                if wdx.abs() > wdy.abs() {
                    (
                        name_pos,
                        if wdx > 0.0 {
                            SvgTextAlign::Left
                        } else {
                            SvgTextAlign::Right
                        },
                        SvgTextVAlign::Center,
                        0.0,
                    )
                } else {
                    (
                        name_pos,
                        SvgTextAlign::Center,
                        if wdy > 0.0 {
                            SvgTextVAlign::Top
                        } else {
                            SvgTextVAlign::Bottom
                        },
                        0.0,
                    )
                }
            };

            out.push(SvgElement::Text {
                x: xform.x(name_pos.0),
                y: xform.px_y(name_pos.1),
                font_alias: "F1",
                size_pt: (signex_types::schematic::SCHEMATIC_TEXT_MM * xform.mm_to_unit) as f32,
                align,
                v_align,
                rotation_deg,
                fill_rgb: palette.pin,
                text: normalize_standard_text_with_ctx(&pin.name, &pin_eval_ctx),
            });
        }

        if lib.show_pin_numbers && pin.number_visible && !pin.number.is_empty() {
            let mut pin_eval_ctx = eval_ctx.clone();
            pin_eval_ctx.current_pin = Some(pin.number.as_str());
            pin_eval_ctx.net_name_by_pin = pin_net_names;

            let mid = Point::new(
                pin.position.x + dir_x * length * 0.5,
                pin.position.y + dir_y * length * 0.5,
            );
            let (mwx, mwy) = symbol_world_point(sym, &mid);
            let (perp_x, perp_y, align) = if wdx.abs() >= wdy.abs() {
                (0.0, 0.8, SvgTextAlign::Center)
            } else {
                (-0.8, 0.0, SvgTextAlign::Right)
            };
            let num_pos = (mwx + perp_x, mwy + perp_y);
            out.push(SvgElement::Text {
                x: xform.x(num_pos.0),
                y: xform.px_y(num_pos.1),
                font_alias: "F3",
                size_pt: (signex_types::schematic::SCHEMATIC_TEXT_MM * xform.mm_to_unit) as f32,
                align,
                v_align: SvgTextVAlign::Center,
                rotation_deg: 0.0,
                fill_rgb: palette.field_text,
                text: normalize_standard_text_with_ctx(&pin.number, &pin_eval_ctx),
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

pub(super) fn symbol_eval_variables(sym: &Symbol) -> HashMap<String, String> {
    let mut vars = sym.fields.clone();
    for prop in &sym.custom_properties {
        if !prop.key.is_empty() {
            vars.insert(prop.key.clone(), prop.value.clone());
        }
    }
    vars.entry("refdes".to_string())
        .or_insert_with(|| sym.reference.clone());
    vars.entry("reference".to_string())
        .or_insert_with(|| sym.reference.clone());
    vars.entry("value".to_string())
        .or_insert_with(|| sym.value.clone());
    vars
}

pub(super) fn field_effective_style(prop: &TextProp, sym: &Symbol) -> (f64, HAlign, VAlign) {
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
