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
        conflicts_on_net(ctx, net, matrix, out);
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

/// A power flag declares "this rail is driven"; it does not drive it, so its
/// `PowerOutput` pin meeting another `PowerOutput` pin (a regulator output, or
/// another flag on the same rail) is the flag doing its job, not two drivers
/// fighting. Only a flag is exempt: an ordinary power port (a VCC symbol) on
/// the net does not excuse two real drivers. Flags do not join nets by value
/// (see `signex_net::is_power_flag`).
fn is_power_flag_pair(p: &PinPair<'_>) -> bool {
    p.axis_a == PinAxis::PowerOut
        && p.axis_b == PinAxis::PowerOut
        && (p.a.is_power_flag || p.b.is_power_flag)
}

fn conflicts_on_net(ctx: &ErcContext, net: &ErcNet, matrix: &PinMatrix, out: &mut Vec<Diagnostic>) {
    let mut pairs: Vec<PinPair<'_>> = Vec::new();
    // Canonical pin order (symbol uuid, anchor): which pin of a pair is the
    // primary, and which pair wins a tie, never depend on document order.
    let mut pins: Vec<&ErcNetPin> = net.pins.iter().collect();
    pins.sort_by_key(|p| (p.symbol, p.anchor()));
    for (i, a) in pins.iter().copied().enumerate() {
        // A No-ERC directive on a pin tip silences that pin's pairs only. The
        // merged finding sits on one pin, so the engine's positional
        // suppression would otherwise drop (or keep) the whole net.
        if ctx.has_no_erc_directive_at(&a.position) {
            continue;
        }
        let Some(axis_a) = PinAxis::of(a.direction) else {
            continue;
        };
        for b in pins[i + 1..].iter().copied() {
            // Two pins of one part sharing a net (an LDO's paired VOUT pins,
            // a buck converter's several SW pins) are one driver, not two.
            // Keyed on (symbol, occurrence): two placements of one sheet file
            // are two parts. ERC nets are still per sheet with a root
            // occurrence, so this only differs once nets span placements.
            if (a.symbol == b.symbol && a.occurrence == b.occurrence)
                || ctx.has_no_erc_directive_at(&b.position)
            {
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
        .at_pin_number(format!("{}/{}", worst.a.anchor(), worst.b.anchor()))
        .about_net(crate::NetFinding::from_net(net))
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

#[cfg(test)]
mod tests {
    use signex_types::schematic::{InstancePath, PinDirection, Point, SchematicSheet};
    use uuid::Uuid;

    use super::*;

    fn pin(symbol: Uuid, occurrence: InstancePath) -> ErcNetPin {
        ErcNetPin {
            symbol,
            occurrence,
            reference: "U1".into(),
            number: "1".into(),
            position: Point::new(0.0, 0.0),
            local_pos: Point::new(0.0, 0.0),
            direction: PinDirection::Output,
            is_power_flag: false,
        }
    }

    fn net_of(pins: Vec<ErcNetPin>) -> ErcNet {
        ErcNet {
            name: "X".into(),
            names: vec!["X".into()],
            class: String::new(),
            pin_types: pins.iter().map(|p| p.direction).collect(),
            pins,
            has_driver: false,
            has_pullup: false,
        }
    }

    fn findings(net: &ErcNet) -> usize {
        let ctx = ErcContext::from_snapshot(&SchematicSheet::default());
        let mut out = Vec::new();
        conflicts_on_net(&ctx, net, &PinMatrix::baseline(), &mut out);
        out.len()
    }

    #[test]
    fn outputs_of_two_placements_of_one_symbol_conflict() {
        let sym = Uuid::from_u128(7);
        let a = InstancePath::from_chain(vec![Uuid::from_u128(1)]);
        let b = InstancePath::from_chain(vec![Uuid::from_u128(2)]);
        assert_eq!(findings(&net_of(vec![pin(sym, a), pin(sym, b)])), 1);
    }

    #[test]
    fn pins_of_one_placement_never_conflict() {
        let sym = Uuid::from_u128(7);
        let a = InstancePath::from_chain(vec![Uuid::from_u128(1)]);
        assert_eq!(findings(&net_of(vec![pin(sym, a.clone()), pin(sym, a)])), 0);
    }
}
