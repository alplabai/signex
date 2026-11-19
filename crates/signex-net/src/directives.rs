//! What the sheet's directives (#2420, #1281) mean for connectivity, derived
//! once here so the netlist, ERC and the board read the same answer:
//!
//! - a Compile Mask takes what it covers out of the netlist and ERC
//!   ([`compile_masked`]);
//! - a Net Blanket gives every net it catches its net class
//!   ([`blanket_nets`], [`blanket_net_classes`]);
//! - a Net Parameters directive gives the net it sits on a class, a
//!   diff-pair class and routing rules ([`net_parameters`]);
//! - a Differential Pair directive pairs the net it sits on with its
//!   partner ([`diff_pairs`], `diff_pair_classes.rs`).
//!
//! Class order: Net Parameters, then Net Blanket, then the project's class
//! patterns ([`directive_net_classes`], [`fill_pattern_classes`]).
//! Design note: `docs/SCHEMATIC_DIRECTIVES.md`.

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use signex_types::net::{Net, NetClassId, NetId, Netlist};
use signex_types::net_class::{NetClass, first_matching_class};
use signex_types::schematic::{Aabb, LabelType, NetRules, Point, SchDirective, SchematicSheet};
use uuid::Uuid;

use crate::net_class::wire_at_point;

pub use crate::diff_pair_classes::DiffPair;

/// `sheet` without what its Compile Masks cover; borrowed unchanged when it
/// has none. An object is covered when its anchor is inside a mask: a
/// symbol's origin, both ends of a wire or bus, the position of a label,
/// junction, no-connect, No-ERC directive or bus entry, and the whole box of
/// a sheet symbol.
pub fn compile_masked(sheet: &SchematicSheet) -> Cow<'_, SchematicSheet> {
    let masks: Vec<&SchDirective> = sheet
        .directives
        .iter()
        .filter(|d| matches!(d, SchDirective::CompileMask { .. }))
        .collect();
    if masks.is_empty() {
        return Cow::Borrowed(sheet);
    }
    let covered = |p: Point| masks.iter().any(|m| m.region_contains(p));
    let mut out = sheet.clone();
    out.symbols.retain(|s| !covered(s.position));
    out.wires.retain(|w| !(covered(w.start) && covered(w.end)));
    out.buses.retain(|b| !(covered(b.start) && covered(b.end)));
    out.labels.retain(|l| !covered(l.position));
    out.junctions.retain(|j| !covered(j.position));
    out.no_connects.retain(|n| !covered(n.position));
    out.no_erc_directives.retain(|n| !covered(n.position));
    out.bus_entries.retain(|e| !covered(e.position));
    out.child_sheets.retain(|c| {
        let far = Point::new(c.position.x + c.size.0, c.position.y + c.size.1);
        !(covered(c.position) && covered(far))
    });
    Cow::Owned(out)
}

/// How a Net Blanket holds a net.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlanketCatch {
    /// A wire lies wholly inside, or a junction does: caught.
    Inside,
    /// Its wires only cross the edge, and the blanket leaves such nets out.
    EdgeOnly,
    /// Its wires only cross the edge, and the blanket includes such nets.
    EdgeIncluded,
}

impl BlanketCatch {
    /// Whether the blanket's parameters reach the net.
    pub fn is_caught(self) -> bool {
        !matches!(self, Self::EdgeOnly)
    }
}

/// One net under a Net Blanket, for its Properties list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlanketNet {
    pub net: NetId,
    pub name: String,
    pub catch: BlanketCatch,
}

/// The power net names of `sheet`: the value of each power port and the
/// text of each power label. A Net Blanket never catches these nets.
fn power_names(sheet: &SchematicSheet) -> HashSet<&str> {
    crate::build::name_carrying_ports(sheet)
        .into_iter()
        .map(|(_, name)| name)
        .chain(
            sheet
                .labels
                .iter()
                .filter(|l| l.label_type == LabelType::Power)
                .map(|l| l.text.as_str()),
        )
        .filter(|name| !name.is_empty())
        .collect()
}

/// Whether the segment `a`..`b` touches `r` (Liang-Barsky clip).
fn segment_touches(r: &Aabb, a: Point, b: Point) -> bool {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let mut t0 = 0.0_f64;
    let mut t1 = 1.0_f64;
    for (p, q) in [
        (-dx, a.x - r.min_x),
        (dx, r.max_x - a.x),
        (-dy, a.y - r.min_y),
        (dy, r.max_y - a.y),
    ] {
        if p == 0.0 {
            if q < 0.0 {
                return false;
            }
            continue;
        }
        let t = q / p;
        if p < 0.0 {
            t0 = t0.max(t);
        } else {
            t1 = t1.min(t);
        }
        if t0 > t1 {
            return false;
        }
    }
    true
}

/// How `blanket` holds the net made of `wires` and `junctions` on `sheet`;
/// `None` when it does not reach it at all.
fn catch_of(
    sheet: &SchematicSheet,
    blanket: &SchDirective,
    wires: &[Uuid],
    junctions: &[Uuid],
) -> Option<BlanketCatch> {
    let SchDirective::Blanket {
        include_edge_nets, ..
    } = blanket
    else {
        return None;
    };
    let region = blanket.region()?;
    let on_sheet = |id: &Uuid| sheet.wires.iter().find(|w| w.uuid == *id);
    let inside = wires
        .iter()
        .filter_map(on_sheet)
        .any(|w| blanket.region_contains(w.start) && blanket.region_contains(w.end))
        || junctions.iter().any(|id| {
            sheet
                .junctions
                .iter()
                .find(|j| j.uuid == *id)
                .is_some_and(|j| blanket.region_contains(j.position))
        });
    if inside {
        return Some(BlanketCatch::Inside);
    }
    let crosses = wires
        .iter()
        .filter_map(on_sheet)
        .any(|w| segment_touches(&region, w.start, w.end));
    match (crosses, include_edge_nets) {
        (false, _) => None,
        (true, true) => Some(BlanketCatch::EdgeIncluded),
        (true, false) => Some(BlanketCatch::EdgeOnly),
    }
}

/// Every net of `netlist` (built from `sheet`) that `blanket` reaches, in
/// netlist order: caught inside, or only crossing its edge. Power nets are
/// left out: a blanket never catches them.
pub fn blanket_nets(
    sheet: &SchematicSheet,
    netlist: &Netlist,
    blanket: &SchDirective,
) -> Vec<BlanketNet> {
    let power = power_names(sheet);
    netlist
        .nets
        .iter()
        .filter(|net| !power.contains(net.name.as_str()))
        .filter_map(|net| {
            let catch = catch_of(sheet, blanket, &net.wires, &net.junctions)?;
            Some(BlanketNet {
                net: net.id,
                name: net.name.clone(),
                catch,
            })
        })
        .collect()
}

/// The class a Net Blanket of `sheet` gives the net made of `wires` and
/// `junctions`: the first Blanket in sheet order that catches it and sets a
/// class. Never a power net's.
fn blanket_class<'s>(
    sheet: &'s SchematicSheet,
    power: &HashSet<&str>,
    name: &str,
    wires: &[Uuid],
    junctions: &[Uuid],
) -> Option<&'s str> {
    if power.contains(name) {
        return None;
    }
    sheet.directives.iter().find_map(|d| {
        let class = d.net_class().filter(|_| d.region().is_some())?;
        catch_of(sheet, d, wires, junctions)
            .filter(|c| c.is_caught())
            .map(|_| class)
    })
}

/// The class each Net Blanket gives a net of `netlist` (built from
/// `sheet`): see [`blanket_nets`] for what a blanket catches. The first
/// Blanket in sheet order wins.
pub fn blanket_net_classes(sheet: &SchematicSheet, netlist: &Netlist) -> Vec<(NetId, String)> {
    let power = power_names(sheet);
    netlist
        .nets
        .iter()
        .filter_map(|net| {
            let class = blanket_class(sheet, &power, &net.name, &net.wires, &net.junctions)?;
            Some((net.id, class.to_string()))
        })
        .collect()
}

/// One Net Parameters directive resolved against the netlist.
#[derive(Debug, Clone, PartialEq)]
pub struct NetParametersOn {
    pub directive: Uuid,
    pub net: NetId,
    pub net_name: String,
    pub net_class: Option<String>,
    pub diff_pair_class: Option<String>,
    pub rules: NetRules,
}

/// The net of `netlist` (built from `sheet`) the wire under `at` is on.
pub(crate) fn net_at<'n>(
    sheet: &SchematicSheet,
    netlist: &'n Netlist,
    at: &Point,
) -> Option<&'n Net> {
    let wire = wire_at_point(sheet, at)?;
    netlist.nets.iter().find(|n| n.wires.contains(&wire))
}

/// Every Net Parameters directive of `sheet` that sits on a wire, with the
/// net it names, in sheet order. A directive on no wire is left out.
pub fn net_parameters(sheet: &SchematicSheet, netlist: &Netlist) -> Vec<NetParametersOn> {
    sheet
        .directives
        .iter()
        .filter_map(|d| {
            let SchDirective::NetParameters {
                uuid,
                position,
                rules,
                ..
            } = d
            else {
                return None;
            };
            let net = net_at(sheet, netlist, position)?;
            Some(NetParametersOn {
                directive: *uuid,
                net: net.id,
                net_name: net.name.clone(),
                net_class: d.net_class().map(str::to_string),
                diff_pair_class: d.diff_pair_class().map(str::to_string),
                rules: rules.clone(),
            })
        })
        .collect()
}

/// The class the directives of `sheet` give each net of `netlist`: a Net
/// Parameters directive on the net (the first in sheet order that sets a
/// class) outranks a Net Blanket.
pub fn directive_net_classes(sheet: &SchematicSheet, netlist: &Netlist) -> Vec<(NetId, String)> {
    let mut out: Vec<(NetId, String)> = Vec::new();
    for on in net_parameters(sheet, netlist) {
        if let Some(class) = on.net_class
            && !out.iter().any(|(id, _)| *id == on.net)
        {
            out.push((on.net, class));
        }
    }
    for (id, class) in blanket_net_classes(sheet, netlist) {
        if !out.iter().any(|(set, _)| *set == id) {
            out.push((id, class));
        }
    }
    out.sort_by_key(|(id, _)| id.0);
    out
}

/// Give every net without a class the first project class whose names or
/// patterns match it; a class a directive set is kept, so the order is
/// Net Parameters, Net Blanket, then the project's patterns.
pub fn fill_pattern_classes(netlist: &mut Netlist, classes: &[NetClass]) {
    for net in netlist.nets.iter_mut().filter(|n| n.class.is_none()) {
        net.class = first_matching_class(classes, &net.name).map(|c| NetClassId(c.name.clone()));
    }
}

/// Every Differential Pair directive of `sheet` that sits on a wire whose
/// net (in `netlist`, built from `sheet`) carries a pair suffix, with both
/// halves looked up by name. No class is known here, so every suffix style
/// pairs (`diff_pair_classes::diff_pairs_with_classes`).
pub fn diff_pairs(sheet: &SchematicSheet, netlist: &Netlist) -> Vec<DiffPair> {
    crate::diff_pair_classes::diff_pairs_with_classes(sheet, netlist, &[])
}

/// Set each net's class from the directives of `sheet`
/// ([`directive_net_classes`]).
pub(crate) fn apply_directive_classes(sheet: &SchematicSheet, netlist: &mut Netlist) {
    for (id, class) in directive_net_classes(sheet, netlist) {
        if let Some(net) = netlist.nets.iter_mut().find(|n| n.id == id) {
            net.class = Some(NetClassId(class));
        }
    }
}

/// Set each net of the stitched project netlist from the directives of the
/// sheets its wires are on: a Net Parameters directive on one of its wires
/// first, then a Net Blanket catching it. Sheets are read in key order, so
/// the answer never depends on the map's order.
pub(crate) fn apply_project_classes(
    sheets: &HashMap<crate::SheetKey, SchematicSheet>,
    nets: &mut [Net],
) {
    let mut keyed: Vec<(&crate::SheetKey, &SchematicSheet)> = sheets
        .iter()
        .filter(|(_, s)| !s.directives.is_empty())
        .collect();
    if keyed.is_empty() {
        return;
    }
    keyed.sort_by(|a, b| a.0.cmp(b.0));
    let parameters: Vec<(Uuid, &str)> = keyed
        .iter()
        .flat_map(|&(_, sheet)| {
            sheet.directives.iter().filter_map(move |d| {
                let SchDirective::NetParameters { position, .. } = d else {
                    return None;
                };
                Some((wire_at_point(sheet, position)?, d.net_class()?))
            })
        })
        .collect();
    let powers: Vec<HashSet<&str>> = keyed.iter().map(|&(_, s)| power_names(s)).collect();
    for net in nets.iter_mut() {
        let by_parameters = parameters
            .iter()
            .find(|(wire, _)| net.wires.contains(wire))
            .map(|(_, class)| class.to_string());
        let class = by_parameters.or_else(|| {
            keyed.iter().zip(&powers).find_map(|(&(_, sheet), power)| {
                blanket_class(sheet, power, &net.name, &net.wires, &net.junctions)
                    .map(str::to_string)
            })
        });
        if let Some(class) = class {
            net.class = Some(NetClassId(class));
        }
    }
}

/// The project's sheets with what their Compile Masks cover taken out, for
/// the stitched netlist; `None` when no sheet has a mask, so the stitch reads
/// the sheets as they are.
pub(crate) fn masked_sheets(
    sheets: &HashMap<crate::SheetKey, SchematicSheet>,
) -> Option<HashMap<crate::SheetKey, SchematicSheet>> {
    let has_mask = |s: &SchematicSheet| {
        s.directives
            .iter()
            .any(|d| matches!(d, SchDirective::CompileMask { .. }))
    };
    if !sheets.values().any(has_mask) {
        return None;
    }
    Some(
        sheets
            .iter()
            .map(|(key, sheet)| (key.clone(), compile_masked(sheet).into_owned()))
            .collect(),
    )
}
