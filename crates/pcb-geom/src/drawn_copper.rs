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

/// Why a board drawing or string on copper gives no copper. The Gerber
/// skips and reports exactly these items, so it agrees with this list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawnCopperFault {
    /// A `graphic_type` this build does not place.
    UnknownKind,
    /// A drawing missing the points its kind needs.
    MissingPoints,
    /// A coordinate, size or angle the geometry cannot hold.
    Geometry(GeomError),
}

impl From<GeomError> for DrawnCopperFault {
    fn from(error: GeomError) -> Self {
        DrawnCopperFault::Geometry(error)
    }
}

/// Every board drawing and string on a copper layer as copper of net 0,
/// one item per stroke piece, all pieces with the identity of their
/// drawing or string. A drawing or string that [`board_graphic_copper`] or
/// [`board_text_copper`] refuses gives no copper at all.
pub fn drawn_copper(board: &PcbBoard) -> Vec<CopperItem> {
    let mut out = Vec::new();
    for graphic in &board.graphics {
        let layers = layer_mask(std::iter::once(graphic.layer.as_str()));
        if layers == 0 {
            continue;
        }
        let (Some(kind), Ok(shapes)) = (graphic.kind(), board_graphic_copper(graphic)) else {
            continue;
        };
        let item = ItemRef::Graphic {
            graphic: graphic.uuid,
            kind: PickKind::of_graphic(kind),
        };
        push(&mut out, item, layers, shapes);
    }
    for text in &board.texts {
        let layers = layer_mask(std::iter::once(text.layer.as_str()));
        if layers == 0 {
            continue;
        }
        if let Ok(shapes) = board_text_copper(text) {
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

/// A board drawing's copper, were it on a copper layer: its stroke and, for
/// a text frame, the strokes of its lines. All or nothing: a drawing any
/// part of which does not convert is refused whole, with why.
pub fn board_graphic_copper(graphic: &BoardGraphic) -> Result<Vec<Shape>, DrawnCopperFault> {
    if graphic.kind().is_none() {
        return Err(DrawnCopperFault::UnknownKind);
    }
    let placed = board_graphic_placement(graphic)?.ok_or(DrawnCopperFault::MissingPoints)?;
    let mut shapes = copper_graphic_strokes(&placed);
    let size = text_frame_size_mm(graphic);
    for line in text_frame_lines(graphic)? {
        let strokes = text_strokes_within(
            &line.text,
            &line.placement,
            size,
            HAlign::Center,
            line.width_mm,
        )?;
        shapes.extend(pen_strokes(strokes, size)?);
    }
    Ok(shapes)
}

/// A board string's glyph strokes, were it on a copper layer, placed as the
/// canvas and the Gerber place it.
pub fn board_text_copper(text: &BoardText) -> Result<Vec<Shape>, DrawnCopperFault> {
    let size = if text.font_size.is_finite() && text.font_size > 0.0 {
        text.font_size
    } else {
        PCB_DEFAULT_TEXT_SIZE_MM
    };
    let at = board_text_placement(text)?;
    let strokes = text_strokes(&text.text, &at, size, HAlign::Center)?;
    pen_strokes(strokes, size).map_err(DrawnCopperFault::from)
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
