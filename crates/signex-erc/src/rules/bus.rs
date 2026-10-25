//! Bus rules: width agreement, naming, and entry-wire membership.
//!
//! A bus carries the members its label(s) name (`signex_net::analyse_buses`,
//! parsed by `signex_types::schematic::parse_bus_name`). A wire fed through a
//! bus entry joins a member net only through its own member label, so these
//! rules check that the label is there and belongs to the bus.
//!
//! # Label anchoring
//!
//! A label that is a *bus name* (its text parses as one) sits on a bus
//! **anywhere along a segment, endpoint or mid-span** — mid-span is where users
//! put it. This single rule is used by every rule that asks "is this label on
//! a bus": here, `orphan_label` and `hier_port_disconnected`. Any other label
//! keeps endpoint-only contact with a bus (D5.4): a net label must not be
//! more leniently connected to a bus than `build_netlist` connects it.

use signex_net::{BusAnalysis, BusGeometry, analyse_buses};
use signex_types::schematic::{Point, SelectedKind, parse_bus_name};

use crate::context::ErcContext;
use crate::diagnostic::Diagnostic;
use crate::{RuleKind, sel};

/// The shared bus analysis for `ctx`, fed from the snapshot geometry.
fn analyse(ctx: &ErcContext) -> BusAnalysis {
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
/// docs: a bus-name label anywhere along a segment, any other label at an
/// endpoint only.
pub(super) fn label_touches_bus(ctx: &ErcContext, label: &crate::context::ErcLabel) -> bool {
    if parse_bus_name(&label.text).is_ok() {
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

pub(crate) fn bus_bit_width_mismatch(ctx: &ErcContext, out: &mut Vec<Diagnostic>) {
    let analysis = analyse(ctx);
    for group in &analysis.groups {
        // Width of each naming label; labels on one run must agree.
        let widths: Vec<(usize, usize)> = group
            .names
            .iter()
            .filter_map(|&li| {
                let width = parse_bus_name(&ctx.labels[li].text).ok()?.width();
                Some((li, width))
            })
            .collect();
        if widths.len() < 2 {
            continue;
        }
        let majority = majority_width(&widths);
        let Some(&(ref_li, _)) = widths.iter().find(|(_, w)| *w == majority) else {
            continue;
        };
        let Some(&(bad_li, bad_w)) = widths.iter().find(|(_, w)| *w != majority) else {
            continue;
        };
        let (reference, conflict) = (&ctx.labels[ref_li], &ctx.labels[bad_li]);
        out.push(
            Diagnostic::new(
                RuleKind::BusBitWidthMismatch,
                format!(
                    "Bus width mismatch: '{}' ({majority} signals) vs '{}' ({bad_w} signals)",
                    reference.text, conflict.text,
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
fn majority_width(widths: &[(usize, usize)]) -> usize {
    let count = |w: usize| widths.iter().filter(|(_, x)| *x == w).count();
    widths
        .iter()
        .map(|(_, w)| *w)
        .fold((0, 0), |best, w| {
            let c = count(w);
            if c > best.1 { (w, c) } else { best }
        })
        .0
}

pub(crate) fn bus_unnamed(ctx: &ErcContext, out: &mut Vec<Diagnostic>) {
    for group in &analyse(ctx).groups {
        if !group.names.is_empty() {
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

pub(crate) fn bus_entry_unlabeled(ctx: &ErcContext, out: &mut Vec<Diagnostic>) {
    for attach in &analyse(ctx).entries {
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

pub(crate) fn bus_member_not_in_bus(ctx: &ErcContext, out: &mut Vec<Diagnostic>) {
    let analysis = analyse(ctx);
    for attach in &analysis.entries {
        let group = &analysis.groups[attach.group];
        // A nameless bus has no member set to test against; `bus_unnamed`
        // already speaks for it.
        if group.members.is_empty() {
            continue;
        }
        for &li in &attach.member_labels {
            let label = &ctx.labels[li];
            if group.carries(&label.text) {
                continue;
            }
            let bus_label = &ctx.labels[group.names[0]];
            out.push(
                Diagnostic::new(
                    RuleKind::BusMemberNotInBus,
                    format!(
                        "Label '{}' is not a member of bus '{}'",
                        label.text, bus_label.text
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
