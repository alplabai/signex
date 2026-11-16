//! Via fields: position, diameter, hole size, net, span (#2407), per-layer
//! diameters (#2401), and lock and mask settings (#2378).

use signex_types::format::pcb_layer_token;
use signex_types::pcb::{LayerSizes, MaskExpansion, PcbBoard, Via, ViaType};
use signex_types::pcb_layer::{CopperLayer, PcbLayer};
use signex_types::pcb_stack::{StackMode, board_copper_layers, sizes_in_mode};
use uuid::Uuid;

use super::{PrimitiveEditRefusal, finite, finite_mask};
use crate::pcb::{BoardCommand, BoardItem};

/// One field of the selected vias, set to an absolute value.
#[derive(Debug, Clone, PartialEq)]
pub enum ViaEdit {
    X(f64),
    Y(f64),
    Diameter(f64),
    HoleSize(f64),
    /// 0 is no net.
    Net(u32),
    /// The two copper layers the via joins, either order. Top to bottom is a
    /// through via, any other span a blind (or buried) one; a micro via
    /// stays micro while it is not through. The board command checks the
    /// layers are the board's.
    Span(CopperLayer, CopperLayer),
    /// Per-layer diameters; `None` is one diameter on every layer.
    Sizes(Option<LayerSizes<f64>>),
    /// How the diameter is set by layer (#2401); each layer keeps its size.
    Mode(StackMode),
    /// The diameter on one layer: its part of the per-layer sizes, or the
    /// one diameter of a via without them.
    LayerDiameter(CopperLayer, f64),
    /// Lock or unlock; the one edit a locked via takes (#2378).
    Locked(bool),
    SolderMaskTop(MaskExpansion),
    SolderMaskBottom(MaskExpansion),
    TentedTop(bool),
    TentedBottom(bool),
    FromHoleEdge(bool),
}

/// The command setting `edit` on every via in `vias`. Refused when a via is
/// locked and the edit is not the unlock.
pub fn via_edit_command(
    board: &PcbBoard,
    vias: &[Uuid],
    edit: ViaEdit,
) -> Result<Option<BoardCommand>, PrimitiveEditRefusal> {
    if vias.is_empty() {
        return Err(PrimitiveEditRefusal::Empty);
    }
    let copper = board_copper_layers(&board.layers);
    let found: Vec<&Via> = vias
        .iter()
        .map(|id| board.vias.iter().find(|v| v.uuid == *id))
        .collect::<Option<_>>()
        .ok_or(PrimitiveEditRefusal::Gone)?;
    let unlocking = matches!(edit, ViaEdit::Locked(_));
    if !unlocking && found.iter().any(|v| v.locked) {
        return Err(PrimitiveEditRefusal::ItemLocked);
    }
    let mut changed = Vec::new();
    for old in found {
        let mut new = old.clone();
        apply(&mut new, &edit, &copper)?;
        if new != *old {
            changed.push(BoardItem::Via(new));
        }
    }
    Ok((!changed.is_empty()).then_some(BoardCommand::ReplaceItems { items: changed }))
}

fn apply(v: &mut Via, edit: &ViaEdit, copper: &[CopperLayer]) -> Result<(), PrimitiveEditRefusal> {
    match edit {
        ViaEdit::X(x) => v.position.x = finite(*x)?,
        ViaEdit::Y(y) => v.position.y = finite(*y)?,
        ViaEdit::Diameter(d) => v.diameter = finite(*d)?,
        ViaEdit::HoleSize(h) => v.drill = finite(*h)?,
        ViaEdit::Net(net) => v.net = *net,
        ViaEdit::Span(a, b) => set_span(v, *a, *b),
        ViaEdit::Sizes(sizes) => v.layer_sizes = sizes.clone(),
        ViaEdit::Mode(mode) => {
            v.layer_sizes = sizes_in_mode(v.layer_sizes.as_ref(), v.diameter, *mode, copper);
        }
        ViaEdit::LayerDiameter(layer, d) => {
            let d = finite(*d)?;
            match v.layer_sizes.as_mut() {
                Some(sizes) => sizes.set(*layer, d),
                None => v.diameter = d,
            }
        }
        ViaEdit::Locked(locked) => v.locked = *locked,
        ViaEdit::SolderMaskTop(mask) => v.attributes.top.expansion = finite_mask(*mask)?,
        ViaEdit::SolderMaskBottom(mask) => v.attributes.bottom.expansion = finite_mask(*mask)?,
        ViaEdit::TentedTop(on) => v.attributes.top.tented = *on,
        ViaEdit::TentedBottom(on) => v.attributes.bottom.tented = *on,
        ViaEdit::FromHoleEdge(on) => v.attributes.from_hole_edge = *on,
    }
    Ok(())
}

fn set_span(v: &mut Via, a: CopperLayer, b: CopperLayer) {
    let (top, bottom) = (a.min(b), a.max(b));
    v.layers = [top, bottom]
        .into_iter()
        .map(|layer| pcb_layer_token(PcbLayer::Copper(layer)))
        .collect();
    let through = top == CopperLayer::Top && bottom == CopperLayer::Bottom;
    v.via_type = match (through, v.via_type) {
        (true, _) => ViaType::Through,
        (false, ViaType::Micro) => ViaType::Micro,
        (false, _) => ViaType::Blind,
    };
}
