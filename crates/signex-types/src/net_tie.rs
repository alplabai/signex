//! Net ties (#1277; Caner, 2026-11-25): a part marked `net_tie` joins two
//! named nets on purpose. The netlist keeps both nets; [`net_ties`] reads
//! which nets each tie joins off the netlist, so the netlist export, the
//! command answers and the tests read one answer. It derives no
//! connectivity: `signex_net::build_netlist` does. On a board a tie is a
//! footprint of type Net Tie ([`board_net_ties`]). See `docs/NET_TIES.md`.

use std::collections::HashMap;

use uuid::Uuid;

use crate::net::Netlist;
use crate::pcb::{ComponentType, PcbBoard};
use crate::schematic::{InstancePath, SchematicSheet};

/// One pin of a net tie and the net it is on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetTiePin {
    pub pin: String,
    pub net: String,
}

/// What a tie does with its nets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetTieStatus {
    /// Its pins are on two or more nets: the intended short.
    Joins,
    /// Every pin is on one net: the tie joins nothing.
    OneNet,
    /// Fewer than two of its pins are on a net.
    Open,
}

impl NetTieStatus {
    /// The stable name a JSON answer and the netlist listing print.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Joins => "joins",
            Self::OneNet => "one_net",
            Self::Open => "open",
        }
    }
}

/// One placed net tie on one sheet occurrence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetTie {
    pub symbol: Uuid,
    pub occurrence: InstancePath,
    /// The designator its netlist terminals carry; the placed one when no
    /// pin is on a net.
    pub designator: String,
    /// Its pins that are on a net, by pin.
    pub pins: Vec<NetTiePin>,
}

impl NetTie {
    /// The distinct nets its pins are on, sorted.
    pub fn nets(&self) -> Vec<&str> {
        nets_of(&self.pins)
    }

    pub fn status(&self) -> NetTieStatus {
        status_of(&self.pins)
    }
}

/// One net-tie footprint on a board: its pads are the copper bridge DRC
/// allows (#1277).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoardNetTie {
    pub footprint: Uuid,
    pub designator: String,
    pub component_type: ComponentType,
    /// Its pads that are on a net, by pad number; `pin` is the pad number.
    pub pads: Vec<NetTiePin>,
}

impl BoardNetTie {
    /// The distinct nets its pads are on, sorted.
    pub fn nets(&self) -> Vec<&str> {
        nets_of(&self.pads)
    }

    pub fn status(&self) -> NetTieStatus {
        status_of(&self.pads)
    }
}

fn nets_of(pins: &[NetTiePin]) -> Vec<&str> {
    let mut nets: Vec<&str> = pins.iter().map(|p| p.net.as_str()).collect();
    nets.sort_unstable();
    nets.dedup();
    nets
}

fn status_of(pins: &[NetTiePin]) -> NetTieStatus {
    match (pins.len(), nets_of(pins).len()) {
        (0 | 1, _) => NetTieStatus::Open,
        (_, 1) => NetTieStatus::OneNet,
        _ => NetTieStatus::Joins,
    }
}

fn sort_pins(pins: &mut [NetTiePin]) {
    pins.sort_by(|a, b| a.pin.cmp(&b.pin).then_with(|| a.net.cmp(&b.net)));
}

/// Every net-tie footprint of `board` (type Net Tie or Net Tie (in BOM)), in
/// board order, with the nets of its pads. A pad of no net is left out.
pub fn board_net_ties(board: &PcbBoard) -> Vec<BoardNetTie> {
    board
        .footprints
        .iter()
        .filter(|f| f.component.component_type.is_net_tie())
        .map(|f| {
            let mut pads: Vec<NetTiePin> = f
                .pads
                .iter()
                .filter_map(|p| {
                    let net = p.net.as_ref().filter(|n| n.number != 0)?;
                    Some(NetTiePin {
                        pin: p.number.clone(),
                        net: net.name.clone(),
                    })
                })
                .collect();
            sort_pins(&mut pads);
            BoardNetTie {
                footprint: f.uuid,
                designator: f.reference.clone(),
                component_type: f.component.component_type,
                pads,
            }
        })
        .collect()
}

/// Every net tie of `sheets` (placed sheets with their occurrence paths), in
/// sheet then placement order, with the nets `netlist` puts its pins on. A
/// power port is never a tie.
pub fn net_ties(netlist: &Netlist, sheets: &[(InstancePath, &SchematicSheet)]) -> Vec<NetTie> {
    let mut ties: Vec<NetTie> = Vec::new();
    let mut slot: HashMap<(Uuid, InstancePath), usize> = HashMap::new();
    for (path, sheet) in sheets {
        for symbol in sheet.symbols.iter().filter(|s| s.net_tie && !s.is_power) {
            slot.insert((symbol.uuid, path.clone()), ties.len());
            ties.push(NetTie {
                symbol: symbol.uuid,
                occurrence: path.clone(),
                designator: symbol.designator_in(path).trim().to_string(),
                pins: Vec::new(),
            });
        }
    }
    for net in &netlist.nets {
        for terminal in &net.terminals {
            let key = (terminal.symbol, terminal.occurrence.clone());
            let Some(&i) = slot.get(&key) else {
                continue;
            };
            ties[i].designator.clone_from(&terminal.reference);
            ties[i].pins.push(NetTiePin {
                pin: terminal.pin.clone(),
                net: net.name.clone(),
            });
        }
    }
    for tie in &mut ties {
        sort_pins(&mut tie.pins);
    }
    ties
}

#[cfg(test)]
#[path = "net_tie_tests.rs"]
mod tests;
