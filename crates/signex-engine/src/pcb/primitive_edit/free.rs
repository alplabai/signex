//! Fields of free arcs, free pads, board strings and dimensions (#2373),
//! each set on every selected item as one [`BoardCommand::ReplaceItems`].
//! Positions and angles are the model's: the app turns what the user typed
//! from the board origin (Y up) into these.

use signex_types::format::pcb_layer_token;
use signex_types::pcb::{BoardText, Pad, PcbBoard, Point};
use signex_types::pcb_arc::{ArcSweep, TrackArc};
use signex_types::pcb_dimension::LinearDimension;
use signex_types::pcb_free::free_pad_frame;
use signex_types::pcb_layer::PcbLayer;
use uuid::Uuid;

use super::pad::{PadEdit, apply as apply_pad};
use super::{PrimitiveEditRefusal, finite};
use crate::pcb::{BoardCommand, BoardItem};

/// An arc as its circle and the angles it runs between, in degrees: from
/// `from_deg` up to `to_deg` as the model's angle rises (clockwise on
/// screen, where y grows down).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ArcSpan {
    pub center: Point,
    pub radius: f64,
    pub from_deg: f64,
    pub to_deg: f64,
}

impl ArcSpan {
    /// The span of `arc`; `None` for one with no circle.
    pub fn of(arc: &TrackArc) -> Option<Self> {
        let sweep = arc.sweep()?;
        let (from, to) = if sweep.sweep >= 0.0 {
            (sweep.start_angle, sweep.start_angle + sweep.sweep)
        } else {
            (sweep.start_angle + sweep.sweep, sweep.start_angle)
        };
        Some(Self {
            center: sweep.center,
            radius: sweep.radius,
            from_deg: from.to_degrees().rem_euclid(360.0),
            to_deg: to.to_degrees().rem_euclid(360.0),
        })
    }

    /// The start, a point on the arc and the end; `None` for a span with no
    /// turn or radius.
    fn three_points(self) -> Option<(Point, Point, Point)> {
        let turn = (self.to_deg - self.from_deg).rem_euclid(360.0);
        if turn == 0.0 || !(self.radius.is_finite() && self.radius > 0.0) {
            return None;
        }
        let sweep = ArcSweep {
            center: self.center,
            radius: self.radius,
            start_angle: self.from_deg.to_radians(),
            sweep: turn.to_radians(),
        };
        Some(sweep.three_points())
    }
}

/// One field of the selected arcs, set to an absolute value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ArcEdit {
    CenterX(f64),
    CenterY(f64),
    Radius(f64),
    FromAngle(f64),
    ToAngle(f64),
    Width(f64),
    /// A copper layer keeps the net; any other layer drops it.
    Layer(PcbLayer),
    /// 0 is no net.
    Net(u32),
}

/// The command setting `edit` on every arc in `arcs`.
pub fn arc_edit_command(
    board: &PcbBoard,
    arcs: &[Uuid],
    edit: ArcEdit,
) -> Result<Option<BoardCommand>, PrimitiveEditRefusal> {
    let found = find_all(&board.free.arcs, arcs, |a| a.uuid)?;
    replace_each(found, BoardItem::Arc, |arc| apply_arc(arc, edit))
}

/// `edit` applied to `arc`: the edit of a placed arc and of the one the
/// Arc tool is about to place.
pub fn apply_arc(arc: &mut TrackArc, edit: ArcEdit) -> Result<(), PrimitiveEditRefusal> {
    let shaped =
        |arc: &mut TrackArc, change: &dyn Fn(&mut ArcSpan)| -> Result<(), PrimitiveEditRefusal> {
            let mut span = ArcSpan::of(arc).ok_or(PrimitiveEditRefusal::NoArc)?;
            change(&mut span);
            let (start, mid, end) = span.three_points().ok_or(PrimitiveEditRefusal::NoArc)?;
            (arc.start, arc.mid, arc.end) = (start, mid, end);
            Ok(())
        };
    match edit {
        ArcEdit::CenterX(x) => {
            let span = ArcSpan::of(arc).ok_or(PrimitiveEditRefusal::NoArc)?;
            let dx = finite(x)? - span.center.x;
            for p in [&mut arc.start, &mut arc.mid, &mut arc.end] {
                p.x += dx;
            }
        }
        ArcEdit::CenterY(y) => {
            let span = ArcSpan::of(arc).ok_or(PrimitiveEditRefusal::NoArc)?;
            let dy = finite(y)? - span.center.y;
            for p in [&mut arc.start, &mut arc.mid, &mut arc.end] {
                p.y += dy;
            }
        }
        ArcEdit::Radius(r) => {
            let r = finite(r)?;
            shaped(arc, &|span: &mut ArcSpan| span.radius = r)?;
        }
        ArcEdit::FromAngle(deg) => {
            let deg = finite(deg)?;
            shaped(arc, &|span: &mut ArcSpan| span.from_deg = deg)?;
        }
        ArcEdit::ToAngle(deg) => {
            let deg = finite(deg)?;
            shaped(arc, &|span: &mut ArcSpan| span.to_deg = deg)?;
        }
        ArcEdit::Width(w) => arc.width = finite(w)?,
        ArcEdit::Layer(layer) => {
            arc.layer = pcb_layer_token(layer);
            if !layer.is_copper() {
                arc.net = 0;
            }
        }
        ArcEdit::Net(net) => {
            let copper = arc.pcb_layer().is_some_and(PcbLayer::is_copper);
            arc.net = if copper { net } else { 0 };
        }
    }
    Ok(())
}

/// One field of the selected strings, set to an absolute value.
#[derive(Debug, Clone, PartialEq)]
pub enum TextEdit {
    Text(String),
    X(f64),
    Y(f64),
    Height(f64),
    Rotation(f64),
    Layer(PcbLayer),
}

/// The command setting `edit` on every string in `texts`.
pub fn text_edit_command(
    board: &PcbBoard,
    texts: &[Uuid],
    edit: &TextEdit,
) -> Result<Option<BoardCommand>, PrimitiveEditRefusal> {
    let found = find_all(&board.texts, texts, |t| t.uuid)?;
    replace_each(found, BoardItem::Text, |text| apply_text(text, edit))
}

/// `edit` applied to `text`.
pub fn apply_text(text: &mut BoardText, edit: &TextEdit) -> Result<(), PrimitiveEditRefusal> {
    match edit {
        TextEdit::Text(value) => text.text = value.clone(),
        TextEdit::X(x) => text.position.x = finite(*x)?,
        TextEdit::Y(y) => text.position.y = finite(*y)?,
        TextEdit::Height(h) => text.font_size = finite(*h)?,
        TextEdit::Rotation(r) => text.rotation = finite(*r)?.rem_euclid(360.0),
        TextEdit::Layer(layer) if layer.takes_text() => text.layer = pcb_layer_token(*layer),
        TextEdit::Layer(_) => return Err(PrimitiveEditRefusal::LayerNotAllowed),
    }
    Ok(())
}

/// One field of the selected dimensions, set to an absolute value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DimensionEdit {
    StartX(f64),
    StartY(f64),
    EndX(f64),
    EndY(f64),
    Offset(f64),
    LineWidth(f64),
    TextHeight(f64),
    Precision(u8),
    Layer(PcbLayer),
}

/// The most decimals a dimension shows.
pub const DIMENSION_MAX_PRECISION: u8 = 6;

/// The command setting `edit` on every dimension in `dimensions`.
pub fn dimension_edit_command(
    board: &PcbBoard,
    dimensions: &[Uuid],
    edit: DimensionEdit,
) -> Result<Option<BoardCommand>, PrimitiveEditRefusal> {
    let found = find_all(&board.free.dimensions, dimensions, |d| d.uuid)?;
    replace_each(found, BoardItem::Dimension, |d| apply_dimension(d, edit))
}

/// `edit` applied to `dimension`.
pub fn apply_dimension(
    dimension: &mut LinearDimension,
    edit: DimensionEdit,
) -> Result<(), PrimitiveEditRefusal> {
    match edit {
        DimensionEdit::StartX(v) => dimension.start.x = finite(v)?,
        DimensionEdit::StartY(v) => dimension.start.y = finite(v)?,
        DimensionEdit::EndX(v) => dimension.end.x = finite(v)?,
        DimensionEdit::EndY(v) => dimension.end.y = finite(v)?,
        DimensionEdit::Offset(v) => dimension.offset = finite(v)?,
        DimensionEdit::LineWidth(v) => dimension.line_width = finite(v)?,
        DimensionEdit::TextHeight(v) => dimension.text_height = finite(v)?,
        DimensionEdit::Precision(p) if p <= DIMENSION_MAX_PRECISION => dimension.precision = p,
        DimensionEdit::Precision(_) => return Err(PrimitiveEditRefusal::OutOfRange),
        DimensionEdit::Layer(layer) if layer.takes_text() => {
            dimension.layer = pcb_layer_token(layer);
        }
        DimensionEdit::Layer(_) => return Err(PrimitiveEditRefusal::LayerNotAllowed),
    }
    if dimension.start == dimension.end {
        return Err(PrimitiveEditRefusal::NoLength);
    }
    Ok(())
}

/// The command setting `edit` on every free pad in `pads`. Positions and
/// turns are the board's, since a free pad has no footprint.
pub fn free_pad_edit_command(
    board: &PcbBoard,
    pads: &[Uuid],
    edit: &PadEdit,
) -> Result<Option<BoardCommand>, PrimitiveEditRefusal> {
    let found = find_all(&board.free.pads, pads, |p| p.uuid)?;
    replace_each(found, BoardItem::FreePad, |pad| {
        apply_free_pad(board, pad, edit)
    })
}

/// `edit` applied to the free pad `pad`.
pub fn apply_free_pad(
    board: &PcbBoard,
    pad: &mut Pad,
    edit: &PadEdit,
) -> Result<(), PrimitiveEditRefusal> {
    apply_pad(board, &free_pad_frame(), pad, edit)
}

/// Every item of `list` named in `ids`, in the order named.
fn find_all<'a, T>(
    list: &'a [T],
    ids: &[Uuid],
    uuid: impl Fn(&T) -> Uuid,
) -> Result<Vec<&'a T>, PrimitiveEditRefusal> {
    if ids.is_empty() {
        return Err(PrimitiveEditRefusal::Empty);
    }
    ids.iter()
        .map(|id| list.iter().find(|x| uuid(x) == *id))
        .collect::<Option<_>>()
        .ok_or(PrimitiveEditRefusal::Gone)
}

/// The replace of every item `change` changes; `None` when none changes.
fn replace_each<T: Clone + PartialEq>(
    found: Vec<&T>,
    wrap: fn(T) -> BoardItem,
    mut change: impl FnMut(&mut T) -> Result<(), PrimitiveEditRefusal>,
) -> Result<Option<BoardCommand>, PrimitiveEditRefusal> {
    let mut changed = Vec::new();
    for old in found {
        let mut new = old.clone();
        change(&mut new)?;
        if new != *old {
            changed.push(wrap(new));
        }
    }
    Ok((!changed.is_empty()).then_some(BoardCommand::ReplaceItems { items: changed }))
}
