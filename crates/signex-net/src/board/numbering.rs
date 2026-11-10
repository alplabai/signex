//! Board net numbers: an existing board net keeps its number across syncs.
//!
//! `NetId`s are geometry-derived and renumber when a wire moves, but tracks,
//! vias and zones store the board's number. So each netlist net is matched to
//! an existing `NetDef` by the pads it holds, and only a new net gets a
//! fresh number (#402).

use std::collections::{HashMap, HashSet};

use signex_types::net::{NetId, Netlist};
use signex_types::pcb::{NetDef, PcbBoard};
use uuid::Uuid;

pub(super) type PadRef = (Uuid, Uuid);

pub(super) struct Numbered {
    /// The new net table: netlist nets, then old nets still in use.
    pub table: Vec<NetDef>,
    pub number_of: HashMap<NetId, u32>,
    pub name_of: HashMap<NetId, String>,
    /// Matched board nets whose name changed: number -> new name.
    pub renamed: HashMap<u32, String>,
    /// Retained nets that only copper uses (no kept pad holds them).
    pub copper_only: Vec<u32>,
}

/// `assigned` maps each pad the sync sets to its net; `kept` are the pads it
/// leaves alone (their old net number stays in use).
pub(super) fn number_nets(
    netlist: &Netlist,
    board: &PcbBoard,
    assigned: &HashMap<PadRef, NetId>,
    kept: &HashSet<PadRef>,
) -> Numbered {
    let matched = match_existing(netlist, board, assigned);

    let copper = copper_numbers(board);
    let mut in_use = copper.clone();
    let mut kept_pad_nets: HashSet<u32> = HashSet::new();
    for fp in &board.footprints {
        for pad in &fp.pads {
            if let Some(n) = &pad.net
                && kept.contains(&(fp.uuid, pad.uuid))
            {
                in_use.insert(n.number);
                kept_pad_nets.insert(n.number);
            }
        }
    }
    let claimed: HashSet<u32> = matched.values().copied().collect();
    let retained: Vec<&NetDef> = board
        .nets
        .iter()
        .filter(|d| d.number != 0 && !claimed.contains(&d.number) && in_use.contains(&d.number))
        .collect();

    // A new net never takes a number the board's table holds now: one
    // dropped and reused in the same sync would read as a rename.
    let mut reserved: HashSet<u32> = claimed;
    reserved.extend(retained.iter().map(|d| d.number));
    reserved.extend(in_use);
    reserved.extend(board.nets.iter().map(|d| d.number));
    let mut next = reserved
        .iter()
        .copied()
        .chain(board.nets.iter().map(|d| d.number))
        .max()
        .unwrap_or(0);

    let old_name: HashMap<u32, &str> = board
        .nets
        .iter()
        .map(|d| (d.number, d.name.as_str()))
        .collect();
    let mut out = Numbered {
        table: Vec::new(),
        number_of: HashMap::new(),
        name_of: HashMap::new(),
        renamed: HashMap::new(),
        copper_only: retained
            .iter()
            .map(|d| d.number)
            .filter(|n| copper.contains(n) && !kept_pad_nets.contains(n))
            .collect(),
    };
    for net in &netlist.nets {
        let (number, name) = match matched.get(&net.id) {
            Some(&number) => {
                let old = old_name.get(&number).copied().unwrap_or_default();
                let keep_old = is_auto_name(&net.name, net.id) && !old.is_empty();
                let name = if keep_old { old } else { net.name.as_str() };
                if name != old {
                    out.renamed.insert(number, name.to_string());
                }
                (number, name)
            }
            None => {
                let number = if net.id.0 != 0 && !reserved.contains(&net.id.0) {
                    net.id.0
                } else {
                    next += 1;
                    while reserved.contains(&next) {
                        next += 1;
                    }
                    next
                };
                reserved.insert(number);
                (number, net.name.as_str())
            }
        };
        out.table.push(NetDef {
            number,
            name: name.to_string(),
        });
        out.number_of.insert(net.id, number);
        out.name_of.insert(net.id, name.to_string());
    }
    out.table.extend(retained.into_iter().cloned());
    out
}

/// The unlabelled-net name the netlist generates from its id.
fn is_auto_name(name: &str, id: NetId) -> bool {
    name == format!("N${}", id.0)
}

/// Net numbers that tracks, vias, zones and the sketch's plated holes point
/// at.
fn copper_numbers(board: &PcbBoard) -> HashSet<u32> {
    let mut used: HashSet<u32> = board.segments.iter().map(|s| s.net).collect();
    used.extend(board.vias.iter().map(|v| v.net));
    used.extend(board.zones.iter().map(|z| z.net));
    if let Some(sketch) = &board.sketch {
        used.extend(sketch.holes.iter().filter_map(|h| h.net));
    }
    used.remove(&0);
    used
}

/// Pair netlist nets with existing board nets: by shared pads (most shared
/// first), then by real label for nets that hold no pads yet.
fn match_existing(
    netlist: &Netlist,
    board: &PcbBoard,
    assigned: &HashMap<PadRef, NetId>,
) -> HashMap<NetId, u32> {
    let mut old_pads: HashMap<u32, HashSet<PadRef>> = HashMap::new();
    for fp in &board.footprints {
        for pad in &fp.pads {
            if let Some(n) = &pad.net
                && n.number != 0
            {
                old_pads
                    .entry(n.number)
                    .or_default()
                    .insert((fp.uuid, pad.uuid));
            }
        }
    }
    let mut new_pads: HashMap<NetId, Vec<PadRef>> = HashMap::new();
    for (&pad, &net) in assigned {
        new_pads.entry(net).or_default().push(pad);
    }

    let mut overlaps: Vec<(usize, usize, u32)> = Vec::new();
    for (i, net) in netlist.nets.iter().enumerate() {
        let Some(pads) = new_pads.get(&net.id) else {
            continue;
        };
        for (&number, old) in &old_pads {
            let shared = pads.iter().filter(|p| old.contains(p)).count();
            if shared > 0 {
                overlaps.push((shared, i, number));
            }
        }
    }
    overlaps.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));

    let mut matched: HashMap<NetId, u32> = HashMap::new();
    let mut claimed: HashSet<u32> = HashSet::new();
    for (_, i, number) in overlaps {
        let id = netlist.nets[i].id;
        if !matched.contains_key(&id) && claimed.insert(number) {
            matched.insert(id, number);
        }
    }
    for net in &netlist.nets {
        if matched.contains_key(&net.id) || is_auto_name(&net.name, net.id) {
            continue;
        }
        let same_name = board
            .nets
            .iter()
            .find(|d| d.number != 0 && d.name == net.name && !claimed.contains(&d.number));
        if let Some(def) = same_name {
            claimed.insert(def.number);
            matched.insert(net.id, def.number);
        }
    }
    matched
}
