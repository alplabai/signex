//! Board drawing fields (#2424): the layer and line width of any drawing,
//! the corners of a line, rectangle or text frame, the centre and radius of
//! an arc or circle, and a text frame's text and text height.

use signex_types::format::{parse_pcb_layer, pcb_layer_token};
use signex_types::pcb::{BoardGraphic, PcbBoard, Point};
use signex_types::pcb_graphic::{BoardGraphicKind, takes_board_graphics};
use signex_types::pcb_layer::PcbLayer;
use uuid::Uuid;

use super::{PrimitiveEditRefusal, finite};
use crate::pcb::{BoardCommand, BoardItem};

/// One field of the selected drawings, set to an absolute value. A field a
/// drawing's kind does not have leaves it as it is.
#[derive(Debug, Clone, PartialEq)]
pub enum GraphicEdit {
    /// Any layer but a keep-out (copper too, #2883).
    Layer(PcbLayer),
    /// The stroke width in mm, 0 or more.
    Width(f64),
    /// A line's, rectangle's or text frame's first point.
    StartX(f64),
    StartY(f64),
    /// A line's, rectangle's or text frame's second point.
    EndX(f64),
    EndY(f64),
    /// Moves an arc or circle so its centre has this x.
    CenterX(f64),
    CenterY(f64),
    /// An arc's or circle's radius in mm, above 0: its points move along
    /// their angles.
    Radius(f64),
    /// A text frame's text; `\n` breaks a line.
    Text(String),
    /// A text frame's text height in mm, above 0.
    TextHeight(f64),
}

/// The command setting `edit` on every drawing in `graphics`.
pub fn graphic_edit_command(
    board: &PcbBoard,
    graphics: &[Uuid],
    edit: &GraphicEdit,
) -> Result<Option<BoardCommand>, PrimitiveEditRefusal> {
    if graphics.is_empty() {
        return Err(PrimitiveEditRefusal::Empty);
    }
    let mut changed = Vec::new();
    for id in graphics {
        let old = board
            .graphics
            .iter()
            .find(|g| g.uuid == *id)
            .ok_or(PrimitiveEditRefusal::Gone)?;
        let mut new = old.clone();
        apply_graphic_edit(&mut new, edit)?;
        if new != *old {
            changed.push(BoardItem::Graphic(new));
        }
    }
    Ok((!changed.is_empty()).then_some(BoardCommand::ReplaceItems { items: changed }))
}

/// Set `edit` on `graphic`, as [`graphic_edit_command`] does on each
/// drawing. The Graphics tools use it for the drawing about to be placed.
pub fn apply_graphic_edit(
    graphic: &mut BoardGraphic,
    edit: &GraphicEdit,
) -> Result<(), PrimitiveEditRefusal> {
    let kind = graphic.kind();
    let corners = matches!(
        kind,
        Some(BoardGraphicKind::Line | BoardGraphicKind::Rectangle | BoardGraphicKind::TextFrame)
    );
    let round = matches!(kind, Some(BoardGraphicKind::Arc | BoardGraphicKind::Circle));
    let frame = kind == Some(BoardGraphicKind::TextFrame);
    match edit {
        GraphicEdit::Layer(layer) => {
            if !takes_board_graphics(*layer) {
                return Err(PrimitiveEditRefusal::GraphicLayerNotAllowed);
            }
            if parse_pcb_layer(&graphic.layer) != Some(*layer) {
                graphic.layer = pcb_layer_token(*layer);
            }
        }
        GraphicEdit::Width(w) => graphic.width = at_least_zero(*w)?,
        GraphicEdit::StartX(x) if corners => set_x(&mut graphic.start, finite(*x)?),
        GraphicEdit::StartY(y) if corners => set_y(&mut graphic.start, finite(*y)?),
        GraphicEdit::EndX(x) if corners => set_x(&mut graphic.end, finite(*x)?),
        GraphicEdit::EndY(y) if corners => set_y(&mut graphic.end, finite(*y)?),
        GraphicEdit::CenterX(x) if round => {
            let at = graphic.center.ok_or(PrimitiveEditRefusal::Gone)?;
            shift(graphic, finite(*x)? - at.x, 0.0);
        }
        GraphicEdit::CenterY(y) if round => {
            let at = graphic.center.ok_or(PrimitiveEditRefusal::Gone)?;
            shift(graphic, 0.0, finite(*y)? - at.y);
        }
        GraphicEdit::Radius(r) if round => set_radius(graphic, above_zero(*r)?),
        GraphicEdit::Text(text) if frame => graphic.text = text.clone(),
        GraphicEdit::TextHeight(h) if frame => graphic.font_size = above_zero(*h)?,
        _ => {}
    }
    Ok(())
}

fn at_least_zero(value: f64) -> Result<f64, PrimitiveEditRefusal> {
    if finite(value)? < 0.0 {
        return Err(PrimitiveEditRefusal::Negative);
    }
    Ok(value)
}

fn above_zero(value: f64) -> Result<f64, PrimitiveEditRefusal> {
    if finite(value)? <= 0.0 {
        return Err(PrimitiveEditRefusal::NotPositive);
    }
    Ok(value)
}

fn set_x(point: &mut Option<Point>, x: f64) {
    let y = point.map_or(0.0, |p| p.y);
    *point = Some(Point::new(x, y));
}

fn set_y(point: &mut Option<Point>, y: f64) {
    let x = point.map_or(0.0, |p| p.x);
    *point = Some(Point::new(x, y));
}

/// Move every point of `graphic` by `(dx, dy)`.
fn shift(graphic: &mut BoardGraphic, dx: f64, dy: f64) {
    for point in [&mut graphic.start, &mut graphic.end, &mut graphic.center]
        .into_iter()
        .flatten()
    {
        *point = Point::new(point.x + dx, point.y + dy);
    }
}

/// Set an arc's or circle's radius, its start and end kept on their angles
/// about the centre.
fn set_radius(graphic: &mut BoardGraphic, radius: f64) {
    graphic.radius = radius;
    let Some(center) = graphic.center else {
        return;
    };
    for point in [&mut graphic.start, &mut graphic.end].into_iter().flatten() {
        let (dx, dy) = (point.x - center.x, point.y - center.y);
        let reach = dx.hypot(dy);
        *point = if reach > 0.0 {
            Point::new(
                center.x + dx * radius / reach,
                center.y + dy * radius / reach,
            )
        } else {
            Point::new(center.x + radius, center.y)
        };
    }
}
