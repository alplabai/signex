//! Pad fields: designator, board position and rotation, size, shape, hole
//! and net. Position and rotation are read and written on the board; the
//! model keeps them footprint-local, so they go through the footprint's
//! placement (`pcb_geom::Placement`).

use pcb_geom::{Placement, PointNm, Side, pad_world_center};
use signex_types::pcb::{Footprint, LayerSizes, Pad, PadNet, PadShape, PcbBoard, Point};
use signex_types::pcb_layer::CopperLayer;
use signex_types::pcb_stack::{StackMode, board_copper_layers, sizes_in_mode};
use uuid::Uuid;

use super::{PrimitiveEditRefusal, finite};
use crate::pcb::{BoardCommand, FootprintPad, placement};

/// One field of the selected pads, set to an absolute value.
#[derive(Debug, Clone, PartialEq)]
pub enum PadEdit {
    Designator(String),
    /// The pad centre's board x.
    X(f64),
    Y(f64),
    /// The pad's angle on the board, degrees counter-clockwise.
    Rotation(f64),
    SizeX(f64),
    SizeY(f64),
    Shape(PadShape),
    HoleSize(f64),
    /// 0 is no net. A net set here is kept when the schematic is synced
    /// (`Pad::net_locked`).
    Net(u32),
    /// Per-layer copper sizes (#2401); `None` is one size on every layer.
    Sizes(Option<LayerSizes<Point>>),
    /// How the size is set by layer; each layer keeps its size.
    Mode(StackMode),
    /// The x or y size on one layer: its part of the per-layer sizes, or the
    /// one size of a pad without them.
    LayerSizeX(CopperLayer, f64),
    LayerSizeY(CopperLayer, f64),
}

/// The command setting `edit` on every pad in `pads` (footprint, pad). A
/// pad of a locked footprint is refused, as for placement edits, and so is
/// a pad of a footprint that locks its primitives (#2365).
pub fn pad_edit_command(
    board: &PcbBoard,
    pads: &[(Uuid, Uuid)],
    edit: &PadEdit,
) -> Result<Option<BoardCommand>, PrimitiveEditRefusal> {
    if pads.is_empty() {
        return Err(PrimitiveEditRefusal::Empty);
    }
    let mut changed = Vec::new();
    for (fp_id, pad_id) in pads {
        let fp = board
            .footprints
            .iter()
            .find(|fp| fp.uuid == *fp_id)
            .ok_or(PrimitiveEditRefusal::Gone)?;
        let old = fp
            .pads
            .iter()
            .find(|p| p.uuid == *pad_id)
            .ok_or(PrimitiveEditRefusal::Gone)?;
        if fp.locked {
            return Err(PrimitiveEditRefusal::Locked(placement::name(fp)));
        }
        if fp.component.lock_primitives {
            return Err(PrimitiveEditRefusal::PrimitivesLocked(placement::name(fp)));
        }
        let mut new = old.clone();
        apply(board, fp, &mut new, edit)?;
        if new != *old {
            changed.push(FootprintPad {
                footprint: fp.uuid,
                pad: new,
            });
        }
    }
    Ok((!changed.is_empty()).then_some(BoardCommand::ReplacePads { pads: changed }))
}

/// `edit` applied to `pad` of `fp`; a free pad's frame is
/// `pcb_free::free_pad_frame` (#2373).
pub(super) fn apply(
    board: &PcbBoard,
    fp: &Footprint,
    pad: &mut Pad,
    edit: &PadEdit,
) -> Result<(), PrimitiveEditRefusal> {
    match edit {
        PadEdit::Designator(text) => pad.number = text.trim().to_string(),
        PadEdit::X(x) => move_to(fp, pad, Some(finite(*x)?), None)?,
        PadEdit::Y(y) => move_to(fp, pad, None, Some(finite(*y)?))?,
        PadEdit::Rotation(degrees) => {
            let world = finite(*degrees)?;
            let own = match Side::of_layer(&fp.layer) {
                Side::Top => world - fp.rotation,
                Side::Bottom => fp.rotation - world,
            };
            pad.rotation = own.rem_euclid(360.0);
        }
        PadEdit::SizeX(x) => pad.size.x = finite(*x)?,
        PadEdit::SizeY(y) => pad.size.y = finite(*y)?,
        PadEdit::Shape(shape) => pad.shape = *shape,
        PadEdit::Sizes(sizes) => pad.layer_sizes = sizes.clone(),
        PadEdit::Mode(mode) => {
            let copper = board_copper_layers(&board.layers);
            pad.layer_sizes = sizes_in_mode(pad.layer_sizes.as_ref(), pad.size, *mode, &copper);
        }
        PadEdit::LayerSizeX(layer, x) => set_layer_size(pad, *layer, Some(finite(*x)?), None),
        PadEdit::LayerSizeY(layer, y) => set_layer_size(pad, *layer, None, Some(finite(*y)?)),
        PadEdit::HoleSize(h) => {
            let hole = finite(*h)?;
            let drill = pad
                .drill
                .as_mut()
                .ok_or_else(|| PrimitiveEditRefusal::NoHole(pad.number.clone()))?;
            drill.diameter = hole;
        }
        PadEdit::Net(0) => {
            pad.net = None;
            pad.net_locked = true;
        }
        PadEdit::Net(number) => {
            // A number missing from the table keeps an empty name; the
            // command refuses it as an unknown net.
            let name = board
                .nets
                .iter()
                .find(|n| n.number == *number)
                .map(|n| n.name.clone())
                .unwrap_or_default();
            pad.net = Some(PadNet {
                number: *number,
                name,
            });
            pad.net_locked = true;
        }
    }
    Ok(())
}

/// Set the pad's x and/or y size on `layer`, the other axis kept.
fn set_layer_size(pad: &mut Pad, layer: CopperLayer, x: Option<f64>, y: Option<f64>) {
    let old = pad.size_on(layer);
    let new = Point::new(x.unwrap_or(old.x), y.unwrap_or(old.y));
    match pad.layer_sizes.as_mut() {
        Some(sizes) => sizes.set(layer, new),
        None => pad.size = new,
    }
}

/// Put the pad's centre at board `x` and/or `y`, the other axis kept.
fn move_to(
    fp: &Footprint,
    pad: &mut Pad,
    x: Option<f64>,
    y: Option<f64>,
) -> Result<(), PrimitiveEditRefusal> {
    let out_of_range = |_| PrimitiveEditRefusal::OutOfRange;
    let placement = Placement::of_footprint(fp).map_err(out_of_range)?;
    let mut world = pad_world_center(fp, pad).map_err(out_of_range)?.to_mm();
    world.x = x.unwrap_or(world.x);
    world.y = y.unwrap_or(world.y);
    let at = PointNm::from_mm(world, "pad position").map_err(out_of_range)?;
    pad.position = placement.to_local(at).to_mm();
    Ok(())
}
