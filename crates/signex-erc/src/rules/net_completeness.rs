//! Net completeness rules (#1286): single-pin nets, nets with no driver,
//! power inputs with no source, and nets that carry only labels.
//!
//! A net that continues off this sheet (see [`NetMembers::leaves_sheet`]) may
//! be completed elsewhere, so these rules skip it rather than guess. Why the
//! driver sets are what they are: `docs/ERC_RULES.md`, "Completeness rules".

use signex_types::schematic::{PinDirection, SelectedKind};

use super::net_members::{NetMembers, net_members};
use crate::context::{ErcContext, ErcPin, ErcSymbol};
use crate::diagnostic::Diagnostic;
use crate::{RuleKind, sel};

/// Run the four net completeness rules over one shared membership pass.
pub(crate) fn net_completeness(ctx: &ErcContext, out: &mut Vec<Diagnostic>) {
    let nets = net_members(ctx);
    single_pin_net(&nets, out);
    net_with_no_driver(&nets, out);
    power_input_not_driven(&nets, out);
    labels_only_net(&nets, out);
}

fn pin_name(symbol: &ErcSymbol, pin: &ErcPin) -> String {
    let reference = match symbol.reference.trim() {
        "" => "(unnamed)",
        r => r,
    };
    format!("{reference}.{}", pin.number)
}

/// A finding at `pin`, about `net`.
fn at_pin(
    kind: RuleKind,
    message: String,
    net: &NetMembers,
    symbol: &ErcSymbol,
    pin: &ErcPin,
) -> Diagnostic {
    Diagnostic::new(kind, message, pin.world_pos)
        .with_primary(sel(symbol.uuid, SelectedKind::Symbol))
        .at_pin_number(pin.number.clone())
        .about_net(net.finding())
}

fn single_pin_net(nets: &[NetMembers], out: &mut Vec<Diagnostic>) {
    for net in nets {
        let [(symbol, pin)] = net.pins.as_slice() else {
            continue;
        };
        if net.leaves_sheet || !net.power_ports.is_empty() {
            continue;
        }
        // Only a named net: a pin with a bare wire stub to nowhere is
        // `DanglingWire`'s finding, not a second one here.
        if !net.has_local_label() {
            continue;
        }
        let message = format!(
            "Net '{}' connects only one pin, {}",
            net.name(),
            pin_name(symbol, pin)
        );
        out.push(at_pin(RuleKind::SinglePinNet, message, net, symbol, pin));
    }
}

/// Pin types that may drive a net. Only inputs, power inputs and no-connect
/// pins cannot; unknown kinds (`Unclassified`, `Passive`, `Clock`, ...) are
/// given the benefit of the doubt.
fn can_drive(direction: PinDirection) -> bool {
    !matches!(
        direction,
        PinDirection::Input | PinDirection::PowerInput | PinDirection::DoNotConnect
    )
}

/// `true` when the net may get its signal from somewhere this sheet's run
/// cannot see, or from a power port.
fn fed_from_outside(net: &NetMembers) -> bool {
    net.leaves_sheet || !net.power_ports.is_empty()
}

fn net_with_no_driver(nets: &[NetMembers], out: &mut Vec<Diagnostic>) {
    for net in nets {
        if fed_from_outside(net) || net.pins.iter().any(|(_, p)| can_drive(p.electrical_type)) {
            continue;
        }
        let Some((symbol, pin)) = net
            .pins
            .iter()
            .find(|(_, p)| p.electrical_type == PinDirection::Input)
        else {
            continue;
        };
        let message = format!(
            "Net '{}' has input pins but nothing drives it (first: {})",
            net.name(),
            pin_name(symbol, pin)
        );
        out.push(at_pin(RuleKind::NetWithNoDriver, message, net, symbol, pin));
    }
}

/// A power input on a net nothing can feed: every pin is an input, power
/// input or no-connect. Distinct from `MissingPowerFlag`, which checks power
/// *ports*: this rule skips every net that carries a power port or flag.
fn power_input_not_driven(nets: &[NetMembers], out: &mut Vec<Diagnostic>) {
    for net in nets {
        // Any pin that may drive counts as a source, as for
        // `NetWithNoDriver`: a regulator output typed `Output`, or a supply
        // fed through a `Passive` ferrite or 0R.
        let sourced = net.pins.iter().any(|(_, p)| can_drive(p.electrical_type));
        if fed_from_outside(net) || sourced {
            continue;
        }
        let Some((symbol, pin)) = net
            .pins
            .iter()
            .find(|(_, p)| p.electrical_type == PinDirection::PowerInput)
        else {
            continue;
        };
        let message = format!(
            "Power input {} on net '{}' has no power source: add a power port, a power flag or a power output",
            pin_name(symbol, pin),
            net.name()
        );
        out.push(at_pin(
            RuleKind::PowerInputNotDriven,
            message,
            net,
            symbol,
            pin,
        ));
    }
}

fn labels_only_net(nets: &[NetMembers], out: &mut Vec<Diagnostic>) {
    for net in nets {
        let Some(first) = net.labels.first() else {
            continue;
        };
        if !net.pins.is_empty() || fed_from_outside(net) || net.wire_count == 0 {
            continue;
        }
        out.push(
            Diagnostic::new(
                RuleKind::LabelsOnlyNet,
                format!("Net '{}' carries only labels and wires, no pin", net.name()),
                first.position,
            )
            .with_primary(sel(first.uuid, SelectedKind::Label))
            .about_net(net.finding()),
        );
    }
}
