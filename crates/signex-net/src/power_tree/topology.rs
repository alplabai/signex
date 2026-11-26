//! What the power tree is built from (#1313): the project's power nets and
//! every fitted part that touches one, read off the stitched project netlist
//! with its parameters. Nothing here re-derives a net.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use signex_types::designator::compare_references;
use signex_types::project::ChannelNaming;
use signex_types::schematic::{InstancePath, Symbol};
use uuid::Uuid;

use super::quantity::{is_ground_net, rail_parameter};
use crate::{
    NetIndex, NetMemberKind, PlacedSymbol, ProjectGraph, SheetKey,
    build_project_netlist_indexed_named,
};

/// The power nets of a project and the parts on them.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PowerTopology {
    /// Every rail by net name: a net named by a power port or a Power label,
    /// a name the caller lists as a rail, and a name a fitted part's rail
    /// parameter gives. Grounds are left out.
    pub rails: BTreeSet<String>,
    /// The power nets that are grounds (`is_ground_net`), by name.
    pub grounds: BTreeSet<String>,
    /// Every fitted part with a pin on a rail or a rail parameter, in
    /// designator order.
    pub parts: Vec<PowerPart>,
}

/// One fitted part on one sheet occurrence that touches a rail.
#[derive(Debug, Clone, PartialEq)]
pub struct PowerPart {
    pub designator: String,
    pub value: String,
    /// The sheet the part is placed on, by key.
    pub sheet: SheetKey,
    /// That sheet's document id (its `schematic_id`).
    pub sheet_id: Uuid,
    pub occurrence: InstancePath,
    /// The part's placed units in placement order; the first is where a
    /// click on the part goes.
    pub units: Vec<Uuid>,
    /// The rails its pins are on, by net name.
    pub pins_on: BTreeSet<String>,
    /// The fields of its units, the first unit's value winning.
    pub parameters: BTreeMap<String, String>,
}

impl PowerPart {
    /// Whether one of the part's parameters names a rail.
    pub fn has_rail_parameter(&self) -> bool {
        self.parameters
            .keys()
            .any(|key| rail_parameter(key).is_some())
    }
}

/// The units of one part: its sheet, its occurrence, and its designator
/// (or, unannotated, its own symbol).
type PartKey = (SheetKey, InstancePath, String);

/// The power topology of `graph`. `listed_rails` are the rails the project
/// names on its own (its rail list), which count as rails even where no
/// power port names them. `is_fitted` says whether the part made of the
/// given units is fitted in the variant the caller reads.
pub fn power_topology(
    graph: &ProjectGraph<'_>,
    naming: &ChannelNaming,
    listed_rails: &BTreeSet<String>,
    is_fitted: &dyn Fn(&[&Symbol]) -> bool,
) -> PowerTopology {
    let (_, index) = build_project_netlist_indexed_named(graph, naming);
    let mut parts = fitted_parts(graph, &index, is_fitted);
    let mut rails: BTreeSet<String> = listed_rails
        .iter()
        .filter(|name| !is_ground_net(name))
        .cloned()
        .collect();
    for part in &parts {
        let named = part.parameters.keys().filter_map(|key| rail_parameter(key));
        rails.extend(
            named
                .filter(|(net, _)| !is_ground_net(net))
                .map(|(net, _)| net.to_string()),
        );
    }
    let by_unit = unit_parts(&parts);
    let mut grounds = BTreeSet::new();
    for i in 0..index.net_count() {
        let Some(net) = index.net(i) else {
            continue;
        };
        if net.net.is_none() {
            continue;
        }
        let is_power = net.members.iter().any(|m| m.names_power_net());
        if is_ground_net(&net.name) {
            if is_power {
                grounds.insert(net.name.clone());
            }
            continue;
        }
        if !is_power && !rails.contains(&net.name) {
            continue;
        }
        rails.insert(net.name.clone());
        for member in &net.members {
            if !matches!(member.kind, NetMemberKind::Pin { .. }) {
                continue;
            }
            let key = (member.sheet.clone(), member.occurrence.clone(), member.id);
            if let Some(&at) = by_unit.get(&key) {
                parts[at].pins_on.insert(net.name.clone());
            }
        }
    }
    parts.retain(|part| !part.pins_on.is_empty() || part.has_rail_parameter());
    PowerTopology {
        rails,
        grounds,
        parts,
    }
}

/// Each unit of each part, by `(sheet, occurrence, symbol uuid)`, to the
/// part's place in `parts`.
fn unit_parts(parts: &[PowerPart]) -> HashMap<(SheetKey, InstancePath, Uuid), usize> {
    let mut out = HashMap::new();
    for (at, part) in parts.iter().enumerate() {
        for unit in &part.units {
            out.insert((part.sheet.clone(), part.occurrence.clone(), *unit), at);
        }
    }
    out
}

/// Every fitted part of the project: its units grouped by sheet occurrence
/// and designator. A unit that is not annotated is a part of its own.
fn fitted_parts(
    graph: &ProjectGraph<'_>,
    index: &NetIndex,
    is_fitted: &dyn Fn(&[&Symbol]) -> bool,
) -> Vec<PowerPart> {
    let mut grouped: BTreeMap<PartKey, Vec<&PlacedSymbol>> = BTreeMap::new();
    for placed in index.placed_symbols() {
        let name = if placed.is_annotated() {
            placed.designator.clone()
        } else {
            format!("\u{1}{}", placed.id)
        };
        let key = (placed.sheet.clone(), placed.occurrence.clone(), name);
        grouped.entry(key).or_default().push(placed);
    }
    let mut parts = Vec::new();
    for ((sheet_key, occurrence, _), placed) in grouped {
        let Some(sheet) = graph.sheets.get(&sheet_key) else {
            continue;
        };
        let units: Vec<&Symbol> = placed
            .iter()
            .filter_map(|p| sheet.symbols.iter().find(|s| s.uuid == p.id))
            .filter(|s| !s.is_power)
            .collect();
        if units.is_empty() || !is_fitted(&units) {
            continue;
        }
        let mut parameters = BTreeMap::new();
        for unit in &units {
            for (key, value) in &unit.fields {
                parameters
                    .entry(key.clone())
                    .or_insert_with(|| value.clone());
            }
        }
        parts.push(PowerPart {
            designator: placed[0].designator.clone(),
            value: placed[0].value.clone(),
            sheet: sheet_key,
            sheet_id: sheet.uuid,
            occurrence,
            units: units.iter().map(|unit| unit.uuid).collect(),
            pins_on: BTreeSet::new(),
            parameters,
        });
    }
    parts.sort_by(|a, b| compare_references(&a.designator, &b.designator));
    parts
}
