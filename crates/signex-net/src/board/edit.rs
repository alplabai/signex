//! The board edit net assignment produces: applied, it returns its inverse.

use std::collections::HashMap;

use signex_types::pcb::{NetDef, PadNet, PcbBoard};
use uuid::Uuid;

/// One pad's net and net lock after the edit (`None` = no net).
#[derive(Debug, Clone, PartialEq)]
pub struct PadNetChange {
    pub footprint: Uuid,
    pub pad: Uuid,
    pub net: Option<PadNet>,
    pub locked: bool,
}

/// One zone's net name after the edit; its net number never changes.
#[derive(Debug, Clone, PartialEq)]
pub struct ZoneNetName {
    pub zone: Uuid,
    pub name: String,
}

/// The board's net table plus the pads whose net changes. Pure data: it
/// holds no reference to the board and can be applied, stored or undone.
#[derive(Debug, Clone)]
pub struct BoardNetEdit {
    pub nets: Vec<NetDef>,
    pub pads: Vec<PadNetChange>,
    /// Zones on a net the table renamed.
    pub zones: Vec<ZoneNetName>,
}

impl BoardNetEdit {
    /// An edit that changes nothing: the board's own net table, no pads.
    pub fn unchanged(board: &PcbBoard) -> Self {
        BoardNetEdit {
            nets: board.nets.clone(),
            pads: Vec::new(),
            zones: Vec::new(),
        }
    }

    /// Write the edit into `board` and return the edit that undoes it.
    ///
    /// A change whose footprint, pad or zone is no longer on the board is
    /// skipped and left out of the inverse, so the inverse only restores what
    /// this call actually wrote.
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
            let old_locked = std::mem::replace(&mut pad.net_locked, change.locked);
            undo.push(PadNetChange {
                footprint: change.footprint,
                pad: change.pad,
                net: old,
                locked: old_locked,
            });
        }
        let mut undo_zones = Vec::with_capacity(self.zones.len());
        for change in self.zones {
            if let Some(zone) = board.zones.iter_mut().find(|z| z.uuid == change.zone) {
                let old = std::mem::replace(&mut zone.net_name, change.name);
                undo_zones.push(ZoneNetName {
                    zone: change.zone,
                    name: old,
                });
            }
        }
        BoardNetEdit {
            nets: old_nets,
            pads: undo,
            zones: undo_zones,
        }
    }
}
