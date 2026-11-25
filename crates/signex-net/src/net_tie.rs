//! Net ties (#1277; Caner, 2026-11-25): a part marked `net_tie` joins two
//! named nets on purpose. The netlist keeps both nets; this module records
//! which nets each tie joins, read off the netlist, so ERC, the netlist
//! export and the board read one answer. See `docs/NET_TIES.md`.

use std::collections::HashMap;

use signex_types::net::Netlist;
use signex_types::schematic::{InstancePath, SchematicSheet};
use uuid::Uuid;

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
        let mut nets: Vec<&str> = self.pins.iter().map(|p| p.net.as_str()).collect();
        nets.sort_unstable();
        nets.dedup();
        nets
    }

    pub fn status(&self) -> NetTieStatus {
        match (self.pins.len(), self.nets().len()) {
            (0 | 1, _) => NetTieStatus::Open,
            (_, 1) => NetTieStatus::OneNet,
            _ => NetTieStatus::Joins,
        }
    }
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
        tie.pins
            .sort_by(|a, b| a.pin.cmp(&b.pin).then_with(|| a.net.cmp(&b.net)));
    }
    ties
}

#[cfg(test)]
#[path = "net_tie_tests.rs"]
mod tests;
