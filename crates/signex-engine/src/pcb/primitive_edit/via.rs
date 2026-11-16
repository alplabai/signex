//! Via fields: position, diameter, hole size, net, span (#2407) and
//! per-layer diameters (#2401).

use signex_types::format::pcb_layer_token;
use signex_types::pcb::{LayerSizes, PcbBoard, Via, ViaType};
use signex_types::pcb_layer::{CopperLayer, PcbLayer};
use signex_types::pcb_stack::{StackMode, board_copper_layers, sizes_in_mode};
use uuid::Uuid;

use super::{PrimitiveEditRefusal, finite};
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
}

/// The command setting `edit` on every via in `vias`.
pub fn via_edit_command(
    board: &PcbBoard,
    vias: &[Uuid],
    edit: ViaEdit,
) -> Result<Option<BoardCommand>, PrimitiveEditRefusal> {
    if vias.is_empty() {
        return Err(PrimitiveEditRefusal::Empty);
    }
    let copper = board_copper_layers(&board.layers);
    let mut changed = Vec::new();
    for id in vias {
        let old = board
            .vias
            .iter()
            .find(|v| v.uuid == *id)
            .ok_or(PrimitiveEditRefusal::Gone)?;
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
