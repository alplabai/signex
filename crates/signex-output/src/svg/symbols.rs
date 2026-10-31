//! Symbol emitters — library body graphics and pins.
//!
//! Body graphics and pin stubs come from the shared `lower_placed_symbol`
//! lowering, the same list the canvas draws. Pin names and numbers are
//! SVG-only for now and are placed here.

use super::geometry::{arc_path_commands, circle_path};
use super::labels::{halign_to_svg, valign_to_svg};
use super::text::glyph_em_pt;
use super::*;
use crate::pdf::layout::PageTransform;
use crate::pdf::palette::SchematicPalette;
use signex_types::markup::ExpressionEvalContext;
use signex_types::schematic::{
    FillType, LibSymbol, Pin, Point, Symbol, SymbolPrimitive, SymbolTransform, instance_pins,
    lower_placed_symbol,
};
use std::collections::HashMap;

/// Emit one `SvgElement` per primitive of the symbol's shared lowering
/// (`lower_placed_symbol`): body graphics and pin stubs. The canvas draws the same
/// list, so the two cannot disagree on angle, mirror, unit or visibility.
pub(super) fn push_symbol_lib_graphics(
    out: &mut Vec<SvgElement>,
    sym: &Symbol,
    lib: Option<&LibSymbol>,
    xform: &PageTransform,
    palette: &SchematicPalette,
) {
    let px = |p: &Point| pt(xform.x(p.x), xform.px_y(p.y));
    let style = |width: f64, fill: FillType| SvgStyle {
        stroke_rgb: Some(palette.symbol_stroke),
        fill_rgb: fill_to_rgb(fill, palette.symbol_stroke, palette.symbol_fill),
        stroke_width: (width.max(0.15) * xform.mm_to_unit) as f32,
    };
    for primitive in lower_placed_symbol(lib, sym) {
        match primitive {
            SymbolPrimitive::Path {
                points,
                closed,
                width,
                fill,
            } => {
                let mut cmds = Vec::with_capacity(points.len() + 1);
                for (i, p) in points.iter().enumerate() {
                    cmds.push(if i == 0 {
                        SvgPathCommand::MoveTo(px(p))
                    } else {
                        SvgPathCommand::LineTo(px(p))
                    });
                }
                if closed {
                    cmds.push(SvgPathCommand::Close);
                }
                out.push(SvgElement::Path {
                    commands: cmds,
                    style: style(width, fill),
                });
            }
            SymbolPrimitive::Circle {
                center,
                radius,
                width,
                fill,
            } => out.push(circle_path(
                xform.x(center.x),
                xform.px_y(center.y),
                (radius * xform.mm_to_unit) as f32,
                style(width, fill),
            )),
            SymbolPrimitive::Arc {
                start,
                mid,
                end,
                width,
                fill,
            } => {
                let mut cmds = arc_path_commands(px(&start), px(&mid), px(&end));
                if matches!(fill, FillType::Outline | FillType::Background) {
                    cmds.push(SvgPathCommand::Close);
                }
                out.push(SvgElement::Path {
                    commands: cmds,
                    style: style(width, fill),
                });
            }
            SymbolPrimitive::Text {
                text,
                position,
                rotation_deg,
                size,
                h_align,
                v_align,
            } => out.push(SvgElement::Text {
                x: xform.x(position.x),
                y: xform.px_y(position.y),
                font_alias: "F1",
                size_pt: glyph_em_pt("F1", size, xform.mm_to_unit),
                align: halign_to_svg(h_align),
                v_align: valign_to_svg(v_align),
                rotation_deg: rotation_deg as f32,
                fill_rgb: (0.15, 0.15, 0.15),
                text: normalize_standard_text(&text),
            }),
            SymbolPrimitive::Pin { tip, body } => out.push(SvgElement::Path {
                commands: vec![
                    SvgPathCommand::MoveTo(px(&tip)),
                    SvgPathCommand::LineTo(px(&body)),
                ],
                style: SvgStyle {
                    stroke_rgb: Some(palette.pin),
                    fill_rgb: None,
                    stroke_width: (0.15 * xform.mm_to_unit) as f32,
                },
            }),
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
    for lp in instance_pins(lib, sym) {
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

        let (wx2, wy2) = symbol_world_point(sym, &body_end);

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

            // The name sits inside the body, `offset` past the pin's body
            // end, reading away from the edge: left-aligned off a left edge,
            // right-aligned off a right edge, hung below or standing above a
            // top or bottom edge. A library that stores no offset gets the
            // default, so a name never straddles the pin's wire.
            let offset = lib.name_offset_mm();
            let name_pos = (wx2 + wdx * offset, wy2 + wdy * offset);
            let (align, v_align) = if wdx.abs() > wdy.abs() {
                (
                    if wdx > 0.0 {
                        SvgTextAlign::Left
                    } else {
                        SvgTextAlign::Right
                    },
                    SvgTextVAlign::Center,
                )
            } else {
                (
                    SvgTextAlign::Center,
                    if wdy > 0.0 {
                        SvgTextVAlign::Top
                    } else {
                        SvgTextVAlign::Bottom
                    },
                )
            };
            let rotation_deg = 0.0;

            out.push(SvgElement::Text {
                x: xform.x(name_pos.0),
                y: xform.px_y(name_pos.1),
                font_alias: "F1",
                size_pt: glyph_em_pt(
                    "F1",
                    signex_types::schematic::SCHEMATIC_TEXT_MM,
                    xform.mm_to_unit,
                ),
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
                size_pt: glyph_em_pt(
                    "F3",
                    signex_types::schematic::SCHEMATIC_TEXT_MM,
                    xform.mm_to_unit,
                ),
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
    signex_types::schematic::pin_direction(pin.rotation)
}

fn symbol_world_point(sym: &Symbol, local: &Point) -> (f64, f64) {
    let p = SymbolTransform::from_symbol(sym).apply(*local);
    (p.x, p.y)
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

#[cfg(test)]
#[path = "symbols_tests.rs"]
mod tests;
