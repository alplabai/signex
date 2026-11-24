//! A repeated sheet symbol on the canvas (#1279): two outlines offset down
//! and right behind it, fainter each step, and a ×N pill at its top-right
//! corner, so a repeated sheet reads as one at any zoom. Its designator is
//! drawn in the accent. The geometry is `signex_types::schematic::RepeatStack`,
//! shared with the PDF and SVG output.

use super::*;
use signex_types::schematic::{ChildSheet, RepeatStack};

/// The inks of a repeated sheet symbol.
pub(super) struct StackLook {
    /// The sheet symbol's outline colour; the layers fade it.
    pub outline: [f32; 4],
    /// The pill's fill (and the designator's colour).
    pub accent: [f32; 4],
    /// The pill's text.
    pub badge_ink: [f32; 4],
    pub stroke_mm: f32,
    pub family: Option<std::sync::Arc<str>>,
}

/// Push the outlines behind `sheet` and its ×N pill when its designator is
/// a repeat. Returns whether it was one, so the caller draws the designator
/// in the accent.
pub(super) fn push_repeat_stack(
    sheet: &ChildSheet,
    look: &StackLook,
    polygons: &mut Vec<PolygonInput>,
    texts: &mut Vec<TextInput>,
) -> bool {
    let Some(stack) = RepeatStack::of(sheet) else {
        return false;
    };
    let quad = |[x0, y0, x1, y1]: [f64; 4]| {
        let (x0, y0, x1, y1) = (x0 as f32, y0 as f32, x1 as f32, y1 as f32);
        vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]]
    };
    for layer in &stack.layers {
        let [r, g, b, a] = look.outline;
        polygons.push(PolygonInput {
            vertices: quad(layer.rect),
            fill_color: [0.0, 0.0, 0.0, 0.0],
            stroke_color: Some([r, g, b, a * layer.alpha]),
            stroke_width_mm: look.stroke_mm,
        });
    }
    polygons.push(PolygonInput {
        vertices: quad(stack.badge),
        fill_color: look.accent,
        stroke_color: None,
        stroke_width_mm: 0.0,
    });
    let [x0, y0, x1, y1] = stack.badge;
    texts.push(TextInput {
        content: stack.badge_text,
        position: [((x0 + x1) * 0.5) as f32, ((y0 + y1) * 0.5) as f32],
        size_mm: stack.badge_text_mm as f32,
        color: look.badge_ink,
        bold: true,
        italic: false,
        family: look.family.clone(),
        rotation_rad: 0.0,
        h_align: HAlign::Center,
        v_align: VAlign::Center,
    });
    true
}
