//! What each net on a sheet is made of, for the net completeness rules
//! (#1286). Membership comes from the shared `merged_connectivity` (wires,
//! junctions, same-name labels, power ports), the one `summarize_nets` uses,
//! so these rules and the netlist agree on what a net is.

use std::collections::{BTreeMap, HashSet};

use signex_net::pt_key;
use signex_types::schematic::LabelType;

use crate::context::{ErcContext, ErcLabel, ErcPin, ErcSymbol, merged_connectivity};

const UNNAMED: &str = "(unnamed)";

/// The members of one net on this sheet.
#[derive(Default)]
pub(super) struct NetMembers<'a> {
    /// Pins of ordinary parts, in symbol order. Pins on a no-connect marker
    /// are left out: they are isolated by design.
    pub pins: Vec<(&'a ErcSymbol, &'a ErcPin)>,
    pub labels: Vec<&'a ErcLabel>,
    /// Power ports and power flags with a tip on the net (a port with no
    /// pins connects at its origin, as in `power_port_short`).
    pub power_ports: Vec<&'a ErcSymbol>,
    pub wire_count: usize,
    /// A sheet pin, a bus entry end or a Global / Hierarchical / Power label
    /// is on the net, so it continues on another sheet or bus that this
    /// sheet's run cannot see.
    pub leaves_sheet: bool,
}

impl NetMembers<'_> {
    /// The net as a finding carries it: its name and every part pin.
    pub fn finding(&self) -> crate::NetFinding {
        let name = self.name();
        let name = if name == UNNAMED { String::new() } else { name };
        let pins = self
            .pins
            .iter()
            .map(|(s, p)| crate::NetFindingPin {
                symbol: s.uuid,
                reference: s.reference.clone(),
                number: p.number.clone(),
                position: p.world_pos,
                direction: p.electrical_type,
            })
            .collect();
        crate::NetFinding::from_pins(name, pins)
    }

    /// Whether a local (Net) label names the net.
    pub fn has_local_label(&self) -> bool {
        self.labels.iter().any(|l| l.label_type == LabelType::Net)
    }

    /// The name the netlist gives the net, or `(unnamed)`.
    pub fn name(&self) -> String {
        let power: Vec<&str> = self
            .power_ports
            .iter()
            .filter(|s| !signex_net::is_power_flag(&s.lib_id))
            .map(|s| s.value.as_str())
            .collect();
        signex_net::best_net_name_of(
            self.labels.iter().map(|l| (l.label_type, l.text.as_str())),
            &power,
        )
        .map(|(_, name)| name)
        .unwrap_or_else(|| UNNAMED.to_string())
    }
}

/// Every net on the sheet that has at least one member, ordered by root.
pub(super) fn net_members(ctx: &ErcContext) -> Vec<NetMembers<'_>> {
    let (mut conn, _) = merged_connectivity(&ctx.wires, &ctx.labels, &ctx.junctions, &ctx.symbols);
    let no_connects: HashSet<(i64, i64)> = ctx
        .no_connects
        .iter()
        .map(|nc| pt_key(&nc.position))
        .collect();
    let mut nets: BTreeMap<(i64, i64), NetMembers> = BTreeMap::new();

    for symbol in ctx.symbols.iter().filter(|s| s.is_power) {
        for tip in super::port_tips(symbol) {
            let net = nets.entry(conn.root_of(&tip)).or_default();
            if !net.power_ports.iter().any(|s| s.uuid == symbol.uuid) {
                net.power_ports.push(symbol);
            }
        }
    }
    for symbol in ctx.symbols.iter().filter(|s| !s.is_power) {
        for pin in symbol.pins.iter().filter(|p| p.connected) {
            if !no_connects.contains(&pt_key(&pin.world_pos)) {
                let net = nets.entry(conn.root_of(&pin.world_pos)).or_default();
                net.pins.push((symbol, pin));
            }
        }
    }
    for label in &ctx.labels {
        let net = nets.entry(conn.root_of(&label.position)).or_default();
        net.labels.push(label);
        if !matches!(label.label_type, LabelType::Net) {
            net.leaves_sheet = true;
        }
    }
    for wire in &ctx.wires {
        nets.entry(conn.root_of(&wire.start))
            .or_default()
            .wire_count += 1;
    }
    let sheet_pins = ctx
        .child_sheets
        .iter()
        .flat_map(|c| c.pins.iter().map(|p| p.position));
    let entry_ends = ctx.bus_entries.iter().flat_map(|be| {
        let far = signex_types::schematic::Point::new(
            be.position.x + be.size.0,
            be.position.y + be.size.1,
        );
        [be.position, far]
    });
    for at in sheet_pins.chain(entry_ends) {
        if let Some(net) = nets.get_mut(&conn.root_of(&at)) {
            net.leaves_sheet = true;
        }
    }
    nets.into_values().collect()
}
