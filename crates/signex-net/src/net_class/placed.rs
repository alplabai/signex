//! Class styles and net names per sheet placement. A child sheet placed twice
//! shares its wire uuids between both placements while each placement's nets
//! are different ones, so what a canvas draws and a picker names depends on
//! the placement the sheet is shown through.

use std::collections::HashMap;

use signex_types::net_class::{NetClass, NetClassStyle, first_matching_class};
use signex_types::schematic::InstancePath;
use uuid::Uuid;

use crate::project::{NetIndex, NetMemberKind};

/// The class style of every wire and junction, per placement. Only elements
/// whose net's class carries a non-default style appear.
pub fn placement_class_styles(
    index: &NetIndex,
    classes: &[NetClass],
) -> HashMap<InstancePath, HashMap<Uuid, NetClassStyle>> {
    let mut out: HashMap<InstancePath, HashMap<Uuid, NetClassStyle>> = HashMap::new();
    for (name, placement, id) in drawn_members(index) {
        let Some(class) = first_matching_class(classes, name) else {
            continue;
        };
        if !class.style.is_default() {
            out.entry(placement.clone())
                .or_default()
                .insert(id, class.style);
        }
    }
    out
}

/// The net name of every wire and junction, per placement.
pub fn placement_net_names(index: &NetIndex) -> HashMap<InstancePath, HashMap<Uuid, String>> {
    let mut out: HashMap<InstancePath, HashMap<Uuid, String>> = HashMap::new();
    for (name, placement, id) in drawn_members(index) {
        out.entry(placement.clone())
            .or_default()
            .insert(id, name.to_string());
    }
    out
}

/// Each named net's wires and junctions as `(net name, placement, uuid)`.
fn drawn_members(index: &NetIndex) -> impl Iterator<Item = (&str, &InstancePath, Uuid)> {
    (0..index.net_count())
        .filter_map(|i| index.net(i))
        .filter(|net| !net.name.is_empty())
        .flat_map(|net| {
            net.members
                .iter()
                .filter(|m| matches!(m.kind, NetMemberKind::Wire | NetMemberKind::Junction))
                .map(|m| (net.name.as_str(), &m.occurrence, m.id))
        })
}
