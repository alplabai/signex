//! Hand-assigning a pad's net: sets the net and locks it.

use signex_types::pcb::{NetDef, PadNet, PcbBoard};
use uuid::Uuid;

use super::edit::{BoardNetEdit, PadNetChange};

/// The edit that puts `net` on one pad and locks it, so a later sync leaves
/// the pad alone. The net table gains `net` when it has no row for its
/// number. Applying the edit returns the inverse, which restores the pad's
/// previous net, its previous lock flag and the previous table.
///
/// A pad that is not on the board makes the edit change nothing.
pub fn assign_pad_net(board: &PcbBoard, footprint: Uuid, pad: Uuid, net: PadNet) -> BoardNetEdit {
    let mut nets = board.nets.clone();
    if net.number != 0 && !nets.iter().any(|n| n.number == net.number) {
        nets.push(NetDef {
            number: net.number,
            name: net.name.clone(),
        });
    }
    BoardNetEdit {
        nets,
        pads: vec![PadNetChange {
            footprint,
            pad,
            net: Some(net),
            locked: true,
        }],
        zones: Vec::new(),
    }
}
