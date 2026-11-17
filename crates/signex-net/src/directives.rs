//! What the sheet's directives (#2420) mean for connectivity, derived once
//! here so the netlist, ERC and the board read the same answer:
//!
//! - a Compile Mask takes what it covers out of the netlist and ERC
//!   ([`compile_masked`]);
//! - a Blanket gives every net inside it its net class
//!   ([`blanket_net_classes`]);
//! - a Differential Pair directive pairs the net it sits on with its
//!   `_P` / `_N` partner ([`diff_pairs`]).
//!
//! Design note: `docs/SCHEMATIC_DIRECTIVES.md`.

use std::borrow::Cow;

use signex_types::net::{NetId, Netlist};
use signex_types::schematic::{Point, SchDirective, SchematicSheet, diff_pair_base};
use uuid::Uuid;

use crate::net_class::wire_at_point;

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

/// The class each Blanket gives a net of `netlist` (built from `sheet`): a
/// net is inside a Blanket when one of its wires lies wholly inside it or one
/// of its junctions does. The first Blanket in sheet order wins.
pub fn blanket_net_classes(sheet: &SchematicSheet, netlist: &Netlist) -> Vec<(NetId, String)> {
    let blankets: Vec<(&SchDirective, &str)> = sheet
        .directives
        .iter()
        .filter_map(|d| Some((d, d.net_class()?)))
        .collect();
    if blankets.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for net in &netlist.nets {
        let inside = |blanket: &SchDirective| {
            let wire_in = net.wires.iter().any(|id| {
                sheet.wires.iter().find(|w| w.uuid == *id).is_some_and(|w| {
                    blanket.region_contains(w.start) && blanket.region_contains(w.end)
                })
            });
            wire_in
                || net.junctions.iter().any(|id| {
                    sheet
                        .junctions
                        .iter()
                        .find(|j| j.uuid == *id)
                        .is_some_and(|j| blanket.region_contains(j.position))
                })
        };
        if let Some((_, class)) = blankets.iter().find(|entry| inside(entry.0)) {
            out.push((net.id, (*class).to_string()));
        }
    }
    out
}

/// One Differential Pair directive resolved against the netlist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffPair {
    /// The directive that marks the pair.
    pub directive: Uuid,
    /// The pair's name: the shared base of its nets (`USB_D`).
    pub name: String,
    /// The `<name>_P` net, when the sheet has one.
    pub positive: Option<String>,
    /// The `<name>_N` net, when the sheet has one.
    pub negative: Option<String>,
}

impl DiffPair {
    /// Whether both halves were found.
    pub fn is_complete(&self) -> bool {
        self.positive.is_some() && self.negative.is_some()
    }
}

/// Every Differential Pair directive of `sheet` that sits on a wire whose
/// net (in `netlist`, built from `sheet`) ends in `_P` or `_N`, with both
/// halves looked up by name. A directive on no wire, or on a net without
/// the suffix, pairs nothing and is left out.
pub fn diff_pairs(sheet: &SchematicSheet, netlist: &Netlist) -> Vec<DiffPair> {
    let mut out = Vec::new();
    for directive in &sheet.directives {
        let SchDirective::DiffPair { uuid, position } = directive else {
            continue;
        };
        let Some(wire) = wire_at_point(sheet, position) else {
            continue;
        };
        let Some(net) = netlist.nets.iter().find(|n| n.wires.contains(&wire)) else {
            continue;
        };
        let Some(base) = diff_pair_base(&net.name) else {
            continue;
        };
        let named = |suffix: &str| {
            let want = format!("{base}{suffix}");
            netlist.nets.iter().any(|n| n.name == want).then_some(want)
        };
        out.push(DiffPair {
            directive: *uuid,
            name: base.to_string(),
            positive: named("_P"),
            negative: named("_N"),
        });
    }
    out
}

/// Set each net's class from the Blankets of `sheet` ([`blanket_net_classes`]).
pub(crate) fn apply_blanket_classes(sheet: &SchematicSheet, netlist: &mut Netlist) {
    for (id, class) in blanket_net_classes(sheet, netlist) {
        if let Some(net) = netlist.nets.iter_mut().find(|n| n.id == id) {
            net.class = Some(signex_types::net::NetClassId(class));
        }
    }
}

/// The project's sheets with what their Compile Masks cover taken out, for
/// the stitched netlist; `None` when no sheet has a mask, so the stitch reads
/// the sheets as they are.
pub(crate) fn masked_sheets(
    sheets: &std::collections::HashMap<crate::SheetKey, SchematicSheet>,
) -> Option<std::collections::HashMap<crate::SheetKey, SchematicSheet>> {
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
