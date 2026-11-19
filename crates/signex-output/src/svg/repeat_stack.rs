//! A repeated sheet symbol in SVG and PDF output (#1279): the two outlines
//! behind it and its ×N pill, from `signex_types::schematic::RepeatStack`,
//! the geometry the canvas draws too. The PDF writer reads the same element
//! list, so this is the one place both formats draw it.

use signex_types::schematic::{ChildSheet, RepeatStack};

use super::geometry::rect_path;
use super::{SvgElement, SvgStyle, SvgTextAlign, SvgTextVAlign};
use crate::pdf::layout::PageTransform;
use crate::pdf::palette::SchematicPalette;

/// Push the stack and the pill of `child` when its designator is a repeat;
/// `stroke_width` is the symbol outline's, in output units. Called before
/// the outline, so the stack prints behind it.
pub(super) fn push_repeat_stack(
    elements: &mut Vec<SvgElement>,
    child: &ChildSheet,
    xform: &PageTransform,
    palette: &SchematicPalette,
    stroke_width: f32,
) {
    let Some(stack) = RepeatStack::of(child) else {
        return;
    };
    let rect = |[x0, y0, x1, y1]: [f64; 4]| {
        let (x, y) = (xform.x(x0.min(x1)), xform.px_y(y0.min(y1)));
        let w = ((x1 - x0).abs() * xform.mm_to_unit) as f32;
        let h = ((y1 - y0).abs() * xform.mm_to_unit) as f32;
        (x, y, w, h)
    };
    for layer in &stack.layers {
        let (x, y, w, h) = rect(layer.rect);
        elements.push(rect_path(
            x,
            y,
            w,
            h,
            SvgStyle {
                stroke_alpha: layer.alpha,
                stroke_rgb: Some(palette.child_sheet_stroke),
                fill_rgb: None,
                stroke_width,
            },
        ));
    }
    let (x, y, w, h) = rect(stack.badge);
    elements.push(rect_path(
        x,
        y,
        w,
        h,
        SvgStyle {
            stroke_alpha: 1.0,
            stroke_rgb: None,
            fill_rgb: Some(palette.child_sheet_text),
            stroke_width: 0.0,
        },
    ));
    elements.push(SvgElement::Text {
        x: x + w * 0.5,
        y: y + h * 0.5,
        font_alias: "F1",
        size_pt: (stack.badge_text_mm * xform.mm_to_unit) as f32,
        align: SvgTextAlign::Center,
        v_align: SvgTextVAlign::Center,
        rotation_deg: 0.0,
        fill_rgb: palette.paper,
        text: stack.badge_text,
    });
}
