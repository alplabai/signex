//! Bus rules: width agreement, name agreement, naming, and entry-wire
//! membership.
//!
//! A bus carries the members its label(s) name (`signex_net::analyse_buses`,
//! parsed by `signex_types::schematic::parse_bus_name`). A wire fed through a
//! bus entry joins a member net only through its own member label, so these
//! rules check that the label is there and belongs to the bus. The analysis
//! runs once per ERC run and is shared by every rule here.
//!
//! # Label anchoring
//!
//! A label that is a *bus name* (its text parses as one, or is a malformed
//! attempt at one) sits on a bus
//! **anywhere along a segment, endpoint or mid-span** — mid-span is where users
//! put it. This single rule is used by every rule that asks "is this label on
//! a bus": here, `orphan_label` and `hier_port_disconnected`. Any other label
//! keeps endpoint-only contact with a bus (D5.4): a net label must not be
//! more leniently connected to a bus than `build_netlist` connects it.

use signex_net::{BusAnalysis, BusGeometry, BusGroup, analyse_buses};
use signex_types::schematic::{Point, SelectedKind, parse_bus_name};

use crate::context::ErcContext;
use crate::diagnostic::Diagnostic;
use crate::{RuleKind, sel};

/// The shared bus analysis for `ctx`, fed from the snapshot geometry. The
/// engine computes it once and hands it to every bus rule.
pub(crate) fn bus_analysis(ctx: &ErcContext) -> BusAnalysis {
    let buses: Vec<(Point, Point)> = ctx.buses.iter().map(|b| (b.start, b.end)).collect();
    let entries: Vec<(Point, (f64, f64))> = ctx
        .bus_entries
        .iter()
        .map(|e| (e.position, e.size))
        .collect();
    let wires: Vec<(Point, Point)> = ctx.wires.iter().map(|w| (w.start, w.end)).collect();
    let junctions: Vec<Point> = ctx.junctions.iter().map(|j| j.position).collect();
    let labels: Vec<(Point, &str)> = ctx
        .labels
        .iter()
        .map(|l| (l.position, l.text.as_str()))
        .collect();
    analyse_buses(&BusGeometry {
        buses: &buses,
        entries: &entries,
        wires: &wires,
        junctions: &junctions,
        labels: &labels,
    })
}

/// True when the label touches a bus under the anchoring rule in the module
/// docs: a bus-name label, or a malformed attempt at one, anywhere along a
/// segment; any other label at an
/// endpoint only.
pub(super) fn label_touches_bus(ctx: &ErcContext, label: &crate::context::ErcLabel) -> bool {
    let attempts_bus_name = match parse_bus_name(&label.text) {
        Ok(_) => true,
        Err(e) => e.looks_like_bus_name(),
    };
    if attempts_bus_name {
        let pk = signex_net::pt_key(&label.position);
        ctx.buses.iter().any(|b| {
            signex_net::point_on_segment(
                pk,
                signex_net::pt_key(&b.start),
                signex_net::pt_key(&b.end),
            )
        })
    } else {
        super::on_any_bus_endpoint(&label.position, ctx)
    }
}

/// Two names on one run that carry a different number of signals.
pub(crate) fn bus_bit_width_mismatch(
    ctx: &ErcContext,
    bus: &BusAnalysis,
    out: &mut Vec<Diagnostic>,
) {
    for group in &bus.groups {
        if group.names.len() < 2 {
            continue;
        }
        let majority = majority_width(group);
        let Some(reference) = group.names.iter().find(|n| n.width == majority) else {
            continue;
        };
        let Some(conflict) = group.names.iter().find(|n| n.width != majority) else {
            continue;
        };
        let conflict_width = conflict.width;
        let (reference, conflict) = (&ctx.labels[reference.label], &ctx.labels[conflict.label]);
        out.push(
            Diagnostic::new(
                RuleKind::BusBitWidthMismatch,
                format!(
                    "Bus width mismatch: '{}' ({majority} signals) vs '{}' ({} signals)",
                    reference.text, conflict.text, conflict_width,
                ),
                conflict.position,
            )
            .with_primary(sel(conflict.uuid, SelectedKind::Label))
            .with_peer(sel(reference.uuid, SelectedKind::Label)),
        );
    }
}

/// The most common width; the earliest label's width wins a tie, so the
/// reference label is deterministic.
fn majority_width(group: &BusGroup) -> usize {
    let count = |w: usize| group.names.iter().filter(|n| n.width == w).count();
    group
        .names
        .iter()
        .map(|n| n.width)
        .fold((0, 0), |best, w| {
            let c = count(w);
            if c > best.1 { (w, c) } else { best }
        })
        .0
}

/// Two names on one run that are equally wide but carry different signals
/// (`D[0..7]` and `D[8..15]`, `D[7..0]` and `A[0..7]`). A run whose widths
/// already disagree is left to [`bus_bit_width_mismatch`].
pub(crate) fn bus_name_conflict(ctx: &ErcContext, bus: &BusAnalysis, out: &mut Vec<Diagnostic>) {
    for group in &bus.groups {
        let Some(first) = group.names.first() else {
            continue;
        };
        if group.names.iter().any(|n| n.width != first.width) {
            continue;
        }
        let Some(other) = group.names.iter().find(|n| n.members != first.members) else {
            continue;
        };
        let (reference, conflict) = (&ctx.labels[first.label], &ctx.labels[other.label]);
        out.push(
            Diagnostic::new(
                RuleKind::BusNameConflict,
                format!(
                    "Bus names disagree: '{}' and '{}' carry different signals",
                    reference.text, conflict.text
                ),
                conflict.position,
            )
            .with_primary(sel(conflict.uuid, SelectedKind::Label))
            .with_peer(sel(reference.uuid, SelectedKind::Label)),
        );
    }
}

/// A bus with no valid name, and every label on a bus that is malformed.
///
/// A plain net name on a bus (`VCC`) is not an attempt at a bus name; it is
/// only mentioned when the bus has nothing else. Any other refusal (a stray
/// space, a missing bracket, text after `]`) is reported on its own label even
/// when the bus has another valid name, because the user clearly meant a bus
/// name and it is silently doing nothing.
pub(crate) fn bus_unnamed(ctx: &ErcContext, bus: &BusAnalysis, out: &mut Vec<Diagnostic>) {
    for group in &bus.groups {
        let unnamed = group.names.is_empty();
        let mut reported_bad = false;
        for (li, why) in &group.rejected {
            if !why.looks_like_bus_name() {
                continue;
            }
            reported_bad = true;
            let label = &ctx.labels[*li];
            let tail = if unnamed {
                "; the bus carries no members"
            } else {
                ""
            };
            out.push(
                Diagnostic::new(
                    RuleKind::BusUnnamed,
                    format!(
                        "Bus label '{}' is not a valid bus name ({why}){tail}",
                        label.text
                    ),
                    label.position,
                )
                .with_primary(sel(label.uuid, SelectedKind::Label)),
            );
        }
        if !unnamed || reported_bad {
            continue;
        }
        let first = &ctx.buses[group.segments[0]];
        let message = match group.rejected.first() {
            Some((li, why)) => format!(
                "Bus has no valid name: label '{}' is not a bus name ({why})",
                ctx.labels[*li].text
            ),
            None => "Bus has no name — label it like 'D[0..7]' or '{SDA SCL}'".to_string(),
        };
        out.push(
            Diagnostic::new(RuleKind::BusUnnamed, message, first.start)
                .with_primary(sel(first.uuid, SelectedKind::Bus)),
        );
    }
}

pub(crate) fn bus_entry_unlabeled(ctx: &ErcContext, bus: &BusAnalysis, out: &mut Vec<Diagnostic>) {
    for attach in &bus.entries {
        if !attach.member_labels.is_empty() {
            continue;
        }
        out.push(
            Diagnostic::new(
                RuleKind::BusEntryUnlabeled,
                "Bus entry wire has no member label — name it with a signal of the bus",
                attach.wire_end,
            )
            .with_primary(sel(
                ctx.bus_entries[attach.entry].uuid,
                SelectedKind::BusEntry,
            )),
        );
    }
}

pub(crate) fn bus_member_not_in_bus(
    ctx: &ErcContext,
    bus: &BusAnalysis,
    out: &mut Vec<Diagnostic>,
) {
    let mut reported = std::collections::HashSet::new();
    for attach in &bus.entries {
        let group = &bus.groups[attach.group];
        // A nameless bus has no member set to test against; `bus_unnamed`
        // already speaks for it.
        let Some(name) = group.names.first() else {
            continue;
        };
        for &li in &attach.member_labels {
            let label = &ctx.labels[li];
            // One wire fed by two entries still holds one label: warn once.
            if group.carries(&label.text) || !reported.insert(li) {
                continue;
            }
            out.push(
                Diagnostic::new(
                    RuleKind::BusMemberNotInBus,
                    format!(
                        "Label '{}' is not a member of bus '{}'",
                        label.text, ctx.labels[name.label].text
                    ),
                    label.position,
                )
                .with_primary(sel(label.uuid, SelectedKind::Label))
                .with_peer(sel(
                    ctx.bus_entries[attach.entry].uuid,
                    SelectedKind::BusEntry,
                )),
            );
        }
    }
}

/// A label that reads as a bus name (`D[0..7]`) but touches no bus. On a wire
/// it names an ordinary net, which is almost always a mistake: the user meant
/// to label a bus. Connectivity is left as it is; whether a bus-name label may
/// name a net at all is decided with cross-sheet buses (#1049).
pub(crate) fn bus_label_on_wire(ctx: &ErcContext, bus: &BusAnalysis, out: &mut Vec<Diagnostic>) {
    let on_a_bus: std::collections::HashSet<usize> = bus
        .groups
        .iter()
        .flat_map(|g| g.names.iter().map(|n| n.label))
        .collect();
    let wires = super::wire_pairs(ctx);
    for (li, label) in ctx.labels.iter().enumerate() {
        if on_a_bus.contains(&li)
            || parse_bus_name(&label.text).is_err()
            || !super::on_any_wire(&label.position, &wires)
        {
            continue;
        }
        out.push(
            Diagnostic::new(
                RuleKind::BusLabelOnWire,
                format!(
                    "Label '{}' reads as a bus name but is not on a bus, so it names a net",
                    label.text
                ),
                label.position,
            )
            .with_primary(sel(label.uuid, SelectedKind::Label)),
        );
    }
}
