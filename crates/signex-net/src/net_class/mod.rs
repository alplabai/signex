//! Net → net class resolution.
//!
//! The project's net-class table ([`NetClass`]) is keyed by net *name*; the
//! [`Netlist`] knows which wires and junctions make up each named net. This
//! module joins the two, so a class follows its net across every wire (and,
//! with a project netlist, every sheet) that carries the name.
//!
//! Precedence is the table's: the first class that contains a net's name wins
//! ([`signex_types::net_class::first_matching_class`]).

use std::collections::HashMap;

mod net_names;
pub use net_names::{element_net_names, wire_at_point};

use signex_types::net::{NetClassId, Netlist};
use signex_types::net_class::{NetClass, NetClassStyle, first_matching_class};
use uuid::Uuid;

/// Set [`Net::class`](signex_types::net::Net::class) on every net that a class
/// contains, and clear it on the rest, so the field always reflects `classes`.
pub fn assign_net_classes(netlist: &mut Netlist, classes: &[NetClass]) {
    for net in &mut netlist.nets {
        net.class = first_matching_class(classes, &net.name).map(|c| NetClassId(c.name.clone()));
    }
}

/// The class style to draw each wire and junction with, keyed by element uuid.
///
/// Only elements whose class carries a non-default style appear, so a project
/// without classes (or with style-less ones) yields an empty map and the
/// renderers take their unchanged path. If one element sits on several nets
/// (a sheet instantiated twice), the net with the lowest id wins.
pub fn element_class_styles(
    netlist: &Netlist,
    classes: &[NetClass],
) -> HashMap<Uuid, NetClassStyle> {
    let mut out = HashMap::new();
    if classes.is_empty() {
        return out;
    }
    for net in &netlist.nets {
        let Some(class) = first_matching_class(classes, &net.name) else {
            continue;
        };
        if class.style.is_default() {
            continue;
        }
        for id in net.wires.iter().chain(&net.junctions) {
            out.entry(*id).or_insert(class.style);
        }
    }
    out
}

#[cfg(test)]
mod tests;
