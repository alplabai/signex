//! Rule: PinTypeConflict.
//!
//! Every net whose pins the pin connection matrix ([`crate::pin_matrix`])
//! marks as conflicting yields ONE finding, at the severity of the worst
//! conflicting cell; the other conflicting pairs are listed in its message so
//! a net of N outputs is one finding, not N(N-1)/2. `Off` cells emit nothing.
//! A power flag (`PWR_FLAG`) is exempt from the `PowerOutput` x `PowerOutput`
//! pair.
//!
//! Pins of the same symbol never conflict with each other.
//!
//! The finding is located at the first pin of the worst pair, with `primary`
//! and `peer` set to the two owning symbols — the same two-object shape
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

/// One conflicting pair of pins on a net, with the matrix cell's verdict.
struct PinPair<'a> {
    a: &'a ErcNetPin,
    axis_a: PinAxis,
    b: &'a ErcNetPin,
    axis_b: PinAxis,
    severity: Severity,
}

/// Lower is worse: Error, then Warning, then Info.
fn badness(severity: Severity) -> u8 {
    match severity {
        Severity::Error => 0,
        Severity::Warning => 1,
        Severity::Info => 2,
        Severity::Off => 3,
    }
}

/// A power flag declares "this rail is driven"; it does not drive it. Power
/// symbols with the same value are merged into one net (#675), so a flag's
/// `PowerOutput` pin meeting another `PowerOutput` pin is the flag doing its
/// job, not two drivers fighting.
fn is_power_flag_pair(p: &PinPair<'_>) -> bool {
    p.axis_a == PinAxis::PowerOut && p.axis_b == PinAxis::PowerOut && (p.a.is_power || p.b.is_power)
}

fn conflicts_on_net(net: &ErcNet, matrix: &PinMatrix, out: &mut Vec<Diagnostic>) {
    let mut pairs: Vec<PinPair<'_>> = Vec::new();
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
            let pair = PinPair {
                a,
                axis_a,
                b,
                axis_b,
                severity,
            };
            if !is_power_flag_pair(&pair) {
                pairs.push(pair);
            }
        }
    }
    // `min_by_key` keeps the first of equal keys, so ties resolve to the
    // first pair found.
    let Some(worst) = pairs.iter().min_by_key(|p| badness(p.severity)) else {
        return;
    };
    out.push(conflict(net, worst, &pairs));
}

fn conflict(net: &ErcNet, worst: &PinPair<'_>, all: &[PinPair<'_>]) -> Diagnostic {
    let on_net = if net.name.is_empty() {
        String::new()
    } else {
        format!(" on net '{}'", net.name)
    };
    let mut message = format!(
        "{} pin of {} conflicts with {} pin of {}{on_net}",
        worst.axis_a.label(),
        display_reference(worst.a),
        worst.axis_b.label(),
        display_reference(worst.b),
    );
    if all.len() > 1 {
        message.push_str(&format!(
            "; {} more conflicting pair(s): {}",
            all.len() - 1,
            other_pairs(worst, all),
        ));
    }
    Diagnostic::new(RuleKind::PinTypeConflict, message, worst.a.position)
        .with_severity(worst.severity)
        .with_primary(sel(worst.a.symbol, SelectedKind::Symbol))
        .with_peer(sel(worst.b.symbol, SelectedKind::Symbol))
}

fn other_pairs(worst: &PinPair<'_>, all: &[PinPair<'_>]) -> String {
    all.iter()
        .filter(|p| !std::ptr::eq(*p, worst))
        .map(|p| {
            format!(
                "{} {} / {} {}",
                display_reference(p.a),
                p.axis_a.label(),
                display_reference(p.b),
                p.axis_b.label()
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn display_reference(pin: &ErcNetPin) -> &str {
    if pin.reference.is_empty() {
        "(unnamed)"
    } else {
        &pin.reference
    }
}
