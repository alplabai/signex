//! Rule: LocalLabelNameOnOtherSheets.
//!
//! A net label is local to its sheet (ADR-0002 D8): two `SDA` net labels on two
//! sheets are two separate nets, never one. Users often expect them to join, so
//! this rule warns when a local Net label's name is also a local Net label on a
//! different sheet and nothing global (a Port, a power port or a Power label of
//! that name) carries the name across.
//!
//! It runs on the sheet being checked plus its descendants, the same view
//! `DuplicateRefDesignator` has, and reuses its reporting scope: a clash wholly
//! inside one child sheet's subtree belongs to that child's own run. A child
//! sheet placed twice is one file and is compared once, never with itself.

use std::collections::{BTreeMap, HashSet};

use signex_net::SheetKey;
use signex_types::schematic::{LabelType, SelectedKind};

use crate::context::{ErcContext, ErcLabel};
use crate::diagnostic::{Diagnostic, LocationSpace};
use crate::{RuleKind, sel};

use super::clash_scope;

/// This sheet (origin 0, unnamed) and every distinct descendant, named by the
/// filename its parent references it with. Filename order, depth first; a file
/// reached again (repeated instance, cycle) is taken once.
fn sheets_in_scope(ctx: &ErcContext) -> Vec<(Option<&str>, &ErcContext)> {
    fn walk<'a>(
        ctx: &'a ErcContext,
        out: &mut Vec<(Option<&'a str>, &'a ErcContext)>,
        seen: &mut HashSet<&'a SheetKey>,
    ) {
        let mut names: Vec<&String> = ctx.children.keys().collect();
        names.sort();
        for name in names {
            let child = &ctx.children[name];
            if let Some(k) = &child.key
                && !seen.insert(k)
            {
                continue;
            }
            out.push((Some(name.as_str()), child));
            walk(child, out, seen);
        }
    }
    let mut out = vec![(None, ctx)];
    let mut seen: HashSet<&SheetKey> = ctx.key.iter().collect();
    walk(ctx, &mut out, &mut seen);
    out
}

fn is_global_carrier(label: &ErcLabel) -> bool {
    matches!(label.label_type, LabelType::Global | LabelType::Power)
}

/// A sheet pin whose name is also a Hierarchical label in the child it points
/// at is a sheet entry the netlist joins: the name crosses the sheet boundary,
/// so the labels on either side are one net and the rule stays quiet.
fn carry_through_sheet_entries<'a>(sheet: &'a ErcContext, carried: &mut HashSet<&'a str>) {
    for cs in &sheet.child_sheets {
        let Some(child) = sheet.children.get(&cs.filename) else {
            continue;
        };
        for pin in &cs.pins {
            let name = pin.name.trim();
            let has_hier = child
                .labels
                .iter()
                .any(|l| l.label_type == LabelType::Hierarchical && l.text.trim() == name);
            if has_hier && !name.is_empty() {
                carried.insert(name);
            }
        }
    }
}

pub(crate) fn local_label_name_on_other_sheets(ctx: &ErcContext, out: &mut Vec<Diagnostic>) {
    let sources = sheets_in_scope(ctx);
    if sources.len() < 2 {
        return;
    }

    // Names something global carries across sheets; those join, so stay quiet.
    let mut carried: HashSet<&str> = HashSet::new();
    // name -> (source index, label) for every local Net label of that name.
    let mut local: BTreeMap<&str, Vec<(usize, &ErcLabel)>> = BTreeMap::new();
    for (origin, (_, sheet)) in sources.iter().enumerate() {
        for l in &sheet.labels {
            let name = l.text.trim();
            if name.is_empty() {
                continue;
            }
            if is_global_carrier(l) {
                carried.insert(name);
            } else if l.label_type == LabelType::Net {
                local.entry(name).or_default().push((origin, l));
            }
        }
        carry_through_sheet_entries(sheet, &mut carried);
        for s in sheet.symbols.iter().filter(|s| s.is_power) {
            let name = s.value.trim();
            if !name.is_empty() {
                carried.insert(name);
            }
        }
    }

    for (name, entries) in local {
        if carried.contains(name) {
            continue;
        }
        let mut origins: Vec<usize> = entries.iter().map(|e| e.0).collect();
        origins.dedup();
        if origins.len() < 2 {
            continue;
        }
        // A clash among descendants only is reported by the lowest sheet that
        // sees both sides, not by every ancestor above it.
        if origins[0] != 0 {
            let involved: Vec<&ErcContext> = origins.iter().map(|&o| sources[o].1).collect();
            if clash_scope::confined_to_one_child(ctx, &involved) {
                continue;
            }
        }
        let sheet_names: Vec<String> = origins
            .iter()
            .map(|&o| match sources[o].0 {
                Some(file) => format!("'{file}'"),
                None => "this sheet".to_string(),
            })
            .collect();
        let message = format!(
            "Net label '{name}' is local to its sheet but the same name is a local net label on {}; they are separate nets. Use a Port or a sheet entry to connect across sheets",
            sheet_names.join(", ")
        );
        let (origin, first) = entries[0];
        let mut d = Diagnostic::new(
            RuleKind::LocalLabelNameOnOtherSheets,
            message,
            first.position,
        );
        if origin == 0 {
            d = d.with_primary(sel(first.uuid, SelectedKind::Label));
        } else {
            d = d
                .in_space(LocationSpace::ChildSheet)
                .owned_by(sources[origin].1.key.clone());
        }
        out.push(d);
    }
}
