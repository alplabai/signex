//! The board edit net assignment produces: applied, it returns its inverse.

use std::collections::HashMap;

use signex_types::pcb::{NetDef, PadNet, PcbBoard};
use uuid::Uuid;

/// One pad's net after the edit (`None` = no net).
#[derive(Debug, Clone, PartialEq)]
pub struct PadNetChange {
    pub footprint: Uuid,
    pub pad: Uuid,
    pub net: Option<PadNet>,
}

/// The board's net table plus the pads whose net changes. Pure data: it
/// holds no reference to the board and can be applied, stored or undone.
#[derive(Debug, Clone)]
pub struct BoardNetEdit {
    pub nets: Vec<NetDef>,
    pub pads: Vec<PadNetChange>,
}

impl BoardNetEdit {
    /// Write the edit into `board` and return the edit that undoes it.
    ///
    /// A change whose footprint or pad is no longer on the board is skipped
    /// and left out of the inverse, so the inverse only restores what this
    /// call actually wrote.
    pub fn apply(self, board: &mut PcbBoard) -> BoardNetEdit {
        let old_nets = std::mem::replace(&mut board.nets, self.nets);
        let footprint_at: HashMap<Uuid, usize> = board
            .footprints
            .iter()
            .enumerate()
            .map(|(i, fp)| (fp.uuid, i))
            .collect();
        let mut undo = Vec::with_capacity(self.pads.len());
        for change in self.pads {
            let Some(&fp_index) = footprint_at.get(&change.footprint) else {
                continue;
            };
            let footprint = &mut board.footprints[fp_index];
            let Some(pad) = footprint.pads.iter_mut().find(|p| p.uuid == change.pad) else {
                continue;
            };
            let old = std::mem::replace(&mut pad.net, change.net);
            undo.push(PadNetChange {
                footprint: change.footprint,
                pad: change.pad,
                net: old,
            });
        }
        BoardNetEdit {
            nets: old_nets,
            pads: undo,
        }
    }
}
