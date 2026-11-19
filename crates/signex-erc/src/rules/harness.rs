//! Signal harness rules (#1280).
//!
//! - `harness_entry_mismatch`: a connector entry its type's definition does
//!   not list, a definition entry the connector lacks, or an entry the far
//!   side of the harness (the child sheet behind a harness-typed sheet entry)
//!   carries and this side lacks. Each finding names both sides, e.g.
//!   `sensor expects entry CS_ADC; this harness has CS`, and sits on the
//!   entry that stands in the expected one's place.
//! - `harness_type_conflict`: connectors or typed sheet entries of two types
//!   on one harness.
//!
//! Waivers and No-ERC directives work as for every rule. Rules and naming:
//! `docs/SIGNAL_HARNESSES.md`.

use signex_net::{HarnessMismatch, harness_mismatches};
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
/// has Y` (or `lacks it` when nothing stands in its place).
fn push_mismatches(
    c: &HarnessConnector,
    who: &str,
    found: Vec<HarnessMismatch>,
    tag: &str,
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
        out.push(
            Diagnostic::new(RuleKind::HarnessEntryMismatch, message, at)
                .with_primary(primary)
                .tagged(format!("{tag}:{}", m.expects)),
        );
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
    push_mismatches(c, &c.harness_type, found, "definition", out);
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
/// against the entries the child's same-named harness port carries.
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
                .and_then(|child| child.harness.port_entries(&entry.name))
            else {
                continue;
            };
            for c in bundle
                .connectors
                .iter()
                .filter_map(|id| ctx.harness.connector(*id))
            {
                let found = harness_mismatches(c.uuid, &entries_of(c), far);
                push_mismatches(c, &symbol.name, found, &symbol.name, out);
            }
        }
    }
}

#[cfg(test)]
#[path = "tests/harness_tests.rs"]
mod tests;
