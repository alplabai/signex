//! Via fields: position, diameter, hole size and net.

use signex_types::pcb::{PcbBoard, Via};
use uuid::Uuid;

use super::{PrimitiveEditRefusal, finite};
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
    let mut changed = Vec::new();
    for id in vias {
        let old = board
            .vias
            .iter()
            .find(|v| v.uuid == *id)
            .ok_or(PrimitiveEditRefusal::Gone)?;
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
    }
    Ok(())
}
