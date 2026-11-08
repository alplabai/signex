//! Hand edits of one pad's net: assign a net and lock, or lock and unlock.

use signex_types::pcb::{NetDef, PadNet, PcbBoard};
use uuid::Uuid;

use super::edit::{BoardNetEdit, PadNetChange};

/// The edit that puts `net` on one pad and locks it, so a later sync leaves
/// the pad alone. The net table gains `net` when it has no row for its
/// number; when it already has a row for that number, the table's name wins
/// and the pad takes it, so the pad and the table never disagree. Applying
/// the edit returns the inverse, which restores the pad's previous net, its
/// previous lock flag and the previous table.
///
/// A footprint or pad that is not on the board makes an empty edit: the net
/// table is left as it is too.
pub fn assign_pad_net(board: &PcbBoard, footprint: Uuid, pad: Uuid, net: PadNet) -> BoardNetEdit {
    if find_pad(board, footprint, pad).is_none() {
        return BoardNetEdit::unchanged(board);
    }
    let mut nets = board.nets.clone();
    let mut net = net;
    if net.number != 0 {
        match nets.iter().find(|n| n.number == net.number) {
            Some(row) => net.name = row.name.clone(),
            None => nets.push(NetDef {
                number: net.number,
                name: net.name.clone(),
            }),
        }
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

/// The edit that locks or unlocks one pad's net and keeps the net itself.
/// Unlocking lets the next sync set the pad again. Applying it returns the
/// exact inverse. A pad that is not on the board makes an empty edit.
pub fn set_pad_net_lock(
    board: &PcbBoard,
    footprint: Uuid,
    pad: Uuid,
    locked: bool,
) -> BoardNetEdit {
    let Some(current) = find_pad(board, footprint, pad) else {
        return BoardNetEdit::unchanged(board);
    };
    BoardNetEdit {
        nets: board.nets.clone(),
        pads: vec![PadNetChange {
            footprint,
            pad,
            net: current.net.clone(),
            locked,
        }],
        zones: Vec::new(),
    }
}

fn find_pad(board: &PcbBoard, footprint: Uuid, pad: Uuid) -> Option<&signex_types::pcb::Pad> {
    board
        .footprints
        .iter()
        .find(|f| f.uuid == footprint)?
        .pads
        .iter()
        .find(|p| p.uuid == pad)
}
