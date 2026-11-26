//! Signal harness rules (#1280): `harness_entry_mismatch` (an entry one side
//! of a harness or its definition expects and the other lacks, named on both
//! sides, on the entry that stands in its place) and `harness_type_conflict`
//! (two types on one harness). Waivers and No-ERC directives work as for
//! every rule. What each finding says: `docs/SIGNAL_HARNESSES.md`, "ERC".

use std::collections::HashSet;

use signex_net::{HarnessLeaf, HarnessMismatch, harness_mismatches};
use signex_types::schematic::{
    HarnessConnector, HarnessDefinition, SelectedKind, harness_definition,
};
use uuid::Uuid;

use crate::context::ErcContext;
use crate::diagnostic::Diagnostic;
use crate::{RuleKind, sel};

/// Connectors or typed sheet entries of two or more types on one harness.
pub(crate) fn harness_type_conflict(ctx: &ErcContext, out: &mut Vec<Diagnostic>) {
    for bundle in ctx.harness.bundles.iter().filter(|b| b.types.len() > 1) {
        let types = bundle.types.join(" and ");
        let message = format!("Harness types {types} meet on one harness");
        let mut connectors = bundle
            .connectors
            .iter()
            .filter_map(|id| ctx.harness.connector(*id));
        let Some(first) = connectors.next() else {
            // Only typed sheet entries meet: report on the first of them.
            if let Some(entry) = bundle.sheet_entries.first() {
                let at = ctx
                    .child_sheets
                    .iter()
                    .flat_map(|cs| cs.pins.iter())
                    .find(|p| p.uuid == entry.pin)
                    .map_or_else(Default::default, |p| p.position);
                out.push(
                    Diagnostic::new(RuleKind::HarnessTypeConflict, message, at)
                        .with_primary(sel(entry.pin, SelectedKind::SheetPin)),
                );
            }
            continue;
        };
        let mut d = Diagnostic::new(RuleKind::HarnessTypeConflict, message, first.tip_point())
            .with_primary(sel(first.uuid, SelectedKind::HarnessConnector));
        if let Some(other) = connectors.find(|c| c.harness_type != first.harness_type) {
            d = d.with_peer(sel(other.uuid, SelectedKind::HarnessConnector));
        }
        out.push(d);
    }
}

/// Entries that do not match the type's definition (when `definitions` has
/// it) or the far side.
pub(crate) fn harness_entry_mismatch(
    ctx: &ErcContext,
    definitions: &[HarnessDefinition],
    out: &mut Vec<Diagnostic>,
) {
    for c in &ctx.harness.connectors {
        definition_mismatches(definitions, c, out);
    }
    far_side_mismatches(ctx, out);
}

/// `(uuid, name)` of every entry of `c`.
fn entries_of(c: &HarnessConnector) -> Vec<(Uuid, String)> {
    c.entries.iter().map(|e| (e.uuid, e.name.clone())).collect()
}

/// One finding per mismatch, worded `<who> expects entry X; this harness
/// has Y` (or `lacks it` when nothing stands in its place). A far-side
/// finding names the sheet entry the harness leaves through as its peer, so
/// the canvas can badge that entry (decision of 2026-11-25).
fn push_mismatches(
    c: &HarnessConnector,
    who: &str,
    found: Vec<HarnessMismatch>,
    tag: &str,
    sheet_entry: Option<Uuid>,
    out: &mut Vec<Diagnostic>,
) {
    for m in found {
        let index = m.entry.and_then(|id| c.entry_index(id));
        let message = match &m.has {
            Some(has) => format!("{who} expects entry {}; this harness has {has}", m.expects),
            None => format!("{who} expects entry {}; this harness lacks it", m.expects),
        };
        let at = index.map_or_else(|| c.tip_point(), |i| c.entry_point(i));
        let primary = match m.entry {
            Some(id) => sel(id, SelectedKind::HarnessEntry),
            None => sel(c.uuid, SelectedKind::HarnessConnector),
        };
        let finding = Diagnostic::new(RuleKind::HarnessEntryMismatch, message, at)
            .with_primary(primary)
            .tagged(format!("{tag}:{}", m.expects));
        out.push(through_sheet_entry(finding, sheet_entry));
    }
}

/// `finding` with the sheet entry it reaches the far side through as its
/// peer, when it has one.
fn through_sheet_entry(finding: Diagnostic, sheet_entry: Option<Uuid>) -> Diagnostic {
    match sheet_entry {
        Some(pin) => finding.with_peer(sel(pin, SelectedKind::SheetPin)),
        None => finding,
    }
}

/// Entries of `c` against its type's definition, when the project has one.
fn definition_mismatches(
    definitions: &[HarnessDefinition],
    c: &HarnessConnector,
    out: &mut Vec<Diagnostic>,
) {
    let Some(def) = harness_definition(definitions, &c.harness_type) else {
        return;
    };
    let names: Vec<String> = def.entries.iter().map(|e| e.name.clone()).collect();
    let entries = entries_of(c);
    let found = harness_mismatches(c.uuid, &entries, &names);
    let paired: Vec<Uuid> = found.iter().filter_map(|m| m.entry).collect();
    push_mismatches(c, &c.harness_type, found, "definition", None, out);
    // An entry the definition does not list, with nothing missing to pair.
    for (i, e) in c.entries.iter().enumerate() {
        if names.contains(&e.name) || paired.contains(&e.uuid) {
            continue;
        }
        out.push(
            Diagnostic::new(
                RuleKind::HarnessEntryMismatch,
                format!("{} defines no entry {}", c.harness_type, e.name),
                c.entry_point(i),
            )
            .with_primary(sel(e.uuid, SelectedKind::HarnessEntry))
            .tagged(format!("undefined:{}", e.name)),
        );
    }
}

/// Each connector of a bundle that runs to a harness-typed sheet entry,
/// against the leaves the child's same-named harness port carries. The parent
/// reports both directions: a child context has no link to its parent.
fn far_side_mismatches(ctx: &ErcContext, out: &mut Vec<Diagnostic>) {
    for bundle in ctx.harness.bundles.iter().filter(|b| !b.nested) {
        for entry in &bundle.sheet_entries {
            let Some(symbol) = ctx
                .child_sheets
                .iter()
                .find(|cs| cs.uuid == entry.sheet_symbol)
            else {
                continue;
            };
            let Some(far) = ctx
                .children
                .get(&symbol.filename)
                .and_then(|child| far_leaves(child, &entry.name, 0))
            else {
                continue;
            };
            for c in bundle
                .connectors
                .iter()
                .filter_map(|id| ctx.harness.connector(*id))
            {
                let near: Vec<&HarnessLeaf> = bundle
                    .leaves
                    .iter()
                    .filter(|l| l.connector == c.uuid)
                    .collect();
                compare_with_far(c, &near, &far, (&symbol.name, entry.pin), out);
            }
        }
    }
}

/// How many sheets that only pass a harness on ERC looks through; a longer
/// chain is read as ending there.
const CARRY_DEPTH: usize = 32;

/// The leaves the harness port `name` of `child` carries. A child that only
/// passes the harness on (#2657) is looked through to its own children; when
/// that reaches no connector the far side is empty, so every entry here is
/// reported. `None` when the child has no such port.
fn far_leaves(child: &ErcContext, name: &str, depth: usize) -> Option<Vec<HarnessLeaf>> {
    let bundle = child.harness.port_bundle(name)?;
    if !bundle.passes_through() {
        return Some(bundle.leaves.clone());
    }
    let mut out = Vec::new();
    if depth >= CARRY_DEPTH {
        return Some(out);
    }
    for entry in &bundle.sheet_entries {
        let leaves = child
            .child_sheets
            .iter()
            .find(|cs| cs.uuid == entry.sheet_symbol)
            .and_then(|cs| child.children.get(&cs.filename))
            .and_then(|grand| far_leaves(grand, &entry.name, depth + 1));
        out.extend(leaves.into_iter().flatten());
    }
    Some(out)
}

/// The leaf paths of `c` against the child's, both ways. A plain entry missing
/// on one side pairs with a plain extra on the other, as a rename the panel
/// can offer; a nested path never does, since renaming its entry would move
/// every leaf under it. What stays unpaired is named on its own. `through`
/// is the far sheet symbol's name and the sheet entry the harness leaves by.
fn compare_with_far(
    c: &HarnessConnector,
    near: &[&HarnessLeaf],
    far: &[HarnessLeaf],
    through: (&str, Uuid),
    out: &mut Vec<Diagnostic>,
) {
    let (sheet, sheet_entry) = through;
    let near_paths: HashSet<String> = near.iter().map(|l| l.dotted()).collect();
    let far_paths: HashSet<String> = far.iter().map(HarnessLeaf::dotted).collect();
    let mut listed = HashSet::new();
    let (plain_missing, nested_missing): (Vec<&HarnessLeaf>, Vec<&HarnessLeaf>) = far
        .iter()
        .filter(|l| !near_paths.contains(&l.dotted()) && listed.insert(l.dotted()))
        .partition(|l| l.is_plain());
    let extra: Vec<&&HarnessLeaf> = near
        .iter()
        .filter(|l| !far_paths.contains(&l.dotted()))
        .collect();
    let plain_extra: Vec<(Uuid, String)> = extra
        .iter()
        .filter(|l| l.is_plain())
        .map(|l| (l.entry, l.dotted()))
        .collect();
    let expected: Vec<String> = plain_missing.iter().map(|l| l.dotted()).collect();
    let mut found = harness_mismatches(c.uuid, &plain_extra, &expected);
    let paired: Vec<Uuid> = found.iter().filter_map(|m| m.entry).collect();
    found.extend(nested_missing.iter().map(|l| HarnessMismatch {
        connector: c.uuid,
        entry: None,
        has: None,
        expects: l.dotted(),
    }));
    // The app reads the far side back from the tag
    // (`far:<sheet symbol>:<expected entry>`).
    let tag = format!("far:{sheet}");
    push_mismatches(c, sheet, found, &tag, Some(sheet_entry), out);
    for leaf in extra
        .iter()
        .filter(|l| !(l.is_plain() && paired.contains(&l.entry)))
    {
        let name = leaf.dotted();
        let at = c
            .entry_index(leaf.entry)
            .map_or_else(|| c.tip_point(), |i| c.entry_point(i));
        let finding = Diagnostic::new(
            RuleKind::HarnessEntryMismatch,
            format!("{sheet} has no entry {name}"),
            at,
        )
        .with_primary(sel(leaf.entry, SelectedKind::HarnessEntry))
        .tagged(format!("extra:{sheet}:{name}"));
        out.push(through_sheet_entry(finding, Some(sheet_entry)));
    }
}

#[cfg(test)]
#[path = "tests/harness_tests.rs"]
mod tests;
