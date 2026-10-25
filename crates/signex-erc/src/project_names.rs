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
//! drops the sheet's own `NetMultipleNames` findings; the per-sheet rule
//! remains for a sheet checked on its own. Nets that `NetLabelConflict`
//! reports on one of their sheets are left to it, as in the per-sheet rule.

use std::collections::{HashMap, HashSet};

use signex_net::{MultiNamedNet, NetNameSource, ProjectGraph, SheetKey, pt_key};
use signex_types::schematic::{SelectedItem, SelectedKind};

use crate::context::ErcContext;
use crate::diagnostic::Diagnostic;
use crate::rules::label_conflict_groups;
use crate::{RuleKind, Violation, sel};

/// Per-sheet facts needed to apply the `NetLabelConflict` overlap and No-ERC
/// directives.
struct SheetFacts {
    ctx: ErcContext,
    conflicted: HashSet<(i64, i64)>,
}

type Facts = HashMap<SheetKey, SheetFacts>;

/// Every multiple-names finding in `graph`, each with the key of the sheet it
/// is filed under (the sheet of the name that won, else the first name).
pub fn project_net_multiple_names(graph: &ProjectGraph) -> Vec<(SheetKey, Violation)> {
    let mut facts = Facts::new();
    let mut out = Vec::new();
    // A child placed twice yields one finding per instance; one marker, one row.
    let mut seen: HashSet<(SheetKey, (i64, i64), Vec<String>)> = HashSet::new();
    for net in signex_net::multi_named_nets(graph) {
        load_facts(graph, &net, &mut facts);
        let Some(primary) = reportable_primary(&net, &facts) else {
            continue;
        };
        let mut names: Vec<String> = net.sources.iter().map(|s| s.name.clone()).collect();
        names.dedup();
        if seen.insert((primary.sheet.clone(), pt_key(&primary.at), names)) {
            out.push((
                primary.sheet.clone(),
                Violation::from(finding(&net, primary)),
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

/// The source the finding sits on (the winning name, else the first), or
/// `None` when `NetLabelConflict` owns the net or a No-ERC directive covers it.
fn reportable_primary<'a>(net: &'a MultiNamedNet, facts: &Facts) -> Option<&'a NetNameSource> {
    let conflicted = net.sources.iter().any(|s| {
        facts
            .get(&s.sheet)
            .is_some_and(|f| f.conflicted.contains(&pt_key(&s.at)))
    });
    let primary = net
        .sources
        .iter()
        .find(|s| s.exported_text() == net.exported)
        .or(net.sources.first())?;
    let no_erc = facts
        .get(&primary.sheet)
        .is_some_and(|f| f.ctx.has_no_erc_directive_at(&primary.at));
    (!conflicted && !no_erc).then_some(primary)
}

/// The diagnostic for `net`, placed on `primary`.
fn finding(net: &MultiNamedNet, primary: &NetNameSource) -> Diagnostic {
    let listing: Vec<String> = net
        .sources
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
    let mut d = Diagnostic::new(
        RuleKind::NetMultipleNames,
        format!(
            "Net has multiple names; the netlist uses '{}': {}",
            net.exported,
            listing.join("; "),
        ),
        primary.at,
    )
    .with_primary(source_selection(primary));
    // A uuid selects on the finding's own canvas only; a peer on another sheet
    // would target the wrong object, so it is left unselected.
    let peer = net
        .sources
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
