//! Schematic netlist → board nets: fills `PcbBoard.nets` and each pad's
//! `PadNet` from the authoritative [`Netlist`] (ADR-0002 D7.3).
//!
//! The schematic is the source of truth: a pad no terminal links to ends up
//! with no net. Assignment is pure; it returns a [`BoardNetEdit`] whose
//! `apply` hands back the inverse edit.

mod edit;
mod key;
#[cfg(test)]
mod tests;

pub use edit::{BoardNetEdit, PadNetChange};
pub use key::{PadLinkKey, ReferencePad, ReferencePadKey};

use std::collections::HashMap;

use signex_types::net::{NetId, Netlist, Terminal};
use signex_types::pcb::{NetDef, PadNet, PcbBoard};
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
/// Net numbers are the netlist's [`NetId`]s, which start at 1, so `0` stays
/// free for "no net" on disk.
pub fn assign_pad_nets<K: PadLinkKey>(
    netlist: &Netlist,
    board: &PcbBoard,
    key: &K,
) -> BoardNetAssignment {
    let mut issues = Vec::new();
    let (order, keyed) = key_terminals(netlist, key, &mut issues);

    let mut pads_of: HashMap<K::Key, Vec<(Uuid, Uuid)>> = HashMap::new();
    for fp in &board.footprints {
        for pad in &fp.pads {
            if let Some(k) = key.pad_key(fp, pad) {
                pads_of.entry(k).or_default().push((fp.uuid, pad.uuid));
            }
        }
    }

    let names: HashMap<NetId, &str> = netlist
        .nets
        .iter()
        .map(|n| (n.id, n.name.as_str()))
        .collect();
    let mut target: HashMap<(Uuid, Uuid), PadNet> = HashMap::new();
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
        footprints.dedup();
        if footprints.len() > 1 {
            issues.push(PadNetIssue::KeyOnSeveralFootprints {
                net,
                terminal: terminal.clone(),
                footprints,
            });
            continue;
        }
        if group.nets.len() > 1 {
            for &(footprint, pad) in pads {
                issues.push(PadNetIssue::PadOnSeveralNets {
                    footprint,
                    pad,
                    nets: group.nets.clone(),
                });
            }
            continue;
        }
        let pad_net = PadNet {
            number: net.0,
            name: names.get(&net).copied().unwrap_or_default().to_string(),
        };
        for &pad in pads {
            target.insert(pad, pad_net.clone());
        }
    }

    BoardNetAssignment {
        edit: BoardNetEdit {
            nets: net_table(netlist),
            pads: pad_changes(board, &target),
        },
        issues,
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

fn net_table(netlist: &Netlist) -> Vec<NetDef> {
    netlist
        .nets
        .iter()
        .map(|n| NetDef {
            number: n.id.0,
            name: n.name.clone(),
        })
        .collect()
}

/// Every pad whose net differs from `target` (absent = no net), in board order.
fn pad_changes(board: &PcbBoard, target: &HashMap<(Uuid, Uuid), PadNet>) -> Vec<PadNetChange> {
    let mut changes = Vec::new();
    for fp in &board.footprints {
        for pad in &fp.pads {
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
