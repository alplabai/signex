//! The board edit net assignment produces: applied, it returns its inverse.

use std::collections::{HashMap, HashSet};

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
#[derive(Debug, Clone, PartialEq)]
pub struct BoardNetEdit {
    pub nets: Vec<NetDef>,
    pub pads: Vec<PadNetChange>,
    /// Zones on a net the table renamed.
    pub zones: Vec<ZoneNetName>,
}

/// Why an edit cannot be written whole: it names something the board does not
/// hold, or names one pad or zone twice (its inverse would then not be exact).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetEditRefusal {
    FootprintNotFound(Uuid),
    PadNotFound {
        footprint: Uuid,
        pad: Uuid,
    },
    ZoneNotFound(Uuid),
    DuplicatePad {
        footprint: Uuid,
        pad: Uuid,
    },
    DuplicateZone(Uuid),
    /// After the edit the pad would still name net `number` (`name`), which
    /// the edit drops from the table.
    PadOnRemovedNet {
        footprint: Uuid,
        pad: Uuid,
        number: u32,
        name: String,
    },
}

impl BoardNetEdit {
    /// Whether [`Self::apply`] would write every change: each footprint, pad
    /// and zone is on `board`, none is named twice, and no pad is left on a
    /// net the edit removes from the table.
    pub fn check(&self, board: &PcbBoard) -> Result<(), NetEditRefusal> {
        self.check_items(board)?;
        self.check_removed_nets(board)
    }

    /// A net the edit removes must have no pad left on it. Only removed nets
    /// are checked: a pad a board already holds on a net missing from its
    /// table is left alone, so the undo of an edit is never refused.
    fn check_removed_nets(&self, board: &PcbBoard) -> Result<(), NetEditRefusal> {
        let kept: HashSet<u32> = self.nets.iter().map(|n| n.number).collect();
        let removed: HashSet<u32> = board
            .nets
            .iter()
            .map(|n| n.number)
            .filter(|number| !kept.contains(number))
            .collect();
        if removed.is_empty() {
            return Ok(());
        }
        let changed: HashMap<(Uuid, Uuid), Option<&PadNet>> = self
            .pads
            .iter()
            .map(|c| ((c.footprint, c.pad), c.net.as_ref()))
            .collect();
        for footprint in &board.footprints {
            for pad in &footprint.pads {
                let net = changed
                    .get(&(footprint.uuid, pad.uuid))
                    .copied()
                    .unwrap_or(pad.net.as_ref());
                if let Some(net) = net.filter(|n| removed.contains(&n.number)) {
                    return Err(NetEditRefusal::PadOnRemovedNet {
                        footprint: footprint.uuid,
                        pad: pad.uuid,
                        number: net.number,
                        name: net.name.clone(),
                    });
                }
            }
        }
        Ok(())
    }

    fn check_items(&self, board: &PcbBoard) -> Result<(), NetEditRefusal> {
        let mut pads = HashSet::with_capacity(self.pads.len());
        for change in &self.pads {
            let (footprint, pad) = (change.footprint, change.pad);
            if !pads.insert((footprint, pad)) {
                return Err(NetEditRefusal::DuplicatePad { footprint, pad });
            }
            let fp = board
                .footprints
                .iter()
                .find(|f| f.uuid == footprint)
                .ok_or(NetEditRefusal::FootprintNotFound(footprint))?;
            if !fp.pads.iter().any(|p| p.uuid == pad) {
                return Err(NetEditRefusal::PadNotFound { footprint, pad });
            }
        }
        let mut zones = HashSet::with_capacity(self.zones.len());
        for change in &self.zones {
            if !zones.insert(change.zone) {
                return Err(NetEditRefusal::DuplicateZone(change.zone));
            }
            if !board.zones.iter().any(|z| z.uuid == change.zone) {
                return Err(NetEditRefusal::ZoneNotFound(change.zone));
            }
        }
        Ok(())
    }

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
