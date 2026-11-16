//! Via fields: position, diameter, hole size, net, lock and mask settings.

use signex_types::pcb::{MaskExpansion, PcbBoard, Via};
use uuid::Uuid;

use super::{PrimitiveEditRefusal, finite, finite_mask};
use crate::pcb::{BoardCommand, BoardItem};

/// One field of the selected vias, set to an absolute value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ViaEdit {
    X(f64),
    Y(f64),
    Diameter(f64),
    HoleSize(f64),
    /// 0 is no net.
    Net(u32),
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
    let found: Vec<&Via> = vias
        .iter()
        .map(|id| board.vias.iter().find(|v| v.uuid == *id))
        .collect::<Option<_>>()
        .ok_or(PrimitiveEditRefusal::Gone)?;
    let unlocking = matches!(edit, ViaEdit::Locked(_));
    if !unlocking && found.iter().any(|v| v.attributes.locked) {
        return Err(PrimitiveEditRefusal::ItemLocked);
    }
    let mut changed = Vec::new();
    for old in found {
        let mut new = old.clone();
        apply(&mut new, edit)?;
        if new != *old {
            changed.push(BoardItem::Via(new));
        }
    }
    Ok((!changed.is_empty()).then_some(BoardCommand::ReplaceItems { items: changed }))
}

fn apply(v: &mut Via, edit: ViaEdit) -> Result<(), PrimitiveEditRefusal> {
    match edit {
        ViaEdit::X(x) => v.position.x = finite(x)?,
        ViaEdit::Y(y) => v.position.y = finite(y)?,
        ViaEdit::Diameter(d) => v.diameter = finite(d)?,
        ViaEdit::HoleSize(h) => v.drill = finite(h)?,
        ViaEdit::Net(net) => v.net = net,
        ViaEdit::Locked(locked) => v.attributes.locked = locked,
        ViaEdit::SolderMaskTop(mask) => v.attributes.top.expansion = finite_mask(mask)?,
        ViaEdit::SolderMaskBottom(mask) => v.attributes.bottom.expansion = finite_mask(mask)?,
        ViaEdit::TentedTop(on) => v.attributes.top.tented = on,
        ViaEdit::TentedBottom(on) => v.attributes.bottom.tented = on,
        ViaEdit::FromHoleEdge(on) => v.attributes.from_hole_edge = on,
    }
    Ok(())
}
