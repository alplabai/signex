//! Rule: PinTypeConflict.
//!
//! For every net, every pair of pins whose electrical types the pin connection
//! matrix ([`crate::pin_matrix`]) marks as a conflict is reported once, at the
//! severity of that matrix cell. `Off` cells emit nothing.
//!
//! Pins of the same symbol never conflict with each other.
//!
//! Each finding is located at the first pin of the pair, with `primary` and
//! `peer` set to the two owning symbols — the same two-object shape
//! `duplicate_ref_designator` uses. How the 14 `PinDirection` variants fold
//! onto the matrix's six axes is documented on [`PinAxis::of`].

use signex_types::schematic::SelectedKind;

use crate::context::{ErcContext, ErcNet, ErcNetPin};
use crate::diagnostic::Diagnostic;
use crate::pin_matrix::{PinAxis, PinMatrix};
use crate::{RuleKind, Severity, sel};

pub(crate) fn pin_type_conflict(ctx: &ErcContext, matrix: &PinMatrix, out: &mut Vec<Diagnostic>) {
    let first_new = out.len();
    for net in &ctx.nets {
        conflicts_on_net(net, matrix, out);
    }
    // Nets come out of a hash map; order this rule's findings by where they
    // are so two runs over the same sheet report identically.
    out[first_new..].sort_by(|a, b| {
        a.location
            .x
            .total_cmp(&b.location.x)
            .then(a.location.y.total_cmp(&b.location.y))
            .then_with(|| a.message.cmp(&b.message))
    });
}

fn conflicts_on_net(net: &ErcNet, matrix: &PinMatrix, out: &mut Vec<Diagnostic>) {
    for (i, a) in net.pins.iter().enumerate() {
        let Some(axis_a) = PinAxis::of(a.direction) else {
            continue;
        };
        for b in &net.pins[i + 1..] {
            // Two pins of one part sharing a net (an LDO's paired VOUT pins,
            // a buck converter's several SW pins) are one driver, not two.
            if a.symbol == b.symbol {
                continue;
            }
            let Some(axis_b) = PinAxis::of(b.direction) else {
                continue;
            };
            let severity = matrix.pair_severity(axis_a, axis_b);
            if severity == Severity::Off {
                continue;
            }
            out.push(conflict(net, a, axis_a, b, axis_b, severity));
        }
    }
}

fn conflict(
    net: &ErcNet,
    a: &ErcNetPin,
    axis_a: PinAxis,
    b: &ErcNetPin,
    axis_b: PinAxis,
    severity: Severity,
) -> Diagnostic {
    let on_net = if net.name.is_empty() {
        String::new()
    } else {
        format!(" on net '{}'", net.name)
    };
    Diagnostic::new(
        RuleKind::PinTypeConflict,
        format!(
            "{} pin of {} conflicts with {} pin of {}{on_net}",
            axis_a.label(),
            display_reference(a),
            axis_b.label(),
            display_reference(b),
        ),
        a.position,
    )
    .with_severity(severity)
    .with_primary(sel(a.symbol, SelectedKind::Symbol))
    .with_peer(sel(b.symbol, SelectedKind::Symbol))
}

fn display_reference(pin: &ErcNetPin) -> &str {
    if pin.reference.is_empty() {
        "(unnamed)"
    } else {
        &pin.reference
    }
}
