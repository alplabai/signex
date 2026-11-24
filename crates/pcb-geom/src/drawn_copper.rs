//! Copper of no net (Caner, 2026-11-24, #2883): a board drawing on a copper
//! layer is copper along its stroke (a text frame's lines too), and a board
//! string on a copper layer is copper along the strokes of its glyphs. The
//! design rule check, the pour fill and the router read this one list, so
//! they agree with the conductor image the Gerber writes.

use signex_types::pcb::{BoardGraphic, BoardText, PCB_DEFAULT_TEXT_SIZE_MM, PcbBoard};
use signex_types::schematic::HAlign;

use crate::copper::{CopperItem, layer_mask};
use crate::graphic::{board_graphic_placement, board_text_placement};
use crate::graphic_index::copper_graphic_strokes;
use crate::index::ItemRef;
use crate::pick_filter::PickKind;
use crate::shape::Shape;
use crate::stroke_font::{text_stroke_width_mm, text_strokes, text_strokes_within};
use crate::text_frame::{text_frame_lines, text_frame_size_mm};
use crate::units::{GeomError, PointNm, mm_to_nm};

/// Every board drawing and string on a copper layer as copper of net 0,
/// one item per stroke piece, all pieces with the identity of their
/// drawing or string. A drawing or string whose geometry does not convert
/// gives no copper.
pub fn drawn_copper(board: &PcbBoard) -> Vec<CopperItem> {
    let mut out = Vec::new();
    for graphic in &board.graphics {
        let layers = layer_mask(std::iter::once(graphic.layer.as_str()));
        let Some(kind) = graphic.kind().filter(|_| layers != 0) else {
            continue;
        };
        let item = ItemRef::Graphic {
            graphic: graphic.uuid,
            kind: PickKind::of_graphic(kind),
        };
        push(&mut out, item, layers, graphic_copper(graphic));
    }
    for text in &board.texts {
        let layers = layer_mask(std::iter::once(text.layer.as_str()));
        if layers != 0 {
            let shapes = text_copper(text).unwrap_or_default();
            push(&mut out, ItemRef::Text(text.uuid), layers, shapes);
        }
    }
    out
}

fn push(out: &mut Vec<CopperItem>, item: ItemRef, layers: u32, shapes: Vec<Shape>) {
    out.extend(shapes.into_iter().map(|shape| CopperItem {
        item,
        net: 0,
        layers,
        shape,
        pad_center: None,
    }));
}

/// A drawing's stroke and, for a text frame, the strokes of its lines.
fn graphic_copper(graphic: &BoardGraphic) -> Vec<Shape> {
    let mut shapes = match board_graphic_placement(graphic) {
        Ok(Some(placed)) => copper_graphic_strokes(&placed),
        Ok(None) | Err(_) => Vec::new(),
    };
    let size = text_frame_size_mm(graphic);
    for line in text_frame_lines(graphic).unwrap_or_default() {
        let strokes = text_strokes_within(
            &line.text,
            &line.placement,
            size,
            HAlign::Center,
            line.width_mm,
        );
        if let Ok(strokes) = strokes {
            shapes.extend(pen_strokes(strokes, size).unwrap_or_default());
        }
    }
    shapes
}

/// A string's glyph strokes, placed as the canvas and the Gerber place it.
fn text_copper(text: &BoardText) -> Result<Vec<Shape>, GeomError> {
    let size = if text.font_size.is_finite() && text.font_size > 0.0 {
        text.font_size
    } else {
        PCB_DEFAULT_TEXT_SIZE_MM
    };
    let at = board_text_placement(text)?;
    pen_strokes(text_strokes(&text.text, &at, size, HAlign::Center)?, size)
}

/// Polylines written with the pen of text `size_mm` high, as capsules: one
/// per piece, a lone point a dot.
fn pen_strokes(strokes: Vec<Vec<PointNm>>, size_mm: f64) -> Result<Vec<Shape>, GeomError> {
    let radius = (mm_to_nm(text_stroke_width_mm(size_mm), "text.pen")? / 2).abs();
    let mut shapes = Vec::new();
    for points in strokes {
        match points.as_slice() {
            [] => {}
            [dot] => shapes.push(Shape::Capsule {
                a: *dot,
                b: *dot,
                radius,
            }),
            _ => shapes.extend(points.windows(2).map(|pair| Shape::Capsule {
                a: pair[0],
                b: pair[1],
                radius,
            })),
        }
    }
    Ok(shapes)
}

#[cfg(test)]
#[path = "drawn_copper_tests.rs"]
mod tests;
