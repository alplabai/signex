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

use crate::context::ErcContext;
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
                    .with_primary(sel(pin.uuid, SelectedKind::SheetPin)),
                );
            }
        }
    }
}

/// A Port (Global label) names one bus project-wide, so two Ports with the
/// same [`join_key`](signex_types::schematic::BusName::join_key) but different
/// members merge only on the members they share. Reports each such bus once,
/// naming the first two sheets that disagree; equally wide Ports that carry
/// different signals are a name conflict, not a width mismatch. Local net label buses never leave their
/// sheet and are not compared.
pub(crate) fn global_bus_width_mismatch(ctx: &ErcContext, out: &mut Vec<Diagnostic>) {
    let sources = sheets_in_scope(ctx);
    if sources.len() < 2 && ctx.labels.len() < 2 {
        return;
    }
    // join key -> one entry per Global bus label, in scope order.
    type Entry<'a> = (usize, &'a crate::context::ErcLabel, HashSet<String>, usize);
    type Seen<'a> = Vec<Entry<'a>>;
    let mut by_key: BTreeMap<String, Seen> = BTreeMap::new();
    for (origin, (_, sheet)) in sources.iter().enumerate() {
        for l in &sheet.labels {
            if l.label_type != LabelType::Global {
                continue;
            }
            let Some(bus) = l.port_name().and_then(|n| parse_bus_name(n).ok()) else {
                continue;
            };
            let members: HashSet<String> = bus.members().into_iter().collect();
            by_key
                .entry(bus.join_key())
                .or_default()
                .push((origin, l, members, bus.width()));
        }
    }
    for seen in by_key.values() {
        // Two names on one sheet are the same-sheet bus rules' finding; only a
        // disagreement between different sheets is reported here.
        let clashes = |a: &Entry, b: &Entry| a.0 != b.0 && a.2 != b.2;
        let Some((first, other)) = seen
            .iter()
            .enumerate()
            .find_map(|(i, a)| seen[i + 1..].iter().find(|b| clashes(a, b)).map(|b| (a, b)))
        else {
            continue;
        };
        // A clash wholly inside one child's subtree is that child's finding.
        // Every sheet taking part in some clash counts, not just the first pair.
        let mut origins: Vec<usize> = seen
            .iter()
            .filter(|a| seen.iter().any(|b| clashes(a, b)))
            .map(|a| a.0)
            .collect();
        origins.sort_unstable();
        origins.dedup();
        if origins[0] != 0 {
            let involved: Vec<&ErcContext> = origins.iter().map(|&o| sources[o].1).collect();
            if clash_scope::confined_to_one_child(ctx, &involved) {
                continue;
            }
        }
        let where_of = |origin: usize| match sources[origin].0 {
            Some(file) => format!("'{file}'"),
            None => "this sheet".to_string(),
        };
        let (kind, message) = if first.3 == other.3 {
            (
                RuleKind::BusNameConflict,
                format!(
                    "Port bus names disagree: '{}' on {} and '{}' on {} each carry {} signals, \
                     but different ones",
                    first.1.text,
                    where_of(first.0),
                    other.1.text,
                    where_of(other.0),
                    first.3,
                ),
            )
        } else {
            (
                RuleKind::BusBitWidthMismatch,
                format!(
                    "Port bus width mismatch: '{}' on {} carries {} signals, '{}' on {} carries \
                     {}; only the shared signals join",
                    first.1.text,
                    where_of(first.0),
                    first.3,
                    other.1.text,
                    where_of(other.0),
                    other.3,
                ),
            )
        };
        let mut d = Diagnostic::new(kind, message, first.1.position);
        if first.0 == 0 {
            d = d.with_primary(sel(first.1.uuid, SelectedKind::Label));
            if other.0 == 0 {
                d = d.with_peer(sel(other.1.uuid, SelectedKind::Label));
            }
        } else {
            d = d
                .in_space(LocationSpace::ChildSheet)
                .owned_by(sources[first.0].1.key.clone());
        }
        out.push(d);
    }
}
