//! Symbol emitters — library body graphics and pins.
//!
//! Body graphics and pin stubs come from the shared `lower_placed_symbol`
//! lowering, the same list the canvas draws. Pin numbers and names come from
//! the shared `lower_pin_labels`, with the expression text evaluated here.

use super::geometry::{arc_path_commands, circle_path};
use super::labels::{halign_to_svg, valign_to_svg};
use super::text::glyph_em_pt;
use super::*;
use crate::pdf::layout::PageTransform;
use crate::pdf::palette::SchematicPalette;
use signex_types::markup::ExpressionEvalContext;
use signex_types::schematic::{
    FillType, LibSymbol, PinLabelKind, Point, SchematicDefaults, Symbol, SymbolPrimitive,
    lower_pin_labels, lower_placed_symbol,
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
    defaults: &SchematicDefaults,
) {
    let px = |p: &Point| pt(xform.x(p.x), xform.px_y(p.y));
    let style = |width: f64, fill: FillType| SvgStyle {
        stroke_rgb: Some(palette.symbol_stroke),
        fill_rgb: fill_to_rgb(fill, palette.symbol_stroke, palette.symbol_fill),
        stroke_width: (defaults.symbol_line_mm(width) * xform.mm_to_unit) as f32,
    };
    for primitive in lower_placed_symbol(lib, sym, defaults) {
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
            SymbolPrimitive::Pin { tip, body, width } => out.push(SvgElement::Path {
                commands: vec![
                    SvgPathCommand::MoveTo(px(&tip)),
                    SvgPathCommand::LineTo(px(&body)),
                ],
                style: SvgStyle {
                    stroke_rgb: Some(palette.pin),
                    fill_rgb: None,
                    stroke_width: (defaults.symbol_line_mm(width) * xform.mm_to_unit) as f32,
                },
            }),
        }
    }
}

/// Pin numbers and names, placed by the shared `lower_pin_labels` the canvas
/// also draws.
pub(super) fn push_symbol_pins(
    out: &mut Vec<SvgElement>,
    sym: &Symbol,
    lib: &LibSymbol,
    xform: &PageTransform,
    eval_ctx: &ExpressionEvalContext<'_>,
    pin_net_names: Option<&HashMap<String, String>>,
    palette: &SchematicPalette,
) {
    for label in lower_pin_labels(lib, sym) {
        let mut pin_eval_ctx = eval_ctx.clone();
        pin_eval_ctx.current_pin = Some(label.pin_number.as_str());
        pin_eval_ctx.net_name_by_pin = pin_net_names;
        let (font_alias, fill_rgb) = match label.kind {
            PinLabelKind::Name => ("F1", palette.pin),
            PinLabelKind::Number => ("F3", palette.field_text),
        };
        out.push(SvgElement::Text {
            x: xform.x(label.position.x),
            y: xform.px_y(label.position.y),
            font_alias,
            size_pt: glyph_em_pt(
                font_alias,
                signex_types::schematic::SCHEMATIC_TEXT_MM,
                xform.mm_to_unit,
            ),
            align: halign_to_svg(label.h_align),
            v_align: valign_to_svg(label.v_align),
            rotation_deg: label.rotation_rad.to_degrees() as f32,
            fill_rgb,
            text: normalize_standard_text_with_ctx(&label.text, &pin_eval_ctx),
        });
    }
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
