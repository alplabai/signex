//! Schematic netlist → board nets: fills `PcbBoard.nets` and each pad's
//! `PadNet` from the authoritative [`Netlist`] (ADR-0002 D7.3).
//!
//! A pad links to a schematic pin by (sheet path, part, pad number), where
//! the part is named by the footprint's stored link and lists every unit of a
//! multi-unit part ([`SymbolPadKey`]). Sync never reads a designator. A
//! footprint with no stored link is reported
//! ([`PadNetIssue::FootprintNotLinked`]) and left completely untouched;
//! [`link_footprints_by_designator`] links old boards once, on request. See
//! ADR-0005 section 11.
//!
//! The schematic is the source of truth for pads it links to, except a pad
//! whose net is locked: that pad keeps its net number (a rename of that
//! number follows) and is reported. On a footprint that is in the schematic,
//! a pad with no pad number, or one that matches no pin of its part, keeps
//! its net and becomes locked, so the lock persists. A footprint whose link
//! matches nothing in the schematic is reported and its pads are left as
//! they are, not locked. Ambiguous pads are left as they are and reported.
//! Existing board net numbers are kept, so tracks, vias and zones keep their
//! net. Assignment is pure; it returns a [`BoardNetEdit`] whose `apply` hands
//! back the inverse.

mod assign;
mod edit;
mod key;
mod link_by_designator;
mod numbering;
#[cfg(test)]
mod tests;

pub use assign::{assign_pad_net, set_pad_net_lock};
pub use edit::{BoardNetEdit, PadNetChange, ZoneNetName};
pub use key::{PadLinkKey, SymbolPad, SymbolPadKey};
pub use link_by_designator::{
    BoardLinkEdit, FootprintLinkChange, LinkByDesignatorReport, link_footprints_by_designator,
};

use std::collections::{HashMap, HashSet};

use numbering::{PadRef, number_nets};
use signex_types::net::{NetId, Netlist, Terminal};
use signex_types::pcb::{PadNet, PcbBoard};
use uuid::Uuid;

/// Why a terminal or pad was left without a net.
#[derive(Debug, Clone, PartialEq)]
pub enum PadNetIssue {
    /// The terminal has no link key, or no pad on the board has its key.
    TerminalWithoutPad { net: NetId, terminal: Terminal },
    /// The pad links to terminals on more than one net.
    PadOnSeveralNets {
        footprint: Uuid,
        pad: Uuid,
        nets: Vec<NetId>,
    },
    /// The terminal's key matches pads on more than one footprint (e.g. two
    /// footprints share a reference), so none of them is assigned.
    KeyOnSeveralFootprints {
        net: NetId,
        terminal: Terminal,
        footprints: Vec<Uuid>,
    },
    /// No pad of this footprint matches a schematic terminal (its linked
    /// symbol is not in the schematic); its pads keep their nets and are not
    /// locked.
    FootprintNotInSchematic { footprint: Uuid },
    /// The footprint has no stored link to a schematic part (placed by hand,
    /// or on a board made before the link). Sync leaves it, and all its pads,
    /// untouched.
    FootprintNotLinked { footprint: Uuid },
    /// The pad has no pad number; it keeps its net, and locks when its
    /// footprint is in the schematic.
    PadWithoutTerminal { footprint: Uuid, pad: Uuid },
    /// The footprint is linked to a symbol, but no pin of that symbol has
    /// the pad's number (shield, mounting or exposed pad); it keeps its net
    /// and locks.
    PadNotOnSymbol { footprint: Uuid, pad: Uuid },
    /// No pad holds this board net any more, but tracks, vias or zones still
    /// use it; they stay on a net with no pads.
    NetKeptForCopper { number: u32 },
    /// The pad's net is locked: the sync leaves it alone, even when a
    /// schematic pin matches it. Informational.
    PadNetLocked { footprint: Uuid, pad: Uuid },
}

/// The edit that brings the board in line with the netlist, and what could
/// not be linked.
#[derive(Debug, Clone)]
pub struct BoardNetAssignment {
    pub edit: BoardNetEdit,
    pub issues: Vec<PadNetIssue>,
}

/// The terminals that share one link key, in netlist order.
struct KeyedTerminals<'a> {
    nets: Vec<NetId>,
    first: (NetId, &'a Terminal),
}

/// Assign every net of `netlist` to the board pads its terminals link to
/// under `key`. The board is only read; apply the returned edit to change it.
///
/// A net matches the board net that holds most of its pads and keeps that
/// number; new nets get fresh ones. `0` stays free for "no net" on disk.
pub fn assign_pad_nets<K: PadLinkKey>(
    netlist: &Netlist,
    board: &PcbBoard,
    key: &K,
) -> BoardNetAssignment {
    let mut issues = Vec::new();
    let (order, keyed) = key_terminals(netlist, key, &mut issues);

    // Every pad with a key is grouped, locked ones included: a link shared
    // by several footprints is a collision whether or not a pad is locked.
    let mut pads_of: HashMap<K::Key, Vec<PadRef>> = HashMap::new();
    let mut kept: HashSet<PadRef> = HashSet::new();
    for fp in &board.footprints {
        for pad in &fp.pads {
            if pad.net_locked {
                kept.insert((fp.uuid, pad.uuid));
                issues.push(PadNetIssue::PadNetLocked {
                    footprint: fp.uuid,
                    pad: pad.uuid,
                });
            }
            if let Some(k) = key.pad_key(fp, pad) {
                pads_of.entry(k).or_default().push((fp.uuid, pad.uuid));
            }
        }
    }

    let mut assigned: HashMap<PadRef, NetId> = HashMap::new();
    for k in &order {
        let group = &keyed[k];
        let (net, terminal) = group.first;
        let Some(pads) = pads_of.get(k) else {
            issues.push(PadNetIssue::TerminalWithoutPad {
                net,
                terminal: terminal.clone(),
            });
            continue;
        };
        let mut footprints: Vec<Uuid> = pads.iter().map(|(fp, _)| *fp).collect();
        footprints.sort_unstable();
        footprints.dedup();
        if footprints.len() > 1 {
            kept.extend(pads.iter().copied());
            issues.push(PadNetIssue::KeyOnSeveralFootprints {
                net,
                terminal: terminal.clone(),
                footprints,
            });
            continue;
        }
        let open: Vec<PadRef> = pads.iter().copied().filter(|p| !kept.contains(p)).collect();
        if group.nets.len() > 1 {
            kept.extend(open.iter().copied());
            for &(footprint, pad) in &open {
                issues.push(PadNetIssue::PadOnSeveralNets {
                    footprint,
                    pad,
                    nets: group.nets.clone(),
                });
            }
            continue;
        }
        for pad in open {
            assigned.insert(pad, net);
        }
    }
    let unspoken = leave_unlinked_pads(board, key, &keyed, &mut kept, &mut issues);

    let numbered = number_nets(netlist, board, &assigned, &kept);
    let mut orphaned = numbered.copper_only.clone();
    orphaned.sort_unstable();
    issues.extend(
        orphaned
            .into_iter()
            .map(|number| PadNetIssue::NetKeptForCopper { number }),
    );
    let target: HashMap<PadRef, PadNet> = assigned
        .iter()
        .map(|(&pad, net)| {
            let pad_net = PadNet {
                number: numbered.number_of[net],
                name: numbered.name_of[net].clone(),
            };
            (pad, pad_net)
        })
        .collect();

    BoardNetAssignment {
        edit: BoardNetEdit {
            nets: numbered.table,
            pads: pad_changes(board, &target, &kept, &unspoken, &numbered.renamed),
            zones: zone_renames(board, &numbered.renamed),
        },
        issues,
    }
}

/// Pads the schematic cannot speak for stay as they are, with an issue.
/// A footprint with no link is reported once and all its pads are left
/// alone. On a linked footprint, `unspoken` (returned, locked) are the pads
/// with no pad number and the pads matching no terminal, but only when the
/// footprint is in the schematic (some pad matches). A footprint none of
/// whose keys is a terminal is reported once as `FootprintNotInSchematic` and
/// its pads are left alone, not locked. A pad whose net is already locked is
/// reported once, by the caller.
fn leave_unlinked_pads<K: PadLinkKey>(
    board: &PcbBoard,
    key: &K,
    keyed: &HashMap<K::Key, KeyedTerminals<'_>>,
    kept: &mut HashSet<PadRef>,
    issues: &mut Vec<PadNetIssue>,
) -> HashSet<PadRef> {
    let mut unspoken = HashSet::new();
    for fp in &board.footprints {
        if key.footprint_unlinked(fp) {
            issues.push(PadNetIssue::FootprintNotLinked { footprint: fp.uuid });
            kept.extend(fp.pads.iter().map(|p| (fp.uuid, p.uuid)));
            continue;
        }
        let keys: Vec<Option<K::Key>> = fp.pads.iter().map(|p| key.pad_key(fp, p)).collect();
        let in_schematic = keys.iter().flatten().any(|k| keyed.contains_key(k));
        if !in_schematic && keys.iter().any(Option::is_some) {
            issues.push(PadNetIssue::FootprintNotInSchematic { footprint: fp.uuid });
        }
        for (pad, k) in fp.pads.iter().zip(&keys) {
            if pad.net_locked {
                continue;
            }
            let at = (fp.uuid, pad.uuid);
            let off_symbol = match k {
                None => {
                    issues.push(PadNetIssue::PadWithoutTerminal {
                        footprint: fp.uuid,
                        pad: pad.uuid,
                    });
                    true
                }
                Some(k) if in_schematic && !keyed.contains_key(k) => {
                    issues.push(PadNetIssue::PadNotOnSymbol {
                        footprint: fp.uuid,
                        pad: pad.uuid,
                    });
                    true
                }
                Some(_) => false,
            };
            if !in_schematic {
                kept.insert(at);
            } else if off_symbol {
                kept.insert(at);
                unspoken.insert(at);
            }
        }
    }
    unspoken
}

/// Group terminals by link key, keeping the first-seen key order so the
/// result does not depend on hash order.
fn key_terminals<'a, K: PadLinkKey>(
    netlist: &'a Netlist,
    key: &K,
    issues: &mut Vec<PadNetIssue>,
) -> (Vec<K::Key>, HashMap<K::Key, KeyedTerminals<'a>>) {
    let mut order = Vec::new();
    let mut keyed: HashMap<K::Key, KeyedTerminals<'a>> = HashMap::new();
    for net in &netlist.nets {
        for terminal in &net.terminals {
            let Some(k) = key.terminal_key(terminal) else {
                issues.push(PadNetIssue::TerminalWithoutPad {
                    net: net.id,
                    terminal: terminal.clone(),
                });
                continue;
            };
            match keyed.get_mut(&k) {
                Some(group) => {
                    if !group.nets.contains(&net.id) {
                        group.nets.push(net.id);
                    }
                }
                None => {
                    order.push(k.clone());
                    keyed.insert(
                        k,
                        KeyedTerminals {
                            nets: vec![net.id],
                            first: (net.id, terminal),
                        },
                    );
                }
            }
        }
    }
    (order, keyed)
}

/// Zones on a renamed net take its new name.
fn zone_renames(board: &PcbBoard, renamed: &HashMap<u32, String>) -> Vec<ZoneNetName> {
    board
        .zones
        .iter()
        .filter_map(|z| {
            let name = renamed.get(&z.net)?;
            (z.net_name != *name).then(|| ZoneNetName {
                zone: z.uuid,
                name: name.clone(),
            })
        })
        .collect()
}

/// Every pad whose net or lock changes, in board order. A kept pad only
/// takes a renamed net's new name; a pad the schematic cannot speak for
/// (`unspoken`) that holds a net also becomes locked. A locked pad keeps its
/// lock and net, bar the rename that keeps it in step with the net table.
fn pad_changes(
    board: &PcbBoard,
    target: &HashMap<PadRef, PadNet>,
    kept: &HashSet<PadRef>,
    unspoken: &HashSet<PadRef>,
    renamed: &HashMap<u32, String>,
) -> Vec<PadNetChange> {
    let mut changes = Vec::new();
    for fp in &board.footprints {
        for pad in &fp.pads {
            let at = (fp.uuid, pad.uuid);
            if kept.contains(&at) {
                let net = pad.net.as_ref().map(|net| match renamed.get(&net.number) {
                    Some(name) if net.name != *name => PadNet {
                        number: net.number,
                        name: name.clone(),
                    },
                    _ => net.clone(),
                });
                let locked = pad.net_locked || (unspoken.contains(&at) && pad.net.is_some());
                if net != pad.net || locked != pad.net_locked {
                    changes.push(PadNetChange {
                        footprint: fp.uuid,
                        pad: pad.uuid,
                        net,
                        locked,
                    });
                }
                continue;
            }
            let wanted = target.get(&at);
            if pad.net.as_ref() != wanted {
                changes.push(PadNetChange {
                    footprint: fp.uuid,
                    pad: pad.uuid,
                    net: wanted.cloned(),
                    locked: false,
                });
            }
        }
    }
    changes
}
