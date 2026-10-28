//! Which net a wire, junction or label is on, by the name the project netlist
//! gives it. A net class is keyed by that name, so the name shown for a label
//! and the name a class stores for it must be this one, not the label's own
//! text: on a child sheet a local label `LOCAL` names the net `/LOCAL`, and a
//! higher-ranked label or power port on the same net can name it instead.

use std::collections::HashMap;

use signex_types::net::Netlist;
use signex_types::schematic::{Point, SchematicSheet, Wire};
use uuid::Uuid;

use crate::build::{anchor_choice, point_on_segment, pt_key};

/// The netlist's net name for every wire and junction in it. An element on
/// several nets (a sheet instantiated twice) takes the first net listed, the
/// one [`element_class_styles`](super::element_class_styles) also styles it by.
pub fn element_net_names(netlist: &Netlist) -> HashMap<Uuid, String> {
    let mut out = HashMap::new();
    for net in &netlist.nets {
        for id in net.wires.iter().chain(&net.junctions) {
            out.entry(*id).or_insert_with(|| net.name.clone());
        }
    }
    out
}

/// The wire a label (or any floating point) at `at` is attached to, by the
/// netlist's own anchoring: a wire that has `at` as an endpoint, else the one
/// interior tap chosen by the same total order the netlist uses. `None` when
/// the point touches no wire.
pub fn wire_at_point(sheet: &SchematicSheet, at: &Point) -> Option<Uuid> {
    let pk = pt_key(at);
    let ends = |w: &Wire| (pt_key(&w.start), pt_key(&w.end));
    if let Some(wire) = sheet.wires.iter().find(|w| {
        let (a, b) = ends(w);
        pk == a || pk == b
    }) {
        return Some(wire.uuid);
    }
    anchor_choice(sheet.wires.iter().filter_map(|w| {
        let (a, b) = ends(w);
        point_on_segment(pk, a, b).then_some((w.uuid, a, b))
    }))
}
