//! Board drawings placed with the Graphics tools (#2424): a line, an arc
//! about its centre, a full circle, a rectangle and a text frame, on any
//! layer but a keep-out (on copper, copper of no net: #2883). Pure: each
//! function builds what one
//! `BoardCommand::AddItems` adds as one undo step.

use signex_types::format::pcb_layer_token;
use signex_types::pcb::{
    BoardGraphic, PCB_DEFAULT_GRAPHIC_WIDTH_MM, PCB_DEFAULT_TEXT_SIZE_MM, PcbBoard, Point,
};
use signex_types::pcb_graphic::{BoardGraphicKind, takes_board_graphics};
use signex_types::pcb_layer::PcbLayer;
use uuid::Uuid;

use super::command::BoardCommand;
use super::items::BoardItem;

/// The layer and stroke width a drawing is placed with.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GraphicStyle {
    pub layer: PcbLayer,
    pub width_mm: f64,
}

impl GraphicStyle {
    /// `layer` with the PCB default line width.
    pub fn on(layer: PcbLayer) -> Self {
        Self {
            layer,
            width_mm: PCB_DEFAULT_GRAPHIC_WIDTH_MM,
        }
    }
}

/// A drawing of `kind` named `id` with `style` and no points yet; `None` on
/// a layer a drawing may not go on, or a width that is not a number at or
/// above zero.
fn blank(id: Uuid, kind: BoardGraphicKind, style: GraphicStyle) -> Option<BoardGraphic> {
    let width_ok = style.width_mm.is_finite() && style.width_mm >= 0.0;
    (takes_board_graphics(style.layer) && width_ok).then(|| BoardGraphic {
        uuid: id,
        graphic_type: kind.token().to_string(),
        layer: pcb_layer_token(style.layer),
        width: style.width_mm,
        start: None,
        end: None,
        center: None,
        radius: 0.0,
        points: Vec::new(),
        text: String::new(),
        font_size: 0.0,
    })
}

fn finite(points: &[Point]) -> bool {
    points.iter().all(|p| p.x.is_finite() && p.y.is_finite())
}

fn distance(a: Point, b: Point) -> f64 {
    (b.x - a.x).hypot(b.y - a.y)
}

/// A line from `a` to `b`; `None` for two equal points.
pub fn new_line(id: Uuid, a: Point, b: Point, style: GraphicStyle) -> Option<BoardGraphic> {
    if a == b || !finite(&[a, b]) {
        return None;
    }
    let mut line = blank(id, BoardGraphicKind::Line, style)?;
    line.start = Some(a);
    line.end = Some(b);
    Some(line)
}

/// An arc about `center` from `start`, whose distance is the radius, to
/// the angle of `end`, the way of increasing angle in the model's axes
/// (clockwise on screen, where y grows down). `end` is moved onto the
/// circle; on the start's own angle the arc is a full turn. `None` when
/// `start` or `end` is the centre.
pub fn new_arc(
    id: Uuid,
    center: Point,
    start: Point,
    end: Point,
    style: GraphicStyle,
) -> Option<BoardGraphic> {
    let radius = distance(center, start);
    let reach = distance(center, end);
    if !finite(&[center, start, end]) || radius == 0.0 || reach == 0.0 {
        return None;
    }
    let scale = radius / reach;
    let on_circle = Point::new(
        center.x + (end.x - center.x) * scale,
        center.y + (end.y - center.y) * scale,
    );
    let mut arc = blank(id, BoardGraphicKind::Arc, style)?;
    arc.center = Some(center);
    arc.start = Some(start);
    arc.end = Some(on_circle);
    arc.radius = radius;
    Some(arc)
}

/// A circle about `center` through `rim`; `None` when they are one point.
pub fn new_circle(
    id: Uuid,
    center: Point,
    rim: Point,
    style: GraphicStyle,
) -> Option<BoardGraphic> {
    let radius = distance(center, rim);
    if !finite(&[center, rim]) || radius == 0.0 {
        return None;
    }
    let mut circle = blank(id, BoardGraphicKind::Circle, style)?;
    circle.center = Some(center);
    circle.end = Some(rim);
    circle.radius = radius;
    Some(circle)
}

/// Whether `a` and `b` span a box with area.
fn spans_area(a: Point, b: Point) -> bool {
    finite(&[a, b]) && a.x != b.x && a.y != b.y
}

/// The upright rectangle with opposite corners `a` and `b`; `None` for a
/// box with no area.
pub fn new_rectangle(id: Uuid, a: Point, b: Point, style: GraphicStyle) -> Option<BoardGraphic> {
    if !spans_area(a, b) {
        return None;
    }
    let mut rect = blank(id, BoardGraphicKind::Rectangle, style)?;
    rect.start = Some(a);
    rect.end = Some(b);
    Some(rect)
}

/// `text` wrapped in the box with opposite corners `a` and `b`, `size_mm`
/// high (the PCB default when not above zero). `None` for a box with no
/// area.
pub fn new_text_frame(
    id: Uuid,
    a: Point,
    b: Point,
    style: GraphicStyle,
    text: String,
    size_mm: f64,
) -> Option<BoardGraphic> {
    if !spans_area(a, b) {
        return None;
    }
    let mut frame = blank(id, BoardGraphicKind::TextFrame, style)?;
    frame.start = Some(a);
    frame.end = Some(b);
    frame.text = text;
    frame.font_size = if size_mm.is_finite() && size_mm > 0.0 {
        size_mm
    } else {
        PCB_DEFAULT_TEXT_SIZE_MM
    };
    Some(frame)
}

/// Add `graphic`: one undo step.
pub fn place_graphic(graphic: BoardGraphic) -> BoardCommand {
    BoardCommand::AddItems {
        items: vec![BoardItem::Graphic(graphic)],
    }
}

/// The board drawing named `id`.
pub fn graphic_of(board: &PcbBoard, id: Uuid) -> Option<&BoardGraphic> {
    board.graphics.iter().find(|g| g.uuid == id)
}

#[cfg(test)]
#[path = "graphic_tests.rs"]
mod tests;
