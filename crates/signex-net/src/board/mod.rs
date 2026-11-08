//! Schematic netlist → board nets: fills `PcbBoard.nets` and each pad's
//! `PadNet` from the authoritative [`Netlist`] (ADR-0002 D7.3).
//!
//! The schematic is the source of truth for pads it links to. Pads it cannot
//! speak for (no link key, or a footprint it does not have) and ambiguous
//! pads are left as they are and reported. Existing board net numbers are
//! kept, so tracks, vias and zones keep their net. Assignment is pure; it
//! returns a [`BoardNetEdit`] whose `apply` hands back the inverse edit.

mod edit;
mod key;
mod numbering;
#[cfg(test)]
mod tests;

pub use edit::{BoardNetEdit, PadNetChange};
pub use key::{PadLinkKey, ReferencePad, ReferencePadKey};

use std::collections::{HashMap, HashSet};

use signex_types::net::{NetId, Netlist, Terminal};
use numbering::{PadRef, number_nets};
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
    /// No pad of this footprint matches a schematic terminal; its pads keep
    /// their nets.
    FootprintNotInSchematic { footprint: Uuid },
    /// The pad has no link key (unnumbered, or its footprint has no
    /// reference); it keeps its net.
    PadWithoutTerminal { footprint: Uuid, pad: Uuid },
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

    let mut pads_of: HashMap<K::Key, Vec<PadRef>> = HashMap::new();
    for fp in &board.footprints {
        for pad in &fp.pads {
            if let Some(k) = key.pad_key(fp, pad) {
                pads_of.entry(k).or_default().push((fp.uuid, pad.uuid));
            }
        }
    }

    let mut assigned: HashMap<PadRef, NetId> = HashMap::new();
    let mut kept: HashSet<PadRef> = HashSet::new();
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
        if group.nets.len() > 1 {
            kept.extend(pads.iter().copied());
            for &(footprint, pad) in pads {
                issues.push(PadNetIssue::PadOnSeveralNets {
                    footprint,
                    pad,
                    nets: group.nets.clone(),
                });
            }
            continue;
        }
        for &pad in pads {
            assigned.insert(pad, net);
        }
    }
    leave_unlinked_pads(board, key, &keyed, &mut kept, &mut issues);

    let numbered = number_nets(netlist, board, &assigned, &kept);
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
            pads: pad_changes(board, &target, &kept),
        },
        issues,
    }
}

/// Pads the schematic cannot speak for stay as they are, with an issue:
/// pads with no link key, and every pad of a footprint none of whose keys is
/// a schematic terminal.
fn leave_unlinked_pads<K: PadLinkKey>(
    board: &PcbBoard,
    key: &K,
    keyed: &HashMap<K::Key, KeyedTerminals<'_>>,
    kept: &mut HashSet<PadRef>,
    issues: &mut Vec<PadNetIssue>,
) {
    for fp in &board.footprints {
        let keys: Vec<Option<K::Key>> = fp.pads.iter().map(|p| key.pad_key(fp, p)).collect();
        let linked = keys.iter().flatten().any(|k| keyed.contains_key(k));
        let has_key = keys.iter().any(Option::is_some);
        if !linked && has_key {
            issues.push(PadNetIssue::FootprintNotInSchematic { footprint: fp.uuid });
        }
        for (pad, k) in fp.pads.iter().zip(&keys) {
            if k.is_none() {
                issues.push(PadNetIssue::PadWithoutTerminal {
                    footprint: fp.uuid,
                    pad: pad.uuid,
                });
            }
            if !linked || k.is_none() {
                kept.insert((fp.uuid, pad.uuid));
            }
        }
    }
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

/// Every pad outside `kept` whose net differs from `target` (absent = no
/// net), in board order.
fn pad_changes(
    board: &PcbBoard,
    target: &HashMap<PadRef, PadNet>,
    kept: &HashSet<PadRef>,
) -> Vec<PadNetChange> {
    let mut changes = Vec::new();
    for fp in &board.footprints {
        for pad in &fp.pads {
            if kept.contains(&(fp.uuid, pad.uuid)) {
                continue;
            }
            let wanted = target.get(&(fp.uuid, pad.uuid));
            if pad.net.as_ref() != wanted {
                changes.push(PadNetChange {
                    footprint: fp.uuid,
                    pad: pad.uuid,
                    net: wanted.cloned(),
                });
            }
        }
    }
    changes
}
