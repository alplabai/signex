//! Rules on a sheet symbol's own pins (#1836): a pin nothing is wired to, and
//! two output pins driving one net. Both read only this sheet's symbols, so
//! they hold whether or not the child file loaded.

use std::collections::HashMap;

use signex_net::SegmentIndex;
use signex_types::schematic::SelectedKind;

use super::{on_any_bus_endpoint, wire_connectivity, wire_pairs};
use crate::context::{ErcChildSheet, ErcContext, ErcSheetPin};
use crate::diagnostic::Diagnostic;
use crate::{RuleKind, sel};

/// A pin is connected when `ErcSheetPin::connected` says so (the netlist's
/// own contact rules, `signex_net::SheetPinContacts`) or a bus ends on it. A
/// bus is matched at its ends only, like the label rules (ADR-0002 D5.4): a
/// bus-typed pin mid-bus is reported, since the netlist does not anchor buses
/// by interior.
pub(crate) fn sheet_pin_unconnected(ctx: &ErcContext, out: &mut Vec<Diagnostic>) {
    for sheet in &ctx.child_sheets {
        for pin in &sheet.pins {
            if pin.connected || on_any_bus_endpoint(&pin.position, ctx) {
                continue;
            }
            out.push(
                Diagnostic::new(
                    RuleKind::SheetPinUnconnected,
                    format!(
                        "Sheet pin '{}' on '{}' is not connected; port '{}' in '{}' floats",
                        pin.name, sheet.name, pin.name, sheet.filename,
                    ),
                    pin.position,
                )
                .with_primary(sel(pin.uuid, SelectedKind::SheetPin)),
            );
        }
    }
}

fn is_output(pin: &ErcSheetPin) -> bool {
    pin.direction.trim().eq_ignore_ascii_case("output")
}

/// Two `output` sheet pins on one net of this sheet. Nets joined by a
/// same-name Global / local label count as one, as in the netlist. One
/// finding per extra output, pointing back at the first.
pub(crate) fn sheet_pin_direction_conflict(ctx: &ErcContext, out: &mut Vec<Diagnostic>) {
    let outputs: Vec<(&ErcChildSheet, &ErcSheetPin)> = ctx
        .child_sheets
        .iter()
        .flat_map(|s| s.pins.iter().map(move |p| (s, p)))
        .filter(|(_, p)| is_output(p))
        .collect();
    if outputs.len() < 2 {
        return;
    }
    let pairs = wire_pairs(ctx);
    let index = SegmentIndex::new(&pairs);
    let mut conn = wire_connectivity(ctx);
    conn.merge_named_labels(
        ctx.labels
            .iter()
            .filter(|l| !l.text.is_empty())
            .map(|l| (l.position, l.label_type, l.text.as_str())),
        &pairs,
    );
    let junctions: Vec<_> = ctx.junctions.iter().map(|j| j.position).collect();
    let power_pins: Vec<_> = ctx
        .symbols
        .iter()
        .filter(|s| s.is_power && !s.value.is_empty() && !signex_net::is_power_flag(&s.lib_id))
        .flat_map(|s| s.pins.iter().map(|p| (p.world_pos, s.value.as_str())))
        .collect();
    conn.merge_power_ports(
        power_pins,
        ctx.labels
            .iter()
            .map(|l| (l.position, l.label_type, l.text.as_str())),
        &pairs,
        &junctions,
    );
    // Anchor every pin before reading any root, so a later union cannot
    // leave an earlier pin's root stale.
    for (_, pin) in &outputs {
        conn.root_of_anchored_in(&pin.position, &index);
    }
    let mut first: HashMap<_, (&ErcChildSheet, &ErcSheetPin)> = HashMap::new();
    for (sheet, pin) in outputs {
        let root = conn.root_of_anchored_in(&pin.position, &index);
        let Some((first_sheet, first_pin)) = first.get(&root).copied() else {
            first.insert(root, (sheet, pin));
            continue;
        };
        out.push(
            Diagnostic::new(
                RuleKind::SheetPinDirectionConflict,
                format!(
                    "Output sheet pin '{}' on '{}' drives the same net as output pin '{}' on '{}'",
                    pin.name, sheet.name, first_pin.name, first_sheet.name,
                ),
                pin.position,
            )
            .with_primary(sel(pin.uuid, SelectedKind::SheetPin))
            .with_peer(sel(first_pin.uuid, SelectedKind::SheetPin)),
        );
    }
}
