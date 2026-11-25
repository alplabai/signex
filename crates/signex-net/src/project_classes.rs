//! Net class directives across a project's sheets (#2656): the class the
//! stitched netlist gives each net, and the conflicts between directives on
//! different sheets that the per-sheet check cannot see. Both read one
//! ranking, [`project_class_assignments`], so the class a conflict names as
//! the winner is the class the netlist writes. Design note:
//! `docs/SCHEMATIC_DIRECTIVES.md`, "Class conflicts".

use std::collections::{HashMap, HashSet};

use signex_types::net::{Net, NetClassId};
use signex_types::project::ChannelNaming;
use signex_types::schematic::{SchDirective, SchematicSheet};
use uuid::Uuid;

use crate::directives::{
    BlanketCatch, ClassAssignment, catch_of, masked_sheets, power_names, split_overridden,
};
use crate::net_class::wire_at_point;
use crate::{ProjectGraph, SheetKey};

/// A class assignment and the sheet its directive is on.
#[derive(Debug, Clone, PartialEq)]
pub struct SheetClassAssignment {
    pub sheet: SheetKey,
    pub assignment: ClassAssignment,
}

/// Every class assignment the directives of `sheets` make to `net`, in rank
/// order: the Net Parameters directives on one of its wires (sheets in key
/// order, each in sheet order), then the Net Blankets catching it (the same
/// order). The first is the class the net takes. Restricted to one sheet,
/// this is that sheet's [`crate::class_assignments`] order.
fn ranked_assignments(
    keyed: &[(&SheetKey, &SchematicSheet)],
    powers: &[HashSet<&str>],
    net: &Net,
) -> Vec<SheetClassAssignment> {
    let mut out = Vec::new();
    for &(key, sheet) in keyed {
        for d in &sheet.directives {
            let SchDirective::NetParameters { position, .. } = d else {
                continue;
            };
            let on_net = wire_at_point(sheet, position).is_some_and(|w| net.wires.contains(&w));
            if let (true, Some(class)) = (on_net, d.net_class()) {
                out.push(SheetClassAssignment {
                    sheet: key.clone(),
                    assignment: ClassAssignment::of(d, class),
                });
            }
        }
    }
    for (&(key, sheet), power) in keyed.iter().zip(powers) {
        if power.contains(net.name.as_str()) {
            continue;
        }
        for d in sheet.directives.iter().filter(|d| d.region().is_some()) {
            let Some(class) = d.net_class() else {
                continue;
            };
            let caught =
                catch_of(sheet, d, &net.wires, &net.junctions).is_some_and(BlanketCatch::is_caught);
            if caught {
                out.push(SheetClassAssignment {
                    sheet: key.clone(),
                    assignment: ClassAssignment::of(d, class),
                });
            }
        }
    }
    out
}

/// The sheets that hold a directive, in key order, so no answer depends on
/// the map's order.
fn keyed_sheets(sheets: &HashMap<SheetKey, SchematicSheet>) -> Vec<(&SheetKey, &SchematicSheet)> {
    let mut keyed: Vec<(&SheetKey, &SchematicSheet)> = sheets
        .iter()
        .filter(|(_, s)| !s.directives.is_empty())
        .collect();
    keyed.sort_by(|a, b| a.0.cmp(b.0));
    keyed
}

/// The ranked assignments of every net of `nets`, in `nets` order; a net no
/// directive classes has an empty list.
fn project_class_assignments(
    sheets: &HashMap<SheetKey, SchematicSheet>,
    nets: &[Net],
) -> Vec<Vec<SheetClassAssignment>> {
    let keyed = keyed_sheets(sheets);
    if keyed.is_empty() {
        return vec![Vec::new(); nets.len()];
    }
    let powers: Vec<HashSet<&str>> = keyed.iter().map(|&(_, s)| power_names(s)).collect();
    nets.iter()
        .map(|net| ranked_assignments(&keyed, &powers, net))
        .collect()
}

/// Set each net of the stitched project netlist to the first of its
/// [`project_class_assignments`].
pub(crate) fn apply_project_classes(sheets: &HashMap<SheetKey, SchematicSheet>, nets: &mut [Net]) {
    let ranked = project_class_assignments(sheets, nets);
    for (net, list) in nets.iter_mut().zip(ranked) {
        if let Some(first) = list.into_iter().next() {
            net.class = Some(NetClassId(first.assignment.class));
        }
    }
}

/// A project net whose directives disagree on its class. The net takes the
/// `winner`'s class; `overridden` lists every later assignment naming
/// another class, in rank order, on any sheet.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectNetClassConflict {
    /// The net's name in the project netlist (the exported name).
    pub net_name: String,
    pub winner: SheetClassAssignment,
    pub overridden: Vec<SheetClassAssignment>,
}

/// What the project check found.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProjectClassConflicts {
    /// In project net order. A sheet placed several times (or repeated)
    /// lists a conflict once: the first net that carries it.
    pub conflicts: Vec<ProjectNetClassConflict>,
    /// Every `(sheet, directive)` that classes a project net, conflicting or
    /// not. The per-sheet check's answer for one of these is replaced by
    /// this one; a directive on a sheet the roots never reach is not here.
    pub judged: HashSet<(SheetKey, Uuid)>,
}

/// Every net class conflict of the stitched project netlist of `graph`,
/// ranked as the netlist ranks them (sheets read with their Compile Masks
/// applied, as the stitch reads them). The netlist is stitched only when two
/// directives of the project set a class, the least a conflict needs.
pub fn project_net_class_conflicts(
    graph: &ProjectGraph,
    naming: &ChannelNaming,
) -> ProjectClassConflicts {
    let classing = graph
        .sheets
        .values()
        .flat_map(|s| &s.directives)
        .filter(|d| d.net_class().is_some())
        .count();
    if classing < 2 {
        return ProjectClassConflicts::default();
    }
    let nets = crate::build_project_netlist_named(graph, naming)
        .netlist
        .nets;
    let masked = masked_sheets(graph.sheets);
    let sheets = masked.as_ref().unwrap_or(graph.sheets);
    let mut out = ProjectClassConflicts::default();
    let mut seen: HashSet<Vec<(SheetKey, Uuid)>> = HashSet::new();
    for (net, list) in nets.iter().zip(project_class_assignments(sheets, &nets)) {
        out.judged.extend(
            list.iter()
                .map(|a| (a.sheet.clone(), a.assignment.directive)),
        );
        let Some((winner, overridden)) = split_overridden(list, |a| a.assignment.class.as_str())
        else {
            continue;
        };
        let identity: Vec<(SheetKey, Uuid)> = std::iter::once(&winner)
            .chain(&overridden)
            .map(|a| (a.sheet.clone(), a.assignment.directive))
            .collect();
        if seen.insert(identity) {
            out.conflicts.push(ProjectNetClassConflict {
                net_name: net.name.clone(),
                winner,
                overridden,
            });
        }
    }
    out
}
