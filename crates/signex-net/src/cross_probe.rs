//! Cross probe between the schematic and the board (#1312): which footprints
//! stand for a set of placed symbols, which symbols stand for a set of
//! footprints, and what on the board carries a set of nets.
//!
//! A part is matched by its sheet occurrence and a unit UUID against the
//! footprint's stored [`SymbolLink`](signex_types::pcb::SymbolLink), never by
//! designator. A net is matched by name. Footprints with no link match
//! nothing.

use std::collections::BTreeSet;

use signex_types::pcb::PcbBoard;
use signex_types::schematic::InstancePath;
use uuid::Uuid;

/// One placed symbol unit on one sheet occurrence.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PartOccurrence {
    pub sheet: InstancePath,
    pub symbol: Uuid,
}

/// The footprints linked to any of `parts`, in board order. A multi-unit
/// part is found from any one of its units.
pub fn footprints_for_parts(board: &PcbBoard, parts: &[PartOccurrence]) -> Vec<Uuid> {
    board
        .footprints
        .iter()
        .filter(|fp| {
            fp.schematic_link.as_ref().is_some_and(|link| {
                parts
                    .iter()
                    .any(|p| link.names_part(&p.sheet, std::slice::from_ref(&p.symbol)))
            })
        })
        .map(|fp| fp.uuid)
        .collect()
}

/// Every unit of the parts `footprints` are linked to, sorted and without
/// repeats. An unlinked or unknown footprint adds nothing.
pub fn parts_for_footprints(
    board: &PcbBoard,
    footprints: impl IntoIterator<Item = Uuid>,
) -> Vec<PartOccurrence> {
    let wanted: BTreeSet<Uuid> = footprints.into_iter().collect();
    let parts: BTreeSet<PartOccurrence> = board
        .footprints
        .iter()
        .filter(|fp| wanted.contains(&fp.uuid))
        .filter_map(|fp| fp.schematic_link.as_ref())
        .flat_map(|link| {
            link.units.iter().map(|unit| PartOccurrence {
                sheet: link.sheet.clone(),
                symbol: *unit,
            })
        })
        .collect();
    parts.into_iter().collect()
}

/// The board net numbers whose name is one of `names`. Net 0 (no net) and an
/// empty name never match.
pub fn nets_named(board: &PcbBoard, names: &[String]) -> BTreeSet<u32> {
    board
        .nets
        .iter()
        .filter(|net| net.number != 0 && !net.name.trim().is_empty())
        .filter(|net| names.iter().any(|name| name.trim() == net.name.trim()))
        .map(|net| net.number)
        .collect()
}

/// The names of the board nets `nets`, sorted and without repeats; a number
/// the board does not define has no name.
pub fn net_names(board: &PcbBoard, nets: &BTreeSet<u32>) -> Vec<String> {
    let names: BTreeSet<String> = board
        .nets
        .iter()
        .filter(|net| net.number != 0 && nets.contains(&net.number))
        .map(|net| net.name.trim().to_string())
        .filter(|name| !name.is_empty())
        .collect();
    names.into_iter().collect()
}

/// What on the board carries one of a set of nets.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BoardNetMembers {
    /// `(footprint, pad)` of every pad on the nets.
    pub pads: Vec<(Uuid, Uuid)>,
    pub tracks: Vec<Uuid>,
    pub vias: Vec<Uuid>,
}

impl BoardNetMembers {
    pub fn is_empty(&self) -> bool {
        self.pads.is_empty() && self.tracks.is_empty() && self.vias.is_empty()
    }
}

/// The pads, tracks and vias on `nets`, in board order.
pub fn board_net_members(board: &PcbBoard, nets: &BTreeSet<u32>) -> BoardNetMembers {
    if nets.is_empty() {
        return BoardNetMembers::default();
    }
    let pads = board
        .footprints
        .iter()
        .flat_map(|fp| fp.pads.iter().map(move |pad| (fp.uuid, pad)))
        .filter(|(_, pad)| pad.net.as_ref().is_some_and(|n| nets.contains(&n.number)))
        .map(|(fp, pad)| (fp, pad.uuid))
        .collect();
    BoardNetMembers {
        pads,
        tracks: board
            .segments
            .iter()
            .filter(|s| nets.contains(&s.net))
            .map(|s| s.uuid)
            .collect(),
        vias: board
            .vias
            .iter()
            .filter(|v| nets.contains(&v.net))
            .map(|v| v.uuid)
            .collect(),
    }
}

/// The net of the pad `pad` of footprint `footprint`, when it has one.
pub fn pad_net(board: &PcbBoard, footprint: Uuid, pad: Uuid) -> Option<u32> {
    board
        .footprints
        .iter()
        .find(|fp| fp.uuid == footprint)?
        .pads
        .iter()
        .find(|p| p.uuid == pad)?
        .net
        .as_ref()
        .map(|n| n.number)
        .filter(|&n| n != 0)
}

#[cfg(test)]
#[path = "cross_probe_tests.rs"]
mod tests;
