//! Buses across sheets (#1049): a bus-typed sheet pin must carry what the
//! child sheet's matching port carries.
//!
//! A pin and a port are the same bus when their names share a
//! [`join_key`](signex_types::schematic::BusName::join_key): `D[0..7]` on the
//! parent and `D[0..3]` on the child are one bus at two widths, and that is
//! the finding. A pin with no such counterpart is the plain "no matching
//! hierarchical label" case `bad_hier_sheet_pin` already reports.

use std::collections::{BTreeMap, HashSet};

use signex_types::schematic::{LabelType, SelectedKind, normalize_port_name, parse_bus_name};

use crate::context::{ErcContext, ErcLabel};
use crate::diagnostic::{Diagnostic, LocationSpace};
use crate::{RuleKind, sel};

use super::clash_scope;
use super::local_label_other_sheets::sheets_in_scope;

/// True when `name` is a bus and one of `others` is the same bus, at whatever
/// width. Lets the pin/label match rule leave a width disagreement to
/// [`bus_port_width_mismatch`] instead of also calling it an unmatched name.
pub(super) fn has_bus_counterpart<'a>(
    name: &str,
    mut others: impl Iterator<Item = &'a str>,
) -> bool {
    let Ok(this) = parse_bus_name(name) else {
        return false;
    };
    let key = this.join_key();
    others.any(|o| parse_bus_name(o).is_ok_and(|b| b.join_key() == key))
}

/// A bus-typed sheet pin whose child port names the same bus with different
/// members. Names both sides and the width each one carries.
pub(crate) fn bus_port_width_mismatch(ctx: &ErcContext, out: &mut Vec<Diagnostic>) {
    for child in &ctx.child_sheets {
        let Some(child_ctx) = ctx.children.get(child.filename.as_str()) else {
            continue;
        };
        for pin in &child.pins {
            let Some(pin_bus) = normalize_port_name(&pin.name).and_then(|n| parse_bus_name(n).ok())
            else {
                continue;
            };
            let key = pin_bus.join_key();
            let pin_members: HashSet<String> = pin_bus.members().into_iter().collect();
            for port in &child_ctx.labels {
                let Some(port_bus) = port.port_name().and_then(|n| parse_bus_name(n).ok()) else {
                    continue;
                };
                let port_members: HashSet<String> = port_bus.members().into_iter().collect();
                if port_bus.join_key() != key || port_members == pin_members {
                    continue;
                }
                out.push(
                    Diagnostic::new(
                        RuleKind::BusBitWidthMismatch,
                        format!(
                            "Bus width mismatch across sheets: sheet pin '{}' on '{}' carries {} \
                             signals, port '{}' in '{}' carries {}",
                            pin.name,
                            child.name,
                            pin_bus.width(),
                            port.text,
                            child.filename,
                            port_bus.width(),
                        ),
                        pin.position,
                    )
                    .with_primary(sel(pin.uuid, SelectedKind::SheetPin))
                    // One pin can disagree with several ports in the child;
                    // the port lives on another sheet, so it tags the finding.
                    .tagged(format!("port:{}", port.uuid)),
                );
            }
        }
    }
}

/// One Global bus label with the sheet it sits on (an index into the scope
/// order) and the members and width of the bus it names.
struct PortBus<'a> {
    origin: usize,
    label: &'a ErcLabel,
    members: HashSet<String>,
    width: usize,
}

type Sources<'a> = [(Option<&'a str>, &'a ErcContext)];

/// A Port (Global label) names one bus project-wide, so two Ports with the
/// same [`join_key`](signex_types::schematic::BusName::join_key) but different
/// members merge only on the members they share. Reports each such bus once,
/// naming the first two sheets that disagree and are not both inside one
/// child's subtree (that child reports them); equally wide Ports that carry
/// different signals are a name conflict, not a width mismatch. Local net
/// label buses never leave their sheet and are not compared.
pub(crate) fn global_bus_width_mismatch(ctx: &ErcContext, out: &mut Vec<Diagnostic>) {
    let sources = sheets_in_scope(ctx);
    if sources.len() < 2 {
        return;
    }
    for seen in port_buses_by_key(&sources).values() {
        if let Some((first, other)) = reportable_clash(ctx, &sources, seen) {
            out.push(clash_diagnostic(&sources, first, other));
        }
    }
}

/// Every Global bus label in scope, grouped by bus join key, in scope order.
fn port_buses_by_key<'a>(sources: &Sources<'a>) -> BTreeMap<String, Vec<PortBus<'a>>> {
    let mut by_key: BTreeMap<String, Vec<PortBus>> = BTreeMap::new();
    for (origin, (_, sheet)) in sources.iter().enumerate() {
        for label in &sheet.labels {
            if label.label_type != LabelType::Global {
                continue;
            }
            let Some(bus) = label.port_name().and_then(|n| parse_bus_name(n).ok()) else {
                continue;
            };
            by_key.entry(bus.join_key()).or_default().push(PortBus {
                origin,
                label,
                members: bus.members().into_iter().collect(),
                width: bus.width(),
            });
        }
    }
    by_key
}

/// The pair of one bus's Ports this run should report, if any.
///
/// Two names on one sheet are the same-sheet bus rules' finding, so only a
/// disagreement between different sheets counts. A clash wholly inside one
/// child's subtree is that child's finding: when every sheet taking part in
/// some clash is inside one child, nothing is reported here. Otherwise the
/// reported pair is the first clashing pair that is not itself inside one
/// child, so the root never repeats a finding a child already owns.
fn reportable_clash<'a, 'b>(
    ctx: &ErcContext,
    sources: &Sources,
    seen: &'b [PortBus<'a>],
) -> Option<(&'b PortBus<'a>, &'b PortBus<'a>)> {
    let clashes = |a: &PortBus, b: &PortBus| a.origin != b.origin && a.members != b.members;
    let mut origins: Vec<usize> = seen
        .iter()
        .filter(|a| seen.iter().any(|b| clashes(a, b)))
        .map(|a| a.origin)
        .collect();
    origins.sort_unstable();
    origins.dedup();
    let sheets_of =
        |origins: &[usize]| -> Vec<&ErcContext> { origins.iter().map(|&o| sources[o].1).collect() };
    if origins.is_empty() || clash_scope::confined_to_one_child(ctx, &sheets_of(&origins)) {
        return None;
    }
    seen.iter().enumerate().find_map(|(i, a)| {
        seen[i + 1..]
            .iter()
            .find(|b| {
                clashes(a, b)
                    && !clash_scope::confined_to_one_child(ctx, &sheets_of(&[a.origin, b.origin]))
            })
            .map(|b| (a, b))
    })
}

/// The finding for a clashing pair, owned by the sheet holding its first Port.
fn clash_diagnostic(sources: &Sources, first: &PortBus, other: &PortBus) -> Diagnostic {
    let where_of = |origin: usize| match sources[origin].0 {
        Some(file) => format!("'{file}'"),
        None => "this sheet".to_string(),
    };
    let (kind, message) = if first.width == other.width {
        (
            RuleKind::BusNameConflict,
            format!(
                "Port bus names disagree: '{}' on {} and '{}' on {} each carry {} signals, \
                 but different ones",
                first.label.text,
                where_of(first.origin),
                other.label.text,
                where_of(other.origin),
                first.width,
            ),
        )
    } else {
        (
            RuleKind::BusBitWidthMismatch,
            format!(
                "Port bus width mismatch: '{}' on {} carries {} signals, '{}' on {} carries \
                 {}; only the shared signals join",
                first.label.text,
                where_of(first.origin),
                first.width,
                other.label.text,
                where_of(other.origin),
                other.width,
            ),
        )
    };
    let d = Diagnostic::new(kind, message, first.label.position);
    // The later Port always sits on another sheet, so a peer is never on the
    // same canvas as the primary.
    if first.origin == 0 {
        d.with_primary(sel(first.label.uuid, SelectedKind::Label))
    } else {
        d.in_space(LocationSpace::ChildSheet)
            .owned_by(sources[first.origin].1.key.clone())
    }
}
