//! Rule: NetMultipleNames, project-wide (#1061).
//!
//! The per-sheet rule (`rules/multiple_names.rs`) cannot see a name that a net
//! gets on another sheet, such as a Global `I2C_SDA` on the root wire of sheet
//! pin `SDA` while the child calls its side Hierarchical `SDA`. This pass reads
//! the stitched project netlist (`signex_net::multi_named_nets`), the same one
//! the export uses, so the name it reports as used is the exported one.
//!
//! One rule, two entry points: for a sheet the project roots reach this pass
//! owns the finding (single-sheet and pin-less nets included), and the caller
//! drops a sheet's own `NetMultipleNames` finding only where a project finding
//! that covers it was emitted (see [`ProjectNetNames::covered`]); the per-sheet
//! rule remains for a sheet checked on its own.
//!
//! Each net is judged per sheet. A sheet's part of the net is left out when
//! `NetLabelConflict` reports the net on that sheet (as in the per-sheet rule),
//! or when a No-ERC directive sits on any of its names there. The finding is
//! made from the remaining sheets, if they still carry two different names.

use std::collections::{HashMap, HashSet};

use signex_net::{MultiNamedNet, NetNameSource, ProjectGraph, SheetKey, pt_key};
use signex_types::schematic::{SelectedItem, SelectedKind};

use crate::context::ErcContext;
use crate::diagnostic::Diagnostic;
use crate::rules::label_conflict_groups;
use crate::{RuleKind, Violation, sel};

/// What the project pass found.
#[derive(Debug, Default)]
pub struct ProjectNetNames {
    /// Each finding, with the key of the sheet it is filed under.
    pub findings: Vec<(SheetKey, Violation)>,
    /// Every `(sheet, position)` name source an emitted finding covers. A
    /// per-sheet finding at one of these is replaced by that finding; any
    /// other per-sheet finding must be kept.
    pub covered: HashSet<(SheetKey, (i64, i64))>,
}

/// Per-sheet facts needed to apply the `NetLabelConflict` overlap and No-ERC
/// directives.
struct SheetFacts {
    ctx: ErcContext,
    conflicted: HashSet<(i64, i64)>,
}

type Facts = HashMap<SheetKey, SheetFacts>;

/// Every multiple-names finding in `graph`, each filed under the sheet of the
/// name that won when that sheet is part of it, else of the best name left.
pub fn project_net_multiple_names(graph: &ProjectGraph) -> ProjectNetNames {
    let mut facts = Facts::new();
    let mut out = ProjectNetNames::default();
    // A child placed twice yields one finding per instance; one marker, one row.
    let mut seen: HashSet<(SheetKey, (i64, i64), Vec<String>)> = HashSet::new();
    for net in signex_net::multi_named_nets(graph) {
        load_facts(graph, &net, &mut facts);
        let included = reportable_sources(&net, &facts);
        let mut names: Vec<String> = included.iter().map(|s| s.name.clone()).collect();
        names.dedup();
        if names.len() < 2 {
            continue;
        }
        let primary = primary_source(&net, &included);
        out.covered
            .extend(included.iter().map(|s| (s.sheet.clone(), pt_key(&s.at))));
        if seen.insert((primary.sheet.clone(), pt_key(&primary.at), names)) {
            out.findings.push((
                primary.sheet.clone(),
                Violation::from(finding(&net, &included, primary)),
            ));
        }
    }
    out
}

/// Cache the conflict/No-ERC facts of every sheet `net` touches.
fn load_facts(graph: &ProjectGraph, net: &MultiNamedNet, facts: &mut Facts) {
    for s in &net.sources {
        let Some(sheet) = graph.sheets.get(&s.sheet) else {
            continue;
        };
        facts.entry(s.sheet.clone()).or_insert_with(|| {
            let ctx = ErcContext::project(sheet, HashMap::new());
            let conflicted = label_conflict_groups(&ctx)
                .iter()
                .flatten()
                .map(|l| pt_key(&l.position))
                .collect();
            SheetFacts { ctx, conflicted }
        });
    }
}

/// The sources of the sheets whose part of `net` is reported: a sheet where
/// `NetLabelConflict` owns the net, or where a No-ERC directive sits on any of
/// the net's names, is left out.
fn reportable_sources<'a>(net: &'a MultiNamedNet, facts: &Facts) -> Vec<&'a NetNameSource> {
    let left_out: HashSet<&SheetKey> = net
        .sources
        .iter()
        .filter(|s| {
            facts.get(&s.sheet).is_some_and(|f| {
                f.conflicted.contains(&pt_key(&s.at)) || f.ctx.has_no_erc_directive_at(&s.at)
            })
        })
        .map(|s| &s.sheet)
        .collect();
    net.sources
        .iter()
        .filter(|s| !left_out.contains(&s.sheet))
        .collect()
}

/// The source the finding sits on: the winning name when its sheet is
/// reported, else the name that would win among the reported ones.
fn primary_source<'a>(net: &'a MultiNamedNet, included: &[&'a NetNameSource]) -> &'a NetNameSource {
    let winner = &net.sources[net.winner];
    if included.iter().any(|s| s.sheet == winner.sheet) {
        return winner;
    }
    let power: Vec<&str> = included
        .iter()
        .filter(|s| s.label_type.is_none())
        .map(|s| s.name.as_str())
        .collect();
    let labelled = included
        .iter()
        .filter_map(|s| s.label_type.map(|t| (t, s.name.as_str())));
    let best = signex_net::best_net_name_of(labelled, &power).map(|(_, n)| n);
    included
        .iter()
        .find(|s| Some(&s.name) == best.as_ref())
        .unwrap_or(&included[0])
}

/// The diagnostic for `net`, listing the `included` sources, placed on
/// `primary`.
fn finding(
    net: &MultiNamedNet,
    included: &[&NetNameSource],
    primary: &NetNameSource,
) -> Diagnostic {
    let listing: Vec<String> = included
        .iter()
        .map(|s| {
            let kind = s.label_type.map_or("power port".to_string(), |t| {
                format!("{t:?}").to_lowercase()
            });
            format!(
                "'{}' ({kind}) on {} at ({:.2}, {:.2})",
                s.name,
                sheet_name(s),
                s.at.x,
                s.at.y
            )
        })
        .collect();
    // A net with no terminal is not exported, so no netlist "uses" its name.
    let used = if net.has_terminals {
        format!("the netlist uses '{}'", net.exported)
    } else {
        format!("no net is exported; it would be named '{}'", net.exported)
    };
    let mut d = Diagnostic::new(
        RuleKind::NetMultipleNames,
        format!("Net has multiple names; {used}: {}", listing.join("; ")),
        primary.at,
    )
    .with_primary(source_selection(primary));
    // A uuid selects on the finding's own canvas only; a peer on another sheet
    // would target the wrong object, so it is left unselected.
    let peer = included
        .iter()
        .find(|s| s.name != primary.name && s.sheet == primary.sheet);
    if let Some(peer) = peer {
        d = d.with_peer(source_selection(peer));
    }
    d
}

fn source_selection(s: &NetNameSource) -> SelectedItem {
    let kind = if s.label_type.is_some() {
        SelectedKind::Label
    } else {
        SelectedKind::Symbol
    };
    sel(s.uuid, kind)
}

/// A sheet as the user sees it: its chain of sheet-symbol names, else its file.
fn sheet_name(s: &NetNameSource) -> String {
    if s.sheet_path.is_empty() {
        format!("sheet {}", s.sheet)
    } else {
        format!("sheet {}", s.sheet_path.join("/"))
    }
}
