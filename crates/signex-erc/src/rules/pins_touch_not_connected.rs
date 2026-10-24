//! Rule: PinsTouchNotConnected.
//!
//! Pin-to-pin contact is not a connection (ADR-0002 D5.1): the netlist never
//! joins two pins just because their tips share a point. A user who butts two
//! parts together usually *means* to connect them, so ERC says out loud that
//! nothing joined them and asks for a wire.
//!
//! Flagged: pin tips of two **different** symbols on one 1 µm point with no
//! wire endpoint, junction, label or no-connect there.
//!
//! Not flagged:
//! - stacked pins of one symbol — a part's own geometry, not a user contact;
//! - two power ports — same value joins by name, so nothing is missing;
//!   different values are `PowerPortShort`'s business (one touch, one
//!   diagnostic);
//! - a point where a wire, junction, label or no-connect sits — that anchor
//!   is what connects the pins, so they are connected.

use std::collections::HashMap;

use signex_net::{ConnectedPoints, pt_key};
use signex_types::schematic::SelectedKind;

use crate::context::{ErcContext, ErcPin, ErcSymbol};
use crate::diagnostic::Diagnostic;
use crate::{RuleKind, sel};

/// `designator.pin`, with a placeholder for an unannotated symbol. A power
/// port is named by its value instead ("GND power port").
fn pin_label(symbol: &ErcSymbol, pin: &ErcPin) -> String {
    if symbol.is_power && !symbol.value.is_empty() {
        return format!("{} power port", symbol.value);
    }
    let reference = if symbol.reference.is_empty() {
        "(unnamed)"
    } else {
        symbol.reference.as_str()
    };
    format!("{reference}.{}", pin.number)
}

/// A port-to-port pair is never this rule's business: same value joins by
/// name, so nothing is missing; different values are `PowerPortShort`'s.
fn port_pair(a: &ErcSymbol, b: &ErcSymbol) -> bool {
    let named_port = |s: &ErcSymbol| s.is_power && !s.value.is_empty();
    named_port(a) && named_port(b)
}

pub(crate) fn pins_touch_not_connected(ctx: &ErcContext, out: &mut Vec<Diagnostic>) {
    // The shared gate: the same anchors `build_netlist` treats as a connection.
    let mut anchors = ConnectedPoints::new();
    anchors.add_anchors(ctx.wires.iter().flat_map(|w| [w.start, w.end]));
    anchors.add_anchors(ctx.junctions.iter().map(|j| j.position));
    anchors.add_anchors(ctx.labels.iter().map(|l| l.position));
    anchors.add_anchors(ctx.no_connects.iter().map(|nc| nc.position));

    let mut by_point: HashMap<(i64, i64), Vec<(&ErcSymbol, &ErcPin)>> = HashMap::new();
    for symbol in &ctx.symbols {
        for pin in &symbol.pins {
            by_point
                .entry(pt_key(&pin.world_pos))
                .or_default()
                .push((symbol, pin));
        }
    }

    // Deterministic order, independent of hash iteration.
    let mut points: Vec<_> = by_point.into_iter().collect();
    points.sort_by_key(|(k, _)| *k);

    for (_, pins) in points {
        let Some(&(first_symbol, first_pin)) = pins.first() else {
            continue;
        };
        if anchors.contains(&first_pin.world_pos) {
            continue;
        }
        // Report each pin of a different symbol against the first pin placed
        // there, so a stack of three parts names every pair member once.
        for &(symbol, pin) in pins.iter().skip(1) {
            if symbol.uuid == first_symbol.uuid {
                continue;
            }
            if port_pair(first_symbol, symbol) {
                continue;
            }
            let (subject, advice) = if first_symbol.is_power || symbol.is_power {
                (
                    "",
                    "pin sits on a power port without a wire — connect with a wire or move the port",
                )
            } else {
                ("Pins ", "add a wire")
            };
            out.push(
                Diagnostic::new(
                    RuleKind::PinsTouchNotConnected,
                    format!(
                        "{subject}{} and {} touch but are not connected — {advice}",
                        pin_label(first_symbol, first_pin),
                        pin_label(symbol, pin),
                    ),
                    pin.world_pos,
                )
                .with_primary(sel(first_symbol.uuid, SelectedKind::Symbol))
                .with_peer(sel(symbol.uuid, SelectedKind::Symbol)),
            );
        }
    }
}
